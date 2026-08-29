//! Operation queue: CRUD and query operations for the operation table.

use chrono::Utc;
use tracing::debug;

use crate::error::{AgentError, AgentResult};
use crate::models::operation::{ErrorInfo, Operation, OperationState, OperationType, Priority};
use crate::models::progress::Progress;

use super::db::Database;

/// Operation queue backed by SQLite.
pub struct OperationQueue {
    db: Database,
}

impl OperationQueue {
    /// Create a new queue using the given database.
    #[must_use]
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Insert a new operation.
    ///
    /// # Errors
    ///
    /// Returns an error if the product already has an active operation of this type.
    pub async fn insert(&self, operation: &Operation) -> AgentResult<()> {
        // Check for existing active operation on this product
        if let Some(active) = self
            .find_active_for_product(&operation.product_code)
            .await?
        {
            return Err(AgentError::ActiveOperationExists {
                product: operation.product_code.clone(),
                operation_type: active.operation_type.to_string(),
            });
        }

        let now = Utc::now().to_rfc3339();
        let params_json = operation
            .parameters
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let progress_json = operation
            .progress
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let error_json = operation
            .error
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.db
            .conn()
            .execute(
                "INSERT INTO operations (operation_id, product_code, operation_type,
                                          state, priority, parameters, metadata,
                                          progress, error, created_at, updated_at,
                                          started_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                turso::params![
                    operation.operation_id.to_string(),
                    operation.product_code.clone(),
                    operation.operation_type.to_string(),
                    operation.state.to_string(),
                    operation.priority.to_string(),
                    params_json,
                    operation
                        .metadata
                        .as_ref()
                        .map(std::string::ToString::to_string),
                    progress_json,
                    error_json,
                    now.clone(),
                    now,
                    operation.started_at.map(|dt| dt.to_rfc3339()),
                    operation.completed_at.map(|dt| dt.to_rfc3339())
                ],
            )
            .await?;

        debug!(
            operation_id = %operation.operation_id,
            product = %operation.product_code,
            op_type = %operation.operation_type,
            "operation inserted"
        );
        Ok(())
    }

    /// Get an operation by ID.
    ///
    /// # Errors
    ///
    /// Returns `OperationNotFound` if the operation does not exist.
    pub async fn get(&self, operation_id: &str) -> AgentResult<Operation> {
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT operation_id, product_code, operation_type, state, priority,
                        parameters, metadata, progress, error,
                        created_at, updated_at, started_at, completed_at
                 FROM operations WHERE operation_id = ?1",
                turso::params![operation_id],
            )
            .await?;

