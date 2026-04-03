//! Adaptive CDN endpoint scoring.
//!
//! Tracks per-host failure weights and reorders endpoints so that
//! healthier servers are tried first. Matches agent.exe behavior:
//! score = base * 0.9^failure_weight, where failure_weight accumulates
//! based on HTTP error category.

use std::collections::HashMap;

use cascette_protocol::{CdnEndpoint, ProtocolError};

use crate::error::{InstallationError, InstallationResult};

/// Endpoints with a score below this threshold are filtered out.
/// Approximately 44 weight units (e.g., 44 network timeouts or 9 server errors)
/// are needed to drop below this from a starting score of 1.0.
const MIN_SCORE_THRESHOLD: f64 = 0.01;

/// Adaptive endpoint scorer that reorders endpoints based on failure history.
///
/// Each host accumulates failure weight from errors. Endpoints are sorted
/// by `0.9^weight` (descending) so that healthier servers are tried first.
/// Endpoints scoring below `MIN_SCORE_THRESHOLD` are filtered out; when
/// all endpoints are exhausted, `sort_endpoints` returns
/// `Err(AllEndpointsExhausted)`.
#[derive(Debug)]
pub struct EndpointScorer {
    /// Accumulated failure weight per host.
    weights: HashMap<String, f64>,
}

impl EndpointScorer {
    /// Create a new scorer with no history.
    pub fn new() -> Self {
        Self {
            weights: HashMap::new(),
        }
    }

    /// Record a successful request to a host.
    ///
    /// Does not reset weight (matching agent.exe — failures accumulate
    /// permanently within a session). Success just means no additional penalty.
    pub fn record_success(&self, _host: &str) {
        // Agent.exe does not reduce failure weight on success within a session.
        // The weight only resets on restart.
    }

    /// Record a failed request and accumulate failure weight.
    pub fn record_failure(&mut self, host: &str, error: &InstallationError) {
        let weight = failure_weight(error);
        if weight > 0.0 {
            *self.weights.entry(host.to_string()).or_insert(0.0) += weight;
        }
    }

    /// Record a data corruption event (hash mismatch after download).
    ///
    /// Applies a weight of 3.0, triple a normal HTTP failure, because
    /// serving corrupted data is a stronger signal of endpoint problems
    /// than a transient connection error.
    pub fn record_data_corruption(&mut self, host: &str) {
        *self.weights.entry(host.to_string()).or_insert(0.0) += 3.0;
    }

    /// Get the score for a host. Higher is better.
    ///
    /// Score = 0.9^failure_weight. A host with no failures scores 1.0.
    pub fn score(&self, host: &str) -> f64 {
        let weight = self.weights.get(host).copied().unwrap_or(0.0);
        0.9_f64.powf(weight)
    }

    /// Sort endpoints by score (highest first), filtering out exhausted hosts.
    ///
    /// Endpoints with scores below `MIN_SCORE_THRESHOLD` are removed.
    /// Returns `Err(AllEndpointsExhausted)` when no viable endpoints remain.
    ///
    /// Stable sort preserves original ordering for hosts with equal scores,
    /// which keeps the MirrorConfig priority (community-first for historic builds).
    pub fn sort_endpoints<'a>(
        &self,
        endpoints: &'a [CdnEndpoint],
    ) -> InstallationResult<Vec<&'a CdnEndpoint>> {
        let mut sorted: Vec<(&CdnEndpoint, f64)> = endpoints
            .iter()
            .map(|e| (e, self.score(&e.host)))
            .filter(|(_, score)| *score >= MIN_SCORE_THRESHOLD)
            .collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        if sorted.is_empty() {
            return Err(InstallationError::AllEndpointsExhausted);
        }
        Ok(sorted.into_iter().map(|(e, _)| e).collect())
    }

    /// Check if any host has accumulated failure weight.
    pub fn has_failures(&self) -> bool {
        self.weights.values().any(|&w| w > 0.0)
    }
}

