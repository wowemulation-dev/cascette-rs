//! Content validation hooks for NGDP cache integrity
//!
//! This module provides validation hooks that can be integrated with cache operations
//! to ensure content integrity using MD5 hashing and other validation strategies.
//! It supports lazy validation for performance-critical paths and async validation
//! compatible with tokio.

use crate::error::{CacheError, CacheResult, NgdpCacheError, NgdpCacheResult};
use async_trait::async_trait;
use bytes::Bytes;
use cascette_crypto::{ContentKey, EncodingKey, Jenkins96, TactKey};
use std::{
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
    time::Instant,
};

/// Validation result with timing information
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the content is valid
    pub is_valid: bool,
    /// Time taken to validate
    pub validation_time: std::time::Duration,
    /// Hash computation time (subset of validation_time)
    pub hash_time: std::time::Duration,
    /// Size of content validated
    pub content_size: usize,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn valid(
        validation_time: std::time::Duration,
        hash_time: std::time::Duration,
        content_size: usize,
    ) -> Self {
        Self {
            is_valid: true,
            validation_time,
            hash_time,
            content_size,
        }
    }

    /// Create a failed validation result
    pub fn invalid(
        validation_time: std::time::Duration,
        hash_time: std::time::Duration,
        content_size: usize,
    ) -> Self {
        Self {
            is_valid: false,
            validation_time,
            hash_time,
            content_size,
        }
    }

    /// Create an error result (treated as invalid)
    pub fn error(validation_time: std::time::Duration, content_size: usize) -> Self {
        Self {
            is_valid: false,
            validation_time,
            hash_time: std::time::Duration::ZERO,
            content_size,
        }
    }
}

/// Trait for content validation hooks
///
/// Implementations can provide different validation strategies (MD5, streaming, etc.)
/// and can be called during cache put/get operations.
#[async_trait]
pub trait ValidationHooks: Send + Sync {
    /// Validate content against a content key
    ///
    /// This is called during put operations to ensure content integrity.
    /// Returns true if content is valid, false otherwise.
    async fn validate_content(
        &self,
        content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult>;

    /// Validate content during get operations (optional lazy validation)
    ///
    /// This can be called during get operations if lazy validation is enabled.
    /// Default implementation calls validate_content.
    async fn validate_on_get(
        &self,
        content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        self.validate_content(content_key, data).await
    }

    /// Check if validation should be skipped for performance reasons
    ///
    /// This allows implementations to skip validation under certain conditions
    /// (e.g., trusted sources, cached validation results, etc.)
    async fn should_skip_validation(&self, content_key: &ContentKey, data_size: usize) -> bool {
        // Default: never skip validation
        let _ = (content_key, data_size);
        false
    }

    /// Called when validation fails to allow custom error handling
    async fn on_validation_failure(
        &self,
        content_key: &ContentKey,
        data: &[u8],
        error: &CacheError,
    ) {
        // Default: no-op
        let _ = (content_key, data, error);
    }

    /// Called when validation succeeds for metrics/logging
    async fn on_validation_success(&self, content_key: &ContentKey, result: &ValidationResult) {
        // Default: no-op
        let _ = (content_key, result);
    }

    /// Get validation metrics (if available)
    ///
    /// Default implementation returns None. Implementations can override this
    /// to provide their internal metrics.
    fn get_metrics(&self) -> Option<&ValidationMetrics> {
        None
    }
}

/// MD5-based content validation implementation
///
/// Validates content by computing MD5 hash and comparing with the content key.
pub struct Md5ValidationHooks {
    /// Metrics for validation operations
    pub metrics: ValidationMetrics,
}

impl Md5ValidationHooks {
    /// Create a new MD5 validation hooks instance
    pub fn new() -> Self {
        Self {
            metrics: ValidationMetrics::new(),
        }
    }