        match rows.next().await? {
            Some(row) => row_to_operation(&row),
            None => Err(AgentError::OperationNotFound(operation_id.to_string())),
        }
    }

    /// Find the active (non-terminal) operation for a product, if any.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn find_active_for_product(
        &self,
        product_code: &str,
    ) -> AgentResult<Option<Operation>> {
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT operation_id, product_code, operation_type, state, priority,
                        parameters, metadata, progress, error,
                        created_at, updated_at, started_at, completed_at
                 FROM operations
                 WHERE product_code = ?1
                   AND state NOT IN ('complete', 'failed', 'cancelled')
                 ORDER BY created_at DESC
                 LIMIT 1",
                turso::params![product_code],
            )
            .await?;

        match rows.next().await? {
            Some(row) => Ok(Some(row_to_operation(&row)?)),
            None => Ok(None),
        }
    }

    /// Get all queued operations ordered by priority and creation time.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn get_queued(&self) -> AgentResult<Vec<Operation>> {
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT operation_id, product_code, operation_type, state, priority,
                        parameters, metadata, progress, error,
                        created_at, updated_at, started_at, completed_at
                 FROM operations
                 WHERE state = 'queued'
                 ORDER BY
                    CASE priority WHEN 'high' THEN 0 WHEN 'normal' THEN 1 ELSE 2 END,
                    created_at ASC",
                (),
            )
            .await?;

        let mut ops = Vec::new();
        while let Some(row) = rows.next().await? {
            ops.push(row_to_operation(&row)?);
        }
        Ok(ops)
    }

    /// Update an operation's state, progress, and error.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn update(&self, operation: &Operation) -> AgentResult<()> {
        let now = Utc::now().to_rfc3339();
        let progress_json = operation
            .progress
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;
        let error_json = operation
            .error
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.db
            .conn()
            .execute(
                "UPDATE operations SET
                    state = ?1, priority = ?2, progress = ?3, error = ?4,
                    metadata = ?5, updated_at = ?6, started_at = ?7, completed_at = ?8
                 WHERE operation_id = ?9",
                turso::params![
                    operation.state.to_string(),
                    operation.priority.to_string(),
                    progress_json,
                    error_json,
                    operation
                        .metadata
                        .as_ref()
                        .map(std::string::ToString::to_string),
                    now,
                    operation.started_at.map(|dt| dt.to_rfc3339()),
                    operation.completed_at.map(|dt| dt.to_rfc3339()),
                    operation.operation_id.to_string()
                ],
            )
            .await?;

        debug!(
            operation_id = %operation.operation_id,
            state = %operation.state,
            "operation updated"
        );
        Ok(())
    }

    /// List operations for a product.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn list_for_product(&self, product_code: &str) -> AgentResult<Vec<Operation>> {
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT operation_id, product_code, operation_type, state, priority,
                        parameters, metadata, progress, error,
                        created_at, updated_at, started_at, completed_at
                 FROM operations
                 WHERE product_code = ?1
                 ORDER BY created_at DESC",
                turso::params![product_code],
            )
            .await?;

        let mut ops = Vec::new();
        while let Some(row) = rows.next().await? {
            ops.push(row_to_operation(&row)?);
        }
        Ok(ops)
    }

    /// Delete old completed operations (retention policy).
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn cleanup_old(&self, retention_days: i64) -> AgentResult<u64> {
        let cutoff = (Utc::now() - chrono::Duration::days(retention_days)).to_rfc3339();
        let rows = self
            .db
            .conn()
            .execute(
                "DELETE FROM operations
                 WHERE state IN ('complete', 'failed', 'cancelled')
                   AND completed_at < ?1",
                turso::params![cutoff],
            )
            .await?;

        debug!(deleted = rows, retention_days, "cleaned up old operations");
        Ok(rows)
    }
}

fn row_to_operation(row: &turso::Row) -> AgentResult<Operation> {
    let operation_id_str: String = row.get(0)?;
    let product_code: String = row.get(1)?;
    let operation_type_str: String = row.get(2)?;
    let state_str: String = row.get(3)?;
    let priority_str: String = row.get(4)?;
    let parameters_json: Option<String> = row.get(5)?;
    let metadata_json: Option<String> = row.get(6)?;
    let progress_json: Option<String> = row.get(7)?;
    let error_json: Option<String> = row.get(8)?;
    let created_at_str: String = row.get(9)?;
    let updated_at_str: String = row.get(10)?;
    let started_at_str: Option<String> = row.get(11)?;
    let completed_at_str: Option<String> = row.get(12)?;

    let operation_id = uuid::Uuid::parse_str(&operation_id_str)
        .map_err(|e| AgentError::Schema(format!("invalid UUID: {e}")))?;
    let operation_type = parse_operation_type(&operation_type_str)?;
    let state = parse_operation_state(&state_str)?;
    let priority = parse_priority(&priority_str)?;

    let parameters = parameters_json
        .map(|s| serde_json::from_str(&s))
        .transpose()?;
    let metadata = metadata_json
        .map(|s| serde_json::from_str(&s))
        .transpose()?;
    let progress: Option<Progress> = progress_json
        .map(|s| serde_json::from_str(&s))
        .transpose()?;
    let error: Option<ErrorInfo> = error_json.map(|s| serde_json::from_str(&s)).transpose()?;

    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
    let started_at = started_at_str
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));
    let completed_at = completed_at_str
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    Ok(Operation {
        operation_id,
        product_code,
        operation_type,
        state,
        priority,
        parameters,
        metadata,
        progress,
        error,
        created_at,
        updated_at,
        started_at,
        completed_at,
    })
}

