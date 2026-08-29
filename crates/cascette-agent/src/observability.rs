//! Observability setup: tracing and Prometheus metrics.

use prometheus::{
    IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry, register_int_counter_vec_with_registry,
    register_int_gauge_vec_with_registry, register_int_gauge_with_registry,
};

/// Prometheus metrics for the agent service.
#[derive(Debug, Clone)]
pub struct Metrics {
    /// Prometheus registry.
    pub registry: Registry,

    /// Number of HTTP requests by method and path.
    pub http_requests_total: IntCounterVec,

    /// Number of active operations by type.
    pub active_operations: IntGaugeVec,

    /// Total operations completed by type and result.
    pub operations_total: IntCounterVec,

    /// Number of registered products.
    pub products_registered: IntGauge,

    /// Number of active game sessions.
    pub active_game_sessions: IntGauge,
}

impl Metrics {
    /// Create a new metrics instance with a fresh registry.
    ///
    /// # Errors
    ///
    /// Returns an error if metric registration fails (duplicate names).
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        let http_requests_total = register_int_counter_vec_with_registry!(
            Opts::new(
                "cascette_agent_http_requests_total",
                "Total HTTP requests by method and path"
            ),
            &["method", "path", "status"],
            registry
        )?;

        let active_operations = register_int_gauge_vec_with_registry!(
            Opts::new(
                "cascette_agent_active_operations",
                "Number of active operations by type"
            ),
            &["operation_type"],
            registry
        )?;

        let operations_total = register_int_counter_vec_with_registry!(
            Opts::new(
                "cascette_agent_operations_total",
                "Total operations completed by type and result"
            ),
            &["operation_type", "result"],
            registry
        )?;

        let products_registered = register_int_gauge_with_registry!(
            Opts::new(
                "cascette_agent_products_registered",
                "Number of registered products"
            ),
            registry
        )?;

        let active_game_sessions = register_int_gauge_with_registry!(
            Opts::new(
                "cascette_agent_active_game_sessions",
                "Number of active game sessions"
            ),
            registry
        )?;

        Ok(Self {
            registry,
            http_requests_total,
            active_operations,
            operations_total,
            products_registered,
            active_game_sessions,
        })
    }
}

/// Initialize the tracing subscriber with the given log filter.
///
/// # Errors
///
/// Returns an error if the subscriber cannot be initialized (e.g., already set).
pub fn init_tracing(filter: &str) -> Result<(), String> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .try_init()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let metrics = Metrics::new().unwrap();
        metrics
            .http_requests_total
            .with_label_values(&["GET", "/health", "200"])
            .inc();
        assert_eq!(
            metrics
                .http_requests_total
                .with_label_values(&["GET", "/health", "200"])
                .get(),
            1
        );
    }

    #[test]
    fn test_metrics_operations() {
        let metrics = Metrics::new().unwrap();
        metrics
            .active_operations
            .with_label_values(&["install"])
            .set(2);
        assert_eq!(
            metrics
                .active_operations
                .with_label_values(&["install"])
                .get(),
            2
        );
    }
}