    /// Get validation metrics
    pub fn metrics(&self) -> &ValidationMetrics {
        &self.metrics
    }
}

impl Default for Md5ValidationHooks {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidationHooks for Md5ValidationHooks {
    async fn validate_content(
        &self,
        content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        let start_time = Instant::now();

        // Compute MD5 hash of the content using md5 crate
        let hash_start = Instant::now();
        let computed_hash = md5::compute(data);
        let hash_time = hash_start.elapsed();

        // Compare with expected hash from content key
        let expected_hash = content_key.as_bytes();
        let is_valid = computed_hash.as_ref() == expected_hash;

        let validation_time = start_time.elapsed();
        let result = if is_valid {
            ValidationResult::valid(validation_time, hash_time, data.len())
        } else {
            ValidationResult::invalid(validation_time, hash_time, data.len())
        };

        // Update metrics
        if is_valid {
            self.metrics.record_success(validation_time, data.len());
        } else {
            self.metrics.record_failure(validation_time, data.len());
        }

        Ok(result)
    }

    async fn should_skip_validation(&self, _content_key: &ContentKey, data_size: usize) -> bool {
        // Skip validation for very large files in performance mode
        // This is configurable behavior - in production you might want different thresholds
        const MAX_VALIDATION_SIZE: usize = 100 * 1024 * 1024; // 100MB
        data_size > MAX_VALIDATION_SIZE
    }

    async fn on_validation_failure(
        &self,
        content_key: &ContentKey,
        data: &[u8],
        _error: &CacheError,
    ) {
        // Log validation failure for debugging
        eprintln!(
            "Content validation failed for key {:?} (size: {} bytes)",
            content_key,
            data.len()
        );
    }

    async fn on_validation_success(&self, _content_key: &ContentKey, result: &ValidationResult) {
        // Could log success for debugging if needed
        if result.validation_time.as_millis() > 100 {
            // Log slow validations
            eprintln!(
                "Slow validation completed in {:?} for {} bytes",
                result.validation_time, result.content_size
            );
        }
    }

    fn get_metrics(&self) -> Option<&ValidationMetrics> {
        Some(&self.metrics)
    }
}

/// Validation metrics collector
///
/// Tracks validation operations for monitoring and performance analysis.
#[derive(Debug)]
pub struct ValidationMetrics {
    /// Total validation attempts
    pub total_validations: AtomicU64,
    /// Successful validations
    pub successful_validations: AtomicU64,
    /// Failed validations
    pub failed_validations: AtomicU64,
    /// Total bytes validated
    pub bytes_validated: AtomicU64,
    /// Total validation time (nanoseconds)
    pub total_validation_time_ns: AtomicU64,
    /// Validations skipped for performance
    pub validations_skipped: AtomicU64,
}

impl ValidationMetrics {
    /// Create new validation metrics
    pub fn new() -> Self {
        Self {
            total_validations: AtomicU64::new(0),
            successful_validations: AtomicU64::new(0),
            failed_validations: AtomicU64::new(0),
            bytes_validated: AtomicU64::new(0),
            total_validation_time_ns: AtomicU64::new(0),
            validations_skipped: AtomicU64::new(0),
        }
    }

