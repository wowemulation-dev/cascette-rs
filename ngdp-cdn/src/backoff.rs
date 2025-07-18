use std::time::Duration;

/// Default initial backoff in milliseconds
const DEFAULT_INITIAL_BACKOFF_MS: u64 = 100;

/// Default maximum backoff in milliseconds
const DEFAULT_MAX_BACKOFF_MS: u64 = 10_000;

/// Default backoff multiplier
const DEFAULT_BACKOFF_MULTIPLIER: f64 = 2.0;

/// Default jitter factor (0.0 to 1.0)
const DEFAULT_JITTER_FACTOR: f64 = 0.1;

/// Retry backoff calculation.
#[derive(Debug, Clone)]
pub struct Backoff {
    /// Initial backoff duration in milliseconds
    pub(crate) initial_backoff_ms: u64,
    /// Maximum backoff duration in milliseconds
    pub(crate) max_backoff_ms: u64,
    /// Backoff multiplier
    pub(crate) backoff_multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    pub(crate) jitter_factor: f64,
}

impl Backoff {
    /// Create new backoff calculator with defaults.
    pub fn new() -> Self {
        Self {
            initial_backoff_ms: DEFAULT_INITIAL_BACKOFF_MS,
            max_backoff_ms: DEFAULT_MAX_BACKOFF_MS,
            backoff_multiplier: DEFAULT_BACKOFF_MULTIPLIER,
            jitter_factor: DEFAULT_JITTER_FACTOR,
        }
    }

    /// Calculate backoff duration with exponential backoff and jitter
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_backoff =
            self.initial_backoff_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped_backoff = base_backoff.min(self.max_backoff_ms as f64);

        // Add jitter
        let jitter_range = capped_backoff * self.jitter_factor;
        let jitter = rand::random::<f64>() * 2.0 * jitter_range - jitter_range;
        let final_backoff = (capped_backoff + jitter).max(0.0) as u64;

        Duration::from_millis(final_backoff)
    }

    /// Set initial backoff in milliseconds
    pub fn set_initial_backoff_ms(&mut self, ms: u64) -> &mut Self {
        self.initial_backoff_ms = ms;
        self
    }

    /// Set maximum backoff in milliseconds
    pub fn set_max_backoff_ms(&mut self, ms: u64) -> &mut Self {
        self.max_backoff_ms = ms;
        self
    }

    /// Set backoff multiplier
    pub fn set_backoff_multiplier(&mut self, multiplier: f64) -> &mut Self {
        self.backoff_multiplier = multiplier;
        self
    }

    /// Set jitter factor (0.0 to 1.0)
    pub fn set_jitter_factor(&mut self, factor: f64) -> &mut Self {
        self.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_defaults() {
        let backoff = Backoff::new();
        assert_eq!(backoff.initial_backoff_ms, DEFAULT_INITIAL_BACKOFF_MS);
        assert_eq!(backoff.max_backoff_ms, DEFAULT_MAX_BACKOFF_MS);
    }

    #[test]
    fn test_jitter_factor_clamping() {
        let mut client1 = Backoff::new();
        client1.set_jitter_factor(1.5);
        assert!((client1.jitter_factor - 1.0).abs() < f64::EPSILON);

        let mut client2 = Backoff::new();
        client2.set_jitter_factor(-0.5);
        assert!((client2.jitter_factor - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_backoff_calculation() {
        let mut backoff = Backoff::new();
        backoff
            .set_initial_backoff_ms(100)
            .set_max_backoff_ms(1000)
            .set_backoff_multiplier(2.0)
            .set_jitter_factor(0.0); // No jitter for predictable test

        // Test exponential backoff
        let backoff0 = backoff.calculate_backoff(0);
        assert_eq!(backoff0.as_millis(), 100); // 100ms * 2^0 = 100ms

        let backoff1 = backoff.calculate_backoff(1);
        assert_eq!(backoff1.as_millis(), 200); // 100ms * 2^1 = 200ms

        let backoff2 = backoff.calculate_backoff(2);
        assert_eq!(backoff2.as_millis(), 400); // 100ms * 2^2 = 400ms

        // Test max backoff capping
        let backoff5 = backoff.calculate_backoff(5);
        assert_eq!(backoff5.as_millis(), 1000); // Would be 3200ms but capped at 1000ms
    }
}