fn parse_operation_type(s: &str) -> AgentResult<OperationType> {
    match s {
        "install" => Ok(OperationType::Install),
        "update" => Ok(OperationType::Update),
        "repair" => Ok(OperationType::Repair),
        "verify" => Ok(OperationType::Verify),
        "uninstall" => Ok(OperationType::Uninstall),
        "backfill" => Ok(OperationType::Backfill),
        _ => Err(AgentError::Schema(format!("unknown operation type: {s}"))),
    }
}

fn parse_operation_state(s: &str) -> AgentResult<OperationState> {
    match s {
        "queued" => Ok(OperationState::Queued),
        "initializing" => Ok(OperationState::Initializing),
        "downloading" => Ok(OperationState::Downloading),
        "verifying" => Ok(OperationState::Verifying),
        "complete" => Ok(OperationState::Complete),
        "failed" => Ok(OperationState::Failed),
        "cancelled" => Ok(OperationState::Cancelled),
        _ => Err(AgentError::Schema(format!("unknown operation state: {s}"))),
    }
}

fn parse_priority(s: &str) -> AgentResult<Priority> {
    match s {
        "low" => Ok(Priority::Low),
        "normal" => Ok(Priority::Normal),
        "high" => Ok(Priority::High),
        _ => Err(AgentError::Schema(format!("unknown priority: {s}"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::product::Product;
    use crate::state::registry::ProductRegistry;

    async fn test_queue() -> (OperationQueue, ProductRegistry) {
        let db = Database::open_memory().await.unwrap();
        let registry = ProductRegistry::new(db.clone());
        let queue = OperationQueue::new(db);

        // Insert a test product (required by foreign key)
        registry
            .insert(&Product::new("wow".to_string(), "WoW".to_string()))
            .await
            .unwrap();

        (queue, registry)
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let (queue, _) = test_queue().await;
        let op = Operation::new(
            "wow".to_string(),
            OperationType::Install,
            Priority::Normal,
            None,
        );
        let id = op.operation_id.to_string();
        queue.insert(&op).await.unwrap();

        let fetched = queue.get(&id).await.unwrap();
        assert_eq!(fetched.product_code, "wow");
        assert_eq!(fetched.operation_type, OperationType::Install);
        assert_eq!(fetched.state, OperationState::Queued);
    }

    #[tokio::test]
    async fn test_duplicate_active_rejected() {
        let (queue, _) = test_queue().await;
        let op1 = Operation::new(
            "wow".to_string(),
            OperationType::Install,
            Priority::Normal,
            None,
        );
        queue.insert(&op1).await.unwrap();

        let op2 = Operation::new(
            "wow".to_string(),
            OperationType::Update,
            Priority::Normal,
            None,
        );
        let result = queue.insert(&op2).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_queued() {
        let (queue, _) = test_queue().await;
        let op = Operation::new(
            "wow".to_string(),
            OperationType::Install,
            Priority::Normal,
            None,
        );
        queue.insert(&op).await.unwrap();

        let queued = queue.get_queued().await.unwrap();
        assert_eq!(queued.len(), 1);
    }

    #[tokio::test]
    async fn test_find_active_for_product() {
        let (queue, _) = test_queue().await;
        assert!(
            queue
                .find_active_for_product("wow")
                .await
                .unwrap()
                .is_none()
        );

        let op = Operation::new(
            "wow".to_string(),
            OperationType::Install,
            Priority::Normal,
            None,
        );
        queue.insert(&op).await.unwrap();

        let active = queue.find_active_for_product("wow").await.unwrap();
        assert!(active.is_some());
    }

    #[tokio::test]
    async fn test_update_state() {
        let (queue, _) = test_queue().await;
        let mut op = Operation::new(
            "wow".to_string(),
            OperationType::Install,
            Priority::Normal,
            None,
        );
        let id = op.operation_id.to_string();
        queue.insert(&op).await.unwrap();

        op.transition_to(OperationState::Initializing).unwrap();
        queue.update(&op).await.unwrap();

        let fetched = queue.get(&id).await.unwrap();
        assert_eq!(fetched.state, OperationState::Initializing);
    }
}