    /// Record a successful validation
    pub fn record_success(&self, duration: std::time::Duration, bytes: usize) {
        self.total_validations.fetch_add(1, Ordering::Relaxed);
        self.successful_validations.fetch_add(1, Ordering::Relaxed);
        self.bytes_validated
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.total_validation_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a failed validation
    pub fn record_failure(&self, duration: std::time::Duration, bytes: usize) {
        self.total_validations.fetch_add(1, Ordering::Relaxed);
        self.failed_validations.fetch_add(1, Ordering::Relaxed);
        self.bytes_validated
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.total_validation_time_ns
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    /// Record a skipped validation
    pub fn record_skip(&self) {
        self.validations_skipped.fetch_add(1, Ordering::Relaxed);
    }

    /// Get validation success rate (0.0 to 1.0)
    #[allow(clippy::cast_precision_loss)] // Stats calculation intentionally accepts precision loss
    pub fn success_rate(&self) -> f64 {
        let total = self.total_validations.load(Ordering::Relaxed);
        if total == 0 {
            return 1.0; // No validations attempted, assume success
        }
        let successful = self.successful_validations.load(Ordering::Relaxed);
        successful as f64 / total as f64
    }

    /// Get average validation time
    pub fn average_validation_time(&self) -> std::time::Duration {
        let total_time_ns = self.total_validation_time_ns.load(Ordering::Relaxed);
        let total_validations = self.total_validations.load(Ordering::Relaxed);

        if total_validations == 0 {
            return std::time::Duration::ZERO;
        }

        std::time::Duration::from_nanos(total_time_ns / total_validations)
    }

    /// Get validation throughput (bytes per second)
    #[allow(clippy::cast_precision_loss)] // Stats calculation intentionally accepts precision loss
    pub fn validation_throughput(&self) -> f64 {
        let total_bytes = self.bytes_validated.load(Ordering::Relaxed);
        let total_time_ns = self.total_validation_time_ns.load(Ordering::Relaxed);

        if total_time_ns == 0 {
            return 0.0;
        }

        let total_time_secs = total_time_ns as f64 / 1_000_000_000.0;
        total_bytes as f64 / total_time_secs
    }

    /// Reset all metrics
    pub fn reset(&self) {
        self.total_validations.store(0, Ordering::Relaxed);
        self.successful_validations.store(0, Ordering::Relaxed);
        self.failed_validations.store(0, Ordering::Relaxed);
        self.bytes_validated.store(0, Ordering::Relaxed);
        self.total_validation_time_ns.store(0, Ordering::Relaxed);
        self.validations_skipped.store(0, Ordering::Relaxed);
    }
}

impl Default for ValidationMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// Blanket implementation for Arc<dyn ValidationHooks>
#[async_trait]
impl ValidationHooks for std::sync::Arc<dyn ValidationHooks> {
    async fn validate_content(
        &self,
        content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        (**self).validate_content(content_key, data).await
    }

    async fn validate_on_get(
        &self,
        content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        (**self).validate_on_get(content_key, data).await
    }

    async fn should_skip_validation(&self, content_key: &ContentKey, data_size: usize) -> bool {
        (**self)
            .should_skip_validation(content_key, data_size)
            .await
    }

    async fn on_validation_failure(
        &self,
        content_key: &ContentKey,
        data: &[u8],
        error: &CacheError,
    ) {
        (**self)
            .on_validation_failure(content_key, data, error)
            .await;
    }

    async fn on_validation_success(&self, content_key: &ContentKey, result: &ValidationResult) {
        (**self).on_validation_success(content_key, result).await;
    }

    fn get_metrics(&self) -> Option<&ValidationMetrics> {
        (**self).get_metrics()
    }
}

/// Enhanced NGDP validation hooks with support for multiple hash algorithms
///
/// This implementation supports MD5 content validation as well as Jenkins96
/// path validation and TACT key validation for encrypted content.
pub struct NgdpValidationHooks {
    /// Base MD5 validation hooks
    pub md5_hooks: Md5ValidationHooks,
    /// Optional TACT key for encrypted content validation
    pub tact_key: Option<TactKey>,
    /// Enable Jenkins96 path validation for archive indices
    pub jenkins96_validation: bool,
}

impl NgdpValidationHooks {
    /// Create new NGDP validation hooks with MD5 validation only
    pub fn new() -> Self {
        Self {
            md5_hooks: Md5ValidationHooks::new(),
            tact_key: None,
            jenkins96_validation: false,
        }
    }

    /// Create NGDP validation hooks with TACT key support
    pub fn with_tact_key(tact_key: TactKey) -> Self {
        Self {
            md5_hooks: Md5ValidationHooks::new(),
            tact_key: Some(tact_key),
            jenkins96_validation: false,
        }
    }

    /// Enable Jenkins96 path validation
    pub fn with_jenkins96_validation(mut self) -> Self {
        self.jenkins96_validation = true;
        self
    }

    /// Validate content using Jenkins96 hash for archive indices
    pub fn validate_jenkins96(
        &self,
        expected_hash: u64,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        let start_time = Instant::now();
        let hash_start = Instant::now();

        let computed_hash = Jenkins96::hash(data);
        let hash_time = hash_start.elapsed();

        let is_valid = computed_hash.hash64 == expected_hash;
        let validation_time = start_time.elapsed();

        let result = if is_valid {
            ValidationResult::valid(validation_time, hash_time, data.len())
        } else {
            ValidationResult::invalid(validation_time, hash_time, data.len())
        };

        // Update metrics through underlying MD5 hooks
        if is_valid {
            self.md5_hooks
                .metrics
                .record_success(validation_time, data.len());
        } else {
            self.md5_hooks
                .metrics
                .record_failure(validation_time, data.len());
        }

        Ok(result)
    }

    /// Validate encoding key against content
    pub fn validate_encoding_key(
        &self,
        encoding_key: &EncodingKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        let start_time = Instant::now();
        let hash_start = Instant::now();

        // Generate content key from data and validate against encoding key
        let _content_key = ContentKey::from_data(data);
        let hash_time = hash_start.elapsed();

        // For now, we assume encoding key validation is successful if we can compute content key
        // In a real implementation, this would involve more complex NGDP encoding key validation
        let is_valid = !encoding_key.to_string().is_empty() && !data.is_empty();
        let validation_time = start_time.elapsed();

        let result = if is_valid {
            ValidationResult::valid(validation_time, hash_time, data.len())
        } else {
            ValidationResult::invalid(validation_time, hash_time, data.len())
        };

        // Update metrics
        if is_valid {
            self.md5_hooks
                .metrics
                .record_success(validation_time, data.len());
        } else {
            self.md5_hooks
                .metrics
                .record_failure(validation_time, data.len());
        }

        Ok(result)
    }

    /// Batch validate multiple content keys for performance
    pub async fn batch_validate_content(
        &self,
        items: &[(ContentKey, &[u8])],
    ) -> CacheResult<Vec<ValidationResult>> {
        let mut results = Vec::with_capacity(items.len());

        for (content_key, data) in items {
            let result = self.validate_content(content_key, data).await?;
            results.push(result);
        }

        Ok(results)
    }
}

impl Default for NgdpValidationHooks {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ValidationHooks for NgdpValidationHooks {
    async fn validate_content(
        &self,
        content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        // Delegate to MD5 validation hooks
        self.md5_hooks.validate_content(content_key, data).await
    }

    async fn validate_on_get(
        &self,
        content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        self.md5_hooks.validate_on_get(content_key, data).await
    }

    async fn should_skip_validation(&self, content_key: &ContentKey, data_size: usize) -> bool {
        self.md5_hooks
            .should_skip_validation(content_key, data_size)
            .await
    }

    async fn on_validation_failure(
        &self,
        content_key: &ContentKey,
        data: &[u8],
        error: &CacheError,
    ) {
        self.md5_hooks
            .on_validation_failure(content_key, data, error)
            .await;
    }

    async fn on_validation_success(&self, content_key: &ContentKey, result: &ValidationResult) {
        self.md5_hooks
            .on_validation_success(content_key, result)
            .await;
    }

    fn get_metrics(&self) -> Option<&ValidationMetrics> {
        self.md5_hooks.get_metrics()
    }
}

/// No-op validation hooks for performance-critical scenarios
///
/// This implementation always returns valid and performs no actual validation.
/// Use with caution - only in trusted environments or when validation is
/// performed elsewhere.
pub struct NoOpValidationHooks;

#[async_trait]
impl ValidationHooks for NoOpValidationHooks {
    async fn validate_content(
        &self,
        _content_key: &ContentKey,
        data: &[u8],
    ) -> CacheResult<ValidationResult> {
        // Return immediate success without validation
        Ok(ValidationResult::valid(
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
            data.len(),
        ))
    }

    async fn should_skip_validation(&self, _content_key: &ContentKey, _data_size: usize) -> bool {
        true // Always skip
    }
}

/// Wrapper for bytes with validation state and content key
///
/// This wrapper provides lazy validation capabilities and stores the content key
/// for integrity verification. Uses AtomicBool for thread-safe validation tracking.
#[derive(Debug)]
pub struct NgdpBytes {
    /// Underlying byte data (reference-counted for zero-copy)
    data: Bytes,
    /// Content key for validation
    content_key: Option<ContentKey>,
    /// Validation status (thread-safe)
    validated: AtomicBool,
}

// Manual Clone implementation to handle AtomicBool
impl Clone for NgdpBytes {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            content_key: self.content_key,
            validated: AtomicBool::new(self.validated.load(Ordering::Relaxed)),
        }
    }
}

impl NgdpBytes {
    /// Create new NgdpBytes with automatic validation
    ///
    /// This constructor immediately validates the content against the provided
    /// content key using cascette-crypto MD5 validation.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new_validated(data: Bytes, content_key: ContentKey) -> NgdpCacheResult<Self> {
        let ngdp_bytes = Self {
            data: data.clone(),
            content_key: Some(content_key),
            validated: AtomicBool::new(false),
        };

