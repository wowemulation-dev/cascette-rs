//! Retry policy implementation with exponential backoff

use rand::{Rng, rng};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::time::Duration;

use crate::error::Result;

/// Cross-platform async sleep function
///
/// On native platforms, uses tokio::time::sleep.
/// On WASM, uses gloo_timers::future::TimeoutFuture.
#[cfg(not(target_arch = "wasm32"))]
async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(target_arch = "wasm32")]
async fn sleep(duration: Duration) {
    gloo_timers::future::TimeoutFuture::new(duration.as_millis() as u32).await;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    pub max_attempts: u32,

    /// Initial backoff duration
    pub initial_backoff: Duration,

    /// Maximum backoff duration
    pub max_backoff: Duration,

    /// Backoff multiplier
    pub multiplier: f64,

    /// Add jitter to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Create retry policy from environment variables
    ///
    /// Note: On WASM, environment variables are not available, so this
    /// will always return default values.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            max_attempts: std::env::var("CASCETTE_MAX_RETRIES")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3),
            initial_backoff: Duration::from_millis(
                std::env::var("CASCETTE_RETRY_BACKOFF")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(100),
            ),
            max_backoff: Duration::from_secs(
                std::env::var("CASCETTE_MAX_BACKOFF")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
            ),
            multiplier: std::env::var("CASCETTE_BACKOFF_MULTIPLIER")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2.0),
            jitter: std::env::var("CASCETTE_RETRY_JITTER")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(true),
        })
    }

    /// Create retry policy from environment variables (WASM version)
    ///
    /// On WASM, environment variables are not available, so this
    /// always returns default values.
    #[cfg(target_arch = "wasm32")]
    pub fn from_env() -> Result<Self> {
        Ok(Self::default())
    }

    /// Execute a function with retry logic
    pub async fn execute<F, Fut, T>(&self, mut f: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut attempt = 0;
        let mut backoff = self.initial_backoff;

        loop {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) if !e.should_retry() || attempt >= self.max_attempts => {
                    return Err(e);
                }
                Err(e) => {
                    attempt += 1;
                    tracing::warn!("Attempt {} failed: {}", attempt, e);

                    // Calculate delay with jitter
                    let mut delay = backoff;
                    if self.jitter {
                        let jitter = rng().random_range(0.0..0.3);
                        #[allow(clippy::cast_precision_loss)]
                        // Precision loss is acceptable for jitter calculation
                        let jitter_ms = (delay.as_millis() as f64 * jitter) as u64;
                        delay += Duration::from_millis(jitter_ms);
                    }

                    sleep(delay).await;

                    // Increase backoff
                    backoff = Duration::from_secs_f64(
                        (backoff.as_secs_f64() * self.multiplier)
                            .min(self.max_backoff.as_secs_f64()),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
#[allow(
    unsafe_code,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args,
    clippy::significant_drop_tightening
)]
mod tests {
    use super::*;
    use crate::error::ProtocolError;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.initial_backoff, Duration::from_millis(100));
        assert_eq!(policy.max_backoff, Duration::from_secs(10));
        assert!((policy.multiplier - 2.0).abs() < f64::EPSILON);
        assert!(policy.jitter);
    }

    #[test]
    fn test_from_env_default_values() {
        // Clear any existing env vars
        for var in [
            "CASCETTE_MAX_RETRIES",
            "CASCETTE_RETRY_BACKOFF",
            "CASCETTE_MAX_BACKOFF",
            "CASCETTE_BACKOFF_MULTIPLIER",
            "CASCETTE_RETRY_JITTER",
        ] {
            unsafe {
                std::env::remove_var(var);
            }
        }

        let policy = RetryPolicy::from_env().expect("Operation should succeed");
        assert_eq!(policy.max_attempts, 3);
        assert_eq!(policy.initial_backoff, Duration::from_millis(100));
        assert_eq!(policy.max_backoff, Duration::from_secs(10));
        assert!((policy.multiplier - 2.0).abs() < f64::EPSILON);
        assert!(policy.jitter);
    }

    #[test]
    fn test_from_env_custom_values() {
        unsafe {
            std::env::set_var("CASCETTE_MAX_RETRIES", "5");
            std::env::set_var("CASCETTE_RETRY_BACKOFF", "200");
            std::env::set_var("CASCETTE_MAX_BACKOFF", "20");
            std::env::set_var("CASCETTE_BACKOFF_MULTIPLIER", "1.5");
            std::env::set_var("CASCETTE_RETRY_JITTER", "false");
        }

        let policy = RetryPolicy::from_env().expect("Operation should succeed");
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.initial_backoff, Duration::from_millis(200));
        assert_eq!(policy.max_backoff, Duration::from_secs(20));
        assert!((policy.multiplier - 1.5).abs() < f64::EPSILON);
        assert!(!policy.jitter);

        // Clean up
        for var in [
            "CASCETTE_MAX_RETRIES",
            "CASCETTE_RETRY_BACKOFF",
            "CASCETTE_MAX_BACKOFF",
            "CASCETTE_BACKOFF_MULTIPLIER",
            "CASCETTE_RETRY_JITTER",
        ] {
            unsafe {
                std::env::remove_var(var);
            }
        }
    }

    #[tokio::test]
    async fn test_execute_success_on_first_try() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let call_count = Arc::new(Mutex::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let result = policy
            .execute(|| async {
                let mut count = call_count_clone.lock().expect("Operation should succeed");
                *count += 1;
                Ok::<i32, ProtocolError>(42)
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.expect("Operation should succeed"), 42);
        assert_eq!(*call_count.lock().expect("Operation should succeed"), 1);
    }

    #[tokio::test]
    async fn test_execute_retry_on_retryable_error() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let call_count = Arc::new(Mutex::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let start = Instant::now();
        let result = policy
            .execute(|| async {
                let mut count = call_count_clone.lock().expect("Operation should succeed");
                *count += 1;
                if *count < 3 {
                    Err(ProtocolError::Timeout)
                } else {
                    Ok::<i32, ProtocolError>(42)
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.expect("Operation should succeed"), 42);
        assert_eq!(*call_count.lock().expect("Operation should succeed"), 3);
        // Should have taken at least 1ms + 2ms for backoff
        assert!(start.elapsed() >= Duration::from_millis(2));
    }

    #[tokio::test]
    async fn test_execute_fail_on_non_retryable_error() {
        let policy = RetryPolicy {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let call_count = Arc::new(Mutex::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let result = policy
            .execute(|| async {
                let mut count = call_count_clone.lock().expect("Operation should succeed");
                *count += 1;
                Err::<i32, ProtocolError>(ProtocolError::Parse("invalid".to_string()))
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::Parse(_)
        ));
        assert_eq!(*call_count.lock().expect("Operation should succeed"), 1);
    }

    #[tokio::test]
    async fn test_execute_exceed_max_attempts() {
        let policy = RetryPolicy {
            max_attempts: 2,
            initial_backoff: Duration::from_millis(1),
            max_backoff: Duration::from_secs(1),
            multiplier: 2.0,
            jitter: false,
        };

        let call_count = Arc::new(Mutex::new(0));
        let call_count_clone = Arc::clone(&call_count);

        let result = policy
            .execute(|| async {
                let mut count = call_count_clone.lock().expect("Operation should succeed");
                *count += 1;
                Err::<i32, ProtocolError>(ProtocolError::Timeout)
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.expect_err("Test operation should fail"),
            ProtocolError::Timeout
        ));
        // Should have called initial + max_attempts times
        assert_eq!(*call_count.lock().expect("Operation should succeed"), 3);
    }

    #[tokio::test]
    async fn test_backoff_progression() {
        let policy = RetryPolicy {
            max_attempts: 4,
            initial_backoff: Duration::from_millis(10),
            max_backoff: Duration::from_millis(50),
            multiplier: 2.0,
            jitter: false,
        };

        let call_count = Arc::new(Mutex::new(0));
        let call_count_clone = Arc::clone(&call_count);
        let start = Instant::now();

        let _result = policy
            .execute(|| async {
                let mut count = call_count_clone.lock().expect("Operation should succeed");
                *count += 1;
                Err::<i32, ProtocolError>(ProtocolError::Timeout)
            })
            .await;

        // Should have taken at least: 10ms + 20ms + 40ms (capped at 50ms)
        // But less than if all were max: 10ms + 20ms + 50ms
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(70)); // 10 + 20 + 40
        assert!(elapsed < Duration::from_millis(150)); // Conservative upper bound
        assert_eq!(*call_count.lock().expect("Operation should succeed"), 5); // initial + 4 retries
    }
}
