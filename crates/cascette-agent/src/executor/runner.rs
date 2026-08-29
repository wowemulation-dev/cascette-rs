//! Background operation runner.
//!
//! Polls the operation queue and dispatches operations to type-specific executors.
//! Uses a semaphore to enforce the configured concurrency limit.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::error::AgentResult;
use crate::models::operation::{ErrorInfo, OperationState, OperationType};
use crate::server::router::AppState;

/// Background operation runner that processes queued operations.
pub struct OperationRunner {
    state: Arc<AppState>,
    max_concurrent: usize,
    cancellation: CancellationToken,
}

impl OperationRunner {
    /// Create a new runner.
    #[must_use]
    pub fn new(
        state: Arc<AppState>,
        max_concurrent: usize,
        cancellation: CancellationToken,
    ) -> Self {
        Self {
            state,
            max_concurrent,
            cancellation,
        }
    }

    /// Run the operation processing loop until cancellation.
    pub async fn run(self) -> AgentResult<()> {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(self.max_concurrent));

        info!(
            max_concurrent = self.max_concurrent,
            "operation runner started"
        );

        loop {
            tokio::select! {
                () = self.cancellation.cancelled() => {
                    info!("operation runner shutting down");
                    break;
                }
                () = self.state.queue_notify.notified() => {
                    self.process_queue(&semaphore).await;
                }
                () = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                    // Fallback poll in case a notification was missed.
                    self.process_queue(&semaphore).await;
                    // Reap sessions whose game processes have exited.
                    self.state.session_tracker.cleanup_dead_processes().await;
                }
            }
        }

        // Wait for in-flight operations
        info!("waiting for in-flight operations to complete");
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            semaphore.acquire_many(self.max_concurrent as u32),
        )
        .await;

        info!("operation runner stopped");
        Ok(())
    }

    async fn process_queue(&self, semaphore: &Arc<tokio::sync::Semaphore>) {
        let Ok(queued) = self.state.queue.get_queued().await else {
            error!("failed to fetch queued operations");
            return;
        };

        for mut operation in queued {
            let Ok(permit) = semaphore.clone().try_acquire_owned() else {
                break; // At concurrency limit
            };

            let state = self.state.clone();
            let cancellation = self.cancellation.child_token();

            tokio::spawn(async move {
                let _permit = permit;
                let op_id = operation.operation_id.to_string();
                let op_type = operation.operation_type;
                let op_type_str = op_type.to_string();

                info!(
                    operation_id = %op_id,
                    product = %operation.product_code,
                    op_type = %op_type,
                    "starting operation"
                );

                // Metric: increment active operations
                state
                    .metrics
                    .active_operations
                    .with_label_values(&[&op_type_str])
                    .inc();

                // Transition to Initializing
                if let Err(e) = operation.transition_to(OperationState::Initializing) {
                    error!(operation_id = %op_id, error = %e, "failed to start operation");
                    state
                        .metrics
                        .active_operations
                        .with_label_values(&[&op_type_str])
                        .dec();
                    return;
                }
                let _ = state.queue.update(&operation).await;

                // Dispatch to type-specific executor
                let result = match op_type {
                    OperationType::Install => {
                        super::install::execute(&mut operation, &state, &cancellation).await
                    }
                    OperationType::Update => {
                        Box::pin(super::update::execute(
                            &mut operation,
                            &state,
                            &cancellation,
                        ))
                        .await
                    }
                    OperationType::Repair => {
                        super::repair::execute(&mut operation, &state, &cancellation).await
                    }
                    OperationType::Verify => {
                        super::verify::execute(&mut operation, &state, &cancellation).await
                    }
                    OperationType::Uninstall => {
                        super::uninstall::execute(&mut operation, &state, &cancellation).await
                    }
                    OperationType::Backfill => {
                        super::backfill::execute(&mut operation, &state, &cancellation).await
                    }
                    OperationType::Extract => {
                        super::extract::execute(&mut operation, &state, &cancellation).await
                    }
                };

                // Metric: decrement active, increment total
                state
                    .metrics
                    .active_operations
                    .with_label_values(&[&op_type_str])
                    .dec();

                match result {
                    Ok(()) => {
                        info!(operation_id = %op_id, "operation completed");
                        state
                            .metrics
                            .operations_total
                            .with_label_values(&[op_type_str.as_str(), "success"])
                            .inc();
                    }
                    Err(e) => {
                        warn!(operation_id = %op_id, error = %e, "operation failed");
                        state
                            .metrics
                            .operations_total
                            .with_label_values(&[op_type_str.as_str(), "failure"])
                            .inc();
                        let _ = operation.fail(ErrorInfo {
                            code: "execution_error".to_string(),
                            message: e.to_string(),
                            details: None,
                        });
                        let _ = state.queue.update(&operation).await;
                    }
                }

                // Update products_registered metric
                if let Ok(products) = state.registry.list().await {
                    #[allow(clippy::cast_possible_wrap)]
                    state.metrics.products_registered.set(products.len() as i64);
                }
            });
        }
    }
}