impl Default for EndpointScorer {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute failure weight for an installation error.
///
/// Weights are based on observed CDN endpoint backoff behavior:
///
/// | Error | Weight |
/// |-------|--------|
/// | HTTP 500, 502, 503, 504 | 5.0 |
/// | HTTP 401, 416 | 2.5 |
/// | HTTP 404 | 2.5 |
/// | Other 4xx, 3xx, 1xx | 0.5 |
/// | Other 5xx | 1.0 |
/// | HTTP 429 (rate limited) | 0.0 (handled by Retry-After) |
/// | Network / timeout | 1.0 |
/// | Non-HTTP errors | 0.0 |
fn failure_weight(error: &InstallationError) -> f64 {
    match error {
        InstallationError::Protocol(pe) => protocol_error_weight(pe),
        InstallationError::Cdn(_) => 1.0,
        _ => 0.0,
    }
}

#[allow(clippy::match_same_arms)] // Arms kept separate to document agent.exe error categories
fn protocol_error_weight(error: &ProtocolError) -> f64 {
    match error {
        // 429 is handled by RetryPolicy with Retry-After — no scoring penalty
        ProtocolError::RateLimited { .. } => 0.0,

        // 5xx server errors
        ProtocolError::ServerError(status) => {
            let code = status.as_u16();
            match code {
                500 | 502 | 503 | 504 => 5.0,
                _ => 1.0,
            }
        }
        ProtocolError::ServiceUnavailable => 5.0,

        // Classified HTTP status codes
        ProtocolError::HttpStatus(status) => {
            let code = status.as_u16();
            match code {
                429 => 0.0,
                500 | 502 | 503 | 504 => 5.0,
                505..=599 => 1.0,
                401 | 404 | 416 => 2.5,
                100..=199 | 300..=399 | 400..=499 => 0.5,
                _ => 1.0,
            }
        }

        // Range not supported (416)
        ProtocolError::RangeNotSupported => 2.5,

        // Network-level failures
        ProtocolError::Network(_) | ProtocolError::Timeout | ProtocolError::Http(_) => 1.0,

        // Non-server errors (parse, cache, key) — not endpoint-related
        _ => 0.0,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_endpoint(host: &str) -> CdnEndpoint {
        CdnEndpoint {
            host: host.to_string(),
            path: "tpr/wow".to_string(),
            product_path: None,
            scheme: Some("https".to_string()),
            is_fallback: false,
            strict: false,
            max_hosts: None,
        }
    }

    #[test]
    fn test_new_scorer_has_max_scores() {
        let scorer = EndpointScorer::new();
        assert!((scorer.score("any-host") - 1.0).abs() < f64::EPSILON);
        assert!(!scorer.has_failures());
    }

    #[test]
    fn test_server_error_weight() {
        let mut scorer = EndpointScorer::new();
        // ServiceUnavailable maps to weight 5.0 (same as 503)
        let error = InstallationError::Protocol(ProtocolError::ServiceUnavailable);
        scorer.record_failure("bad-host", &error);

        // 0.9^5.0 ≈ 0.59049
        let score = scorer.score("bad-host");
        assert!((score - 0.9_f64.powi(5)).abs() < 0.001);
        assert!(scorer.has_failures());

        // Good host unaffected
        assert!((scorer.score("good-host") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_failures_accumulate() {
        let mut scorer = EndpointScorer::new();
        let error = InstallationError::Cdn("connection reset".to_string());

        scorer.record_failure("host-a", &error);
        scorer.record_failure("host-a", &error);
        scorer.record_failure("host-a", &error);

        // 3 failures × weight 1.0 = 3.0 total → 0.9^3 = 0.729
        let score = scorer.score("host-a");
        assert!((score - 0.9_f64.powi(3)).abs() < 0.001);
    }

    #[test]
    fn test_sort_endpoints_by_score() {
        let mut scorer = EndpointScorer::new();
        let endpoints = vec![
            make_endpoint("bad-host"),
            make_endpoint("good-host"),
            make_endpoint("ok-host"),
        ];

        // bad-host gets 5.0 weight (ServiceUnavailable = 503)
        scorer.record_failure(
            "bad-host",
            &InstallationError::Protocol(ProtocolError::ServiceUnavailable),
        );
        // ok-host gets 1.0 weight (CDN error)
        scorer.record_failure("ok-host", &InstallationError::Cdn("timeout".to_string()));

        let sorted = scorer.sort_endpoints(&endpoints).unwrap();
        assert_eq!(sorted[0].host, "good-host"); // score 1.0
        assert_eq!(sorted[1].host, "ok-host"); // score 0.9
        assert_eq!(sorted[2].host, "bad-host"); // score ~0.59
    }

    #[test]
    fn test_rate_limited_no_penalty() {
        let mut scorer = EndpointScorer::new();
        let error = InstallationError::Protocol(ProtocolError::RateLimited {
            retry_after: Some(std::time::Duration::from_secs(5)),
        });
        scorer.record_failure("host", &error);

        // 429 has 0.0 weight — no penalty
        assert!((scorer.score("host") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stable_sort_preserves_order() {
        let scorer = EndpointScorer::new();
        let endpoints = vec![
            make_endpoint("mirror-1"),
            make_endpoint("mirror-2"),
            make_endpoint("official"),
        ];

        // No failures — all scores equal, original order preserved
        let sorted = scorer.sort_endpoints(&endpoints).unwrap();
        assert_eq!(sorted[0].host, "mirror-1");
        assert_eq!(sorted[1].host, "mirror-2");
        assert_eq!(sorted[2].host, "official");
    }

    #[test]
    fn test_range_not_supported_weight() {
        let mut scorer = EndpointScorer::new();
        let error = InstallationError::Protocol(ProtocolError::RangeNotSupported);
        scorer.record_failure("host", &error);

        // 416 → weight 2.5, score = 0.9^2.5 ≈ 0.7686
        let score = scorer.score("host");
        assert!((score - 0.9_f64.powf(2.5)).abs() < 0.001);
    }

    #[test]
    fn test_timeout_weight() {
        let mut scorer = EndpointScorer::new();
        let error = InstallationError::Protocol(ProtocolError::Timeout);
        scorer.record_failure("host", &error);

        // Timeout → weight 1.0, score = 0.9
        let score = scorer.score("host");
        assert!((score - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_data_corruption_penalizes_harder_than_normal_failures() {
        let mut scorer = EndpointScorer::new();

        // Host A: data corruption (weight 3.0)
        scorer.record_data_corruption("host-a");

        // Host B: two normal CDN failures (weight 1.0 each = 2.0 total)
        let cdn_err = InstallationError::Cdn("timeout".to_string());
        scorer.record_failure("host-b", &cdn_err);
        scorer.record_failure("host-b", &cdn_err);

        // Host A (0.9^3.0 ≈ 0.729) should score lower than host B (0.9^2.0 = 0.81)
        assert!(scorer.score("host-a") < scorer.score("host-b"));

        // Verify sort_endpoints pushes corrupted host behind the 2-failure host
        let endpoints = vec![make_endpoint("host-a"), make_endpoint("host-b")];
        let sorted = scorer.sort_endpoints(&endpoints).unwrap();
        assert_eq!(sorted[0].host, "host-b");
        assert_eq!(sorted[1].host, "host-a");
    }

    #[test]
    fn test_non_server_errors_no_penalty() {
        let mut scorer = EndpointScorer::new();
        // Parse errors are not endpoint-related
        let error = InstallationError::Protocol(ProtocolError::Parse("bad data".to_string()));
        scorer.record_failure("host", &error);
        assert!((scorer.score("host") - 1.0).abs() < f64::EPSILON);

        // Format errors are not endpoint-related
        let error = InstallationError::Format("corrupt".to_string());
        scorer.record_failure("host", &error);
        assert!((scorer.score("host") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_all_endpoints_exhausted() {
        let mut scorer = EndpointScorer::new();
        let endpoints = vec![make_endpoint("host-a"), make_endpoint("host-b")];

        // Drive both hosts below MIN_SCORE_THRESHOLD (0.01).
        // 0.9^50 ≈ 0.0052, well below 0.01.
        for _ in 0..50 {
            scorer.record_failure("host-a", &InstallationError::Cdn("fail".to_string()));
            scorer.record_failure("host-b", &InstallationError::Cdn("fail".to_string()));
        }

        let result = scorer.sort_endpoints(&endpoints);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            InstallationError::AllEndpointsExhausted
        ));
    }
}