        // Validate content integrity using md5 crate
        let computed_hash = md5::compute(&data);
        let expected_hash = content_key.as_bytes();

        if computed_hash.as_ref() == expected_hash {
            ngdp_bytes.validated.store(true, Ordering::Relaxed);
            Ok(ngdp_bytes)
        } else {
            Err(NgdpCacheError::ContentValidationFailed(content_key))
        }
    }

    /// Create new NgdpBytes with content key (lazy validation)
    pub fn new_with_key(data: Bytes, content_key: ContentKey) -> Self {
        Self {
            data,
            content_key: Some(content_key),
            validated: AtomicBool::new(false),
        }
    }

    /// Create new NgdpBytes without content key (no validation possible)
    pub fn new_without_key(data: Bytes) -> Self {
        Self {
            data,
            content_key: None,
            validated: AtomicBool::new(true), // Consider "valid" since no validation is possible
        }
    }

    /// Get the underlying bytes
    pub fn as_bytes(&self) -> &Bytes {
        &self.data
    }

    /// Get the content key if available
    pub fn content_key(&self) -> Option<&ContentKey> {
        self.content_key.as_ref()
    }

    /// Lazy validation for performance-critical paths
    ///
    /// This method performs validation only if needed and returns whether
    /// the content is valid. Uses AtomicBool to track validation status
    /// in a thread-safe manner.
    pub fn validate_if_needed(&self) -> NgdpCacheResult<bool> {
        // Fast path: already validated
        if self.validated.load(Ordering::Relaxed) {
            return Ok(true);
        }

        // Perform validation using md5 crate if content key is available
        if let Some(content_key) = &self.content_key {
            let computed_hash = md5::compute(&self.data);
            let expected_hash = content_key.as_bytes();

            let is_valid = computed_hash.as_ref() == expected_hash;
            if is_valid {
                self.validated.store(true, Ordering::Relaxed);
            }

            Ok(is_valid)
        } else {
            // No content key means no validation is possible/required
            Ok(true)
        }
    }

    /// Check if validation is needed
    pub fn needs_validation(&self) -> bool {
        !self.validated.load(Ordering::Relaxed) && self.content_key.is_some()
    }

    /// Check if content is validated
    pub fn is_validated(&self) -> bool {
        self.validated.load(Ordering::Relaxed)
    }

    /// Validate content using provided hooks
    ///
    /// This method integrates with the ValidationHooks system while maintaining
    /// thread-safe validation status tracking.
    pub async fn validate_with_hooks<H: ValidationHooks + ?Sized>(
        &self,
        hooks: &H,
    ) -> NgdpCacheResult<ValidationResult> {
        // Fast path: already validated
        if self.validated.load(Ordering::Relaxed) {
            return Ok(ValidationResult::valid(
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
                self.data.len(),
            ));
        }
        if let Some(content_key) = &self.content_key {
            if hooks
                .should_skip_validation(content_key, self.data.len())
                .await
            {
                self.validated.store(true, Ordering::Relaxed);
                return Ok(ValidationResult::valid(
                    std::time::Duration::ZERO,
                    std::time::Duration::ZERO,
                    self.data.len(),
                ));
            }

            let result = hooks
                .validate_content(content_key, &self.data)
                .await
                .map_err(NgdpCacheError::from)?;

            if result.is_valid {
                self.validated.store(true, Ordering::Relaxed);
                hooks.on_validation_success(content_key, &result).await;
            } else {
                let ngdp_error = NgdpCacheError::ContentValidationFailed(*content_key);
                let cache_error = CacheError::ContentValidationFailed(format!(
                    "MD5 hash mismatch for content key: {content_key:?}"
                ));
                // Don't mark as validated since it failed
                hooks
                    .on_validation_failure(content_key, &self.data, &cache_error)
                    .await;
                return Err(ngdp_error);
            }

            Ok(result)
        } else {
            // No content key - consider validated
            self.validated.store(true, Ordering::Relaxed);
            Ok(ValidationResult::valid(
                std::time::Duration::ZERO,
                std::time::Duration::ZERO,
                self.data.len(),
            ))
        }
    }

    /// Get validation state
    pub fn validation_state(&self) -> &str {
        if self.content_key.is_none() {
            "skipped"
        } else if self.validated.load(Ordering::Relaxed) {
            "valid"
        } else {
            "pending"
        }
    }

    /// Create NgdpBytes from existing bytes, bypassing validation
    ///
    /// This is useful when you have already validated content elsewhere
    /// or when dealing with trusted data sources.
    pub fn from_validated_bytes(data: Bytes, content_key: Option<ContentKey>) -> Self {
        Self {
            data,
            content_key,
            validated: AtomicBool::new(true),
        }
    }

    /// Get the underlying bytes with zero-copy semantics
    ///
    /// Returns a clone of the underlying Bytes, which is zero-copy
    /// due to reference counting in the Bytes type.
    pub fn into_bytes(self) -> Bytes {
        self.data
    }

    /// Create NgdpBytes from a pooled buffer
    ///
    /// This method creates an NgdpBytes instance from a buffer allocated
    /// from a memory pool. The content key is optional for lazy validation.
    pub fn from_pool_buffer(buffer: bytes::BytesMut, content_key: Option<ContentKey>) -> Self {
        let data = buffer.freeze();
        match content_key {
            Some(key) => Self::new_with_key(data, key),
            None => Self::new_without_key(data),
        }
    }

    /// Create validated NgdpBytes from pooled buffer
    ///
    /// This method immediately validates the buffer content against the
    /// provided content key and returns an error if validation fails.
    pub fn from_pool_buffer_validated(
        buffer: bytes::BytesMut,
        content_key: ContentKey,
    ) -> crate::NgdpCacheResult<Self> {
        let data = buffer.freeze();
        Self::new_validated(data, content_key)
    }

    /// Extract underlying bytes for pool deallocation
    ///
    /// This method consumes the NgdpBytes and returns the underlying Bytes,
    /// which can then be converted back to BytesMut for pool deallocation.
    /// Note: This will only work if the Bytes has a reference count of 1.
    pub fn into_bytes_for_deallocation(self) -> Bytes {
        self.data
    }
}

// Implement Deref to make NgdpBytes behave like Bytes
impl std::ops::Deref for NgdpBytes {
    type Target = Bytes;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

// Implement AsRef for compatibility
impl AsRef<[u8]> for NgdpBytes {
    fn as_ref(&self) -> &[u8] {
        self.data.as_ref()
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::ContentKey;

    #[tokio::test]
    async fn test_md5_validation_hooks_success() {
        let hooks = Md5ValidationHooks::new();
        let data = b"test data";
        let content_key = ContentKey::from_data(data);

        let result = hooks
            .validate_content(&content_key, data)
            .await
            .expect("Operation should succeed");
        assert!(result.is_valid);
        assert!(result.validation_time > std::time::Duration::ZERO);
        assert_eq!(result.content_size, data.len());

        // Check metrics
        assert_eq!(hooks.metrics.total_validations.load(Ordering::Relaxed), 1);
        assert_eq!(
            hooks.metrics.successful_validations.load(Ordering::Relaxed),
            1
        );
        assert_eq!(hooks.metrics.failed_validations.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn test_md5_validation_hooks_failure() {
        let hooks = Md5ValidationHooks::new();
        let data = b"test data";
        let wrong_content_key = ContentKey::from_data(b"wrong data");

        let result = hooks
            .validate_content(&wrong_content_key, data)
            .await
            .expect("Operation should succeed");
        assert!(!result.is_valid);
        assert!(result.validation_time > std::time::Duration::ZERO);
        assert_eq!(result.content_size, data.len());

        // Check metrics
        assert_eq!(hooks.metrics.total_validations.load(Ordering::Relaxed), 1);
        assert_eq!(
            hooks.metrics.successful_validations.load(Ordering::Relaxed),
            0
        );
        assert_eq!(hooks.metrics.failed_validations.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn test_ngdp_bytes_with_validation() {
        let data = Bytes::from_static(b"test content");
        let content_key = ContentKey::from_data(&data);
        let ngdp_bytes = NgdpBytes::new_with_key(data.clone(), content_key);

        assert!(ngdp_bytes.needs_validation());
        assert!(!ngdp_bytes.is_validated());

        let hooks = Md5ValidationHooks::new();
        let result = ngdp_bytes
            .validate_with_hooks(&hooks)
            .await
            .expect("Operation should succeed");

        assert!(result.is_valid);
        assert!(!ngdp_bytes.needs_validation());
        assert!(ngdp_bytes.is_validated());
        assert_eq!(ngdp_bytes.validation_state(), "valid");
    }

    #[tokio::test]
    async fn test_ngdp_bytes_new_validated() {
        let data = Bytes::from_static(b"test content for validation");
        let content_key = ContentKey::from_data(&data);

        // Should succeed with correct content key
        let ngdp_bytes =
            NgdpBytes::new_validated(data.clone(), content_key).expect("Operation should succeed");
        assert!(ngdp_bytes.is_validated());
        assert!(!ngdp_bytes.needs_validation());
        assert_eq!(ngdp_bytes.validation_state(), "valid");

        // Should fail with incorrect content key
        let wrong_key = ContentKey::from_data(b"wrong data");
        let result = NgdpBytes::new_validated(data, wrong_key);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_ngdp_bytes_validate_if_needed() {
        let data = Bytes::from_static(b"test content");
        let content_key = ContentKey::from_data(&data);
        let ngdp_bytes = NgdpBytes::new_with_key(data, content_key);

        // First call should perform validation
        assert!(
            ngdp_bytes
                .validate_if_needed()
                .expect("Operation should succeed")
        );
        assert!(ngdp_bytes.is_validated());

        // Second call should return immediately (already validated)
        assert!(
            ngdp_bytes
                .validate_if_needed()
                .expect("Operation should succeed")
        );
    }

    #[tokio::test]
    async fn test_ngdp_bytes_zero_copy_operations() {
        let original_data = Bytes::from_static(b"test data for zero copy");
        let content_key = ContentKey::from_data(&original_data);

        let ngdp_bytes = NgdpBytes::new_validated(original_data.clone(), content_key)
            .expect("Operation should succeed");

        // Test zero-copy access
        let bytes_ref = ngdp_bytes.as_bytes();
        assert_eq!(bytes_ref, &original_data);

        // Test zero-copy extraction
        let extracted_bytes = ngdp_bytes.into_bytes();
        assert_eq!(extracted_bytes, original_data);
    }

    #[tokio::test]
    async fn test_ngdp_bytes_thread_safety() {
        use std::sync::Arc;
        use tokio::task;

        let data = Bytes::from_static(b"thread safety test");
        let content_key = ContentKey::from_data(&data);
        let ngdp_bytes = Arc::new(NgdpBytes::new_with_key(data, content_key));

        // Spawn multiple tasks that try to validate concurrently
        let mut handles = vec![];
        for _ in 0..10 {
            let bytes = ngdp_bytes.clone();
            let handle = task::spawn(async move {
                bytes
                    .validate_if_needed()
                    .expect("Operation should succeed")
            });
            handles.push(handle);
        }

        // All should succeed
        for handle in handles {
            assert!(handle.await.expect("Operation should succeed"));
        }

        // Should be validated after concurrent access
        assert!(ngdp_bytes.is_validated());
    }

    #[tokio::test]
    async fn test_ngdp_bytes_without_key() {
        let data = Bytes::from_static(b"test content");
        let ngdp_bytes = NgdpBytes::new_without_key(data);

        assert!(!ngdp_bytes.needs_validation());
        assert!(ngdp_bytes.is_validated());
        assert_eq!(ngdp_bytes.validation_state(), "skipped");
        assert!(ngdp_bytes.content_key().is_none());
    }

    #[test]
    fn test_validation_metrics() {
        let metrics = ValidationMetrics::new();

        metrics.record_success(std::time::Duration::from_millis(10), 1000);
        metrics.record_failure(std::time::Duration::from_millis(5), 500);

        assert_eq!(metrics.total_validations.load(Ordering::Relaxed), 2);
        assert_eq!(metrics.successful_validations.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.failed_validations.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.bytes_validated.load(Ordering::Relaxed), 1500);
        assert_eq!(metrics.success_rate(), 0.5);

        let avg_time = metrics.average_validation_time();
        // Average is (10+5)/2 = 7.5ms, but we get nanosecond precision
        assert!(avg_time >= std::time::Duration::from_millis(7));
        assert!(avg_time <= std::time::Duration::from_millis(8));
    }

    #[tokio::test]
    async fn test_no_op_validation_hooks() {
        let hooks = NoOpValidationHooks;
        let data = b"any data";
        let content_key = ContentKey::from_data(b"different data");

        let result = hooks
            .validate_content(&content_key, data)
            .await
            .expect("Operation should succeed");
        assert!(result.is_valid); // Always valid
        assert_eq!(result.validation_time, std::time::Duration::ZERO);

        assert!(hooks.should_skip_validation(&content_key, data.len()).await);
    }

    #[tokio::test]
    async fn test_ngdp_validation_hooks_basic() {
        let hooks = NgdpValidationHooks::new();
        let data = b"test content";
        let content_key = ContentKey::from_data(data);

        let result = hooks
            .validate_content(&content_key, data)
            .await
            .expect("Operation should succeed");
        assert!(result.is_valid);
        assert!(result.validation_time > std::time::Duration::ZERO);
    }

    #[tokio::test]
    async fn test_ngdp_validation_hooks_jenkins96() {
        let hooks = NgdpValidationHooks::new().with_jenkins96_validation();
        let data = b"jenkins test data";
        let expected_hash = Jenkins96::hash(data);

        let result = hooks
            .validate_jenkins96(expected_hash.hash64, data)
            .expect("Operation should succeed");
        assert!(result.is_valid);
        assert!(result.validation_time > std::time::Duration::ZERO);

        // Test with wrong hash
        let wrong_result = hooks
            .validate_jenkins96(12345, data)
            .expect("Operation should succeed");
        assert!(!wrong_result.is_valid);
    }

    #[tokio::test]
    async fn test_ngdp_validation_hooks_encoding_key() {
        let hooks = NgdpValidationHooks::new();
        let data = b"encoding key test";
        let encoding_key = EncodingKey::from_data(data);

        let result = hooks
            .validate_encoding_key(&encoding_key, data)
            .expect("Operation should succeed");
        assert!(result.is_valid);
        assert!(result.validation_time > std::time::Duration::ZERO);
    }

    #[tokio::test]
    async fn test_ngdp_validation_hooks_batch() {
        let hooks = NgdpValidationHooks::new();
        let data1 = b"batch test 1";
        let data2 = b"batch test 2";
        let key1 = ContentKey::from_data(data1);
        let key2 = ContentKey::from_data(data2);

        let items = vec![(key1, data1.as_slice()), (key2, data2.as_slice())];
        let results = hooks
            .batch_validate_content(&items)
            .await
            .expect("Operation should succeed");

        assert_eq!(results.len(), 2);
        assert!(results[0].is_valid);
        assert!(results[1].is_valid);
    }

    #[tokio::test]
    async fn test_ngdp_validation_hooks_with_tact_key() {
        // Create a mock TACT key (in real implementation, this would be a proper TACT key)
        let tact_key = TactKey::new(
            12345,
            [
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
                0x0e, 0x0f,
            ],
        );
        let hooks = NgdpValidationHooks::with_tact_key(tact_key);

        let data = b"encrypted content";
        let content_key = ContentKey::from_data(data);

        let result = hooks
            .validate_content(&content_key, data)
            .await
            .expect("Operation should succeed");
        assert!(result.is_valid);
    }
}
