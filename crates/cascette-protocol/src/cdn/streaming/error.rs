//! Streaming-specific error types for CDN operations
//!
//! Provides detailed error contexts and recovery suggestions for streaming operations,
//! extending the base CDN error types with network-specific failure modes.

use thiserror::Error;

use tracing::debug;

/// Error type for streaming operations
///
/// Provides detailed context about streaming failures including network errors,
/// protocol violations, and resource constraints. Each error variant includes
/// actionable information for debugging and potential recovery strategies.
#[derive(Debug, Error)]
pub enum StreamingError {
    /// Network request failed
    ///
    /// Represents failures in the underlying network layer, including DNS resolution,
    /// connection establishment, and data transfer errors.
    #[error("Network request failed: {source}")]
    NetworkRequest {
        /// The underlying network error
        #[source]
        source: reqwest::Error,
    },

    /// HTTP client setup failed
    ///
    /// Occurs when the HTTP client cannot be initialized due to configuration issues
    /// or system resource constraints.
    #[error("HTTP client setup failed: {source}")]
    HttpClientSetup {
        /// The underlying client setup error
        #[source]
        source: reqwest::Error,
    },

    /// HTTP request returned non-success status
    ///
    /// The server returned an error status code. Common causes include:
    /// - 404: Archive or file not found
    /// - 403: Access denied (authentication/authorization failure)
    /// - 416: Range not satisfiable (invalid range request)
    /// - 500/502/503: Server errors (temporary, retry recommended)
    #[error("HTTP request failed with status {status_code} for URL: {url}")]
    HttpStatus {
        /// HTTP status code returned by the server
        status_code: u16,
        /// The URL that generated the error
        url: String,
    },

    /// Server does not support range requests
    ///
    /// The CDN server does not advertise support for HTTP range requests,
    /// which are required for efficient streaming operations.
    #[error("Server does not support range requests for URL: {url}")]
    RangeNotSupported {
        /// The URL that does not support ranges
        url: String,
    },

    /// Invalid range specification
    ///
    /// The requested byte range is malformed or logically invalid.
    /// Common causes include start > end, negative values, or ranges
    /// exceeding the actual content size.
    #[error("Invalid range specification: {reason}")]
    InvalidRange {
        /// Description of why the range is invalid
        reason: String,
    },

    /// Content length not available
    ///
    /// The server did not provide a Content-Length header, which is required
    /// for range request calculations and content validation.
    #[error("Content length not available for URL: {url}")]
    MissingContentLength {
        /// The URL missing content length
        url: String,
    },

    /// Request timeout exceeded
    ///
    /// The operation exceeded the configured timeout. This may indicate
    /// network congestion, server overload, or an inappropriately short timeout.
    #[error("Request timeout exceeded after {timeout_ms}ms for URL: {url}")]
    Timeout {
        /// Timeout that was exceeded in milliseconds
        timeout_ms: u64,
        /// The URL that timed out
        url: String,
    },

    /// Connection pool exhausted
    ///
    /// All available connections are in use and the pool cannot create new ones.
    /// This typically indicates high load or connection leaks.
    #[error("Connection pool exhausted: {reason}")]
    ConnectionPoolExhausted {
        /// Description of the pool exhaustion
        reason: String,
    },

    /// Range coalescing failed
    ///
    /// Multiple ranges could not be combined into an efficient request pattern.
    /// This is usually due to ranges being too scattered or numerous.
    #[error("Range coalescing failed: {reason}")]
    RangeCoalescingFailed {
        /// Description of why coalescing failed
        reason: String,
    },

    /// Stream buffer overflow
    ///
    /// The internal buffer for streaming data has exceeded its capacity.
    /// This may indicate a memory leak or inappropriate buffer sizing.
    #[error("Stream buffer overflow: {buffer_size} bytes exceeded")]
    BufferOverflow {
        /// Size of the buffer that overflowed
        buffer_size: usize,
    },

    /// Archive format error during streaming
    ///
    /// The streaming data does not conform to the expected archive format.
    /// This could indicate data corruption or an incorrect URL.
    #[error("Archive format error during streaming: {source}")]
    ArchiveFormat {
        /// The underlying archive format error
        #[source]
        source: super::super::ArchiveError,
    },

    /// IO error during streaming
    ///
    /// File system or other IO operation failed during streaming.
    #[error("IO error during streaming: {source}")]
    Io {
        /// The underlying IO error
        #[source]
        source: std::io::Error,
    },

    /// Configuration error
    ///
    /// The streaming configuration contains invalid or incompatible settings.
    #[error("Configuration error: {reason}")]
    Configuration {
        /// Description of the configuration problem
        reason: String,
    },

    /// CDN server failover error
    ///
    /// A specific CDN server failed, and the request is being retried
    /// with the next available server in the priority list.
    #[error("CDN server {server} failed: {source}")]
    CdnFailover {
        /// The CDN server hostname that failed
        server: String,
        /// The underlying error that caused the failure
        #[source]
        source: Box<Self>,
    },

    /// All CDN servers exhausted
    ///
    /// All configured CDN servers have been tried and failed.
    /// This indicates a widespread CDN outage or network connectivity issues.
    #[error(
        "All CDN servers failed after {attempts} attempts. Last error from {last_server}: {last_error}"
    )]
    AllCdnServersFailed {
        /// Number of server attempts made
        attempts: u32,
        /// The last server that was attempted
        last_server: String,
        /// The last error encountered
        last_error: String,
    },

    /// CDN path not cached
    ///
    /// The CDN path for a product has not been cached and must be resolved
    /// through a Ribbit query before content can be accessed.
    #[error("CDN path not cached for product '{product}'. Query Ribbit endpoint first.")]
    CdnPathNotCached {
        /// The product name that needs path resolution
        product: String,
    },

    /// CDN path resolution failed
    ///
    /// Failed to resolve the CDN path for a product through the Ribbit API.
    /// This may indicate network issues or an invalid product name.
    #[error("CDN path resolution failed for product '{product}': {reason}")]
    CdnPathResolution {
        /// The product name that failed resolution
        product: String,
        /// Description of the resolution failure
        reason: String,
    },

    /// Hash format validation error
    ///
    /// The provided content hash does not meet CASC format requirements.
    /// Content hashes must be 32-character hexadecimal strings.
    #[error("Invalid content hash format: {hash}. Expected 32-character hex string.")]
    InvalidHashFormat {
        /// The invalid hash that was provided
        hash: String,
    },

    /// CDN region not available
    ///
    /// The requested CDN region is not available or accessible.
    /// This commonly occurs with China region restrictions.
    #[error("CDN region '{region}' not available: {reason}")]
    CdnRegionUnavailable {
        /// The region that is unavailable
        region: String,
        /// Reason why the region is unavailable
        reason: String,
    },

    /// Rate limiting exceeded
    ///
    /// The CDN has imposed rate limiting on requests.
    /// Implement exponential backoff before retrying.
    #[error("Rate limit exceeded for {url}. Retry after {retry_after_ms}ms")]
    RateLimitExceeded {
        /// The URL that triggered rate limiting
        url: String,
        /// Suggested retry delay in milliseconds
        retry_after_ms: u64,
    },

    /// CDN content verification failed
    ///
    /// Downloaded content hash does not match the expected value.
    /// This indicates data corruption during transfer or CDN issues.
    #[error("Content verification failed. Expected hash {expected}, got {actual}")]
    ContentVerificationFailed {
        /// The expected content hash
        expected: String,
        /// The actual computed hash
        actual: String,
    },

    /// CDN mirror synchronization lag
    ///
    /// Community mirror has not yet synchronized the requested content.
    /// Try official Blizzard CDN or wait for mirror synchronization.
    #[error(
        "Mirror {mirror} has not synchronized content {hash}. Try official CDN or wait for sync."
    )]
    MirrorSyncLag {
        /// The mirror hostname that lacks the content
        mirror: String,
        /// The content hash that is missing
        hash: String,
    },

    /// BLTE format error during streaming
    ///
    /// The BLTE content could not be parsed or decompressed during streaming.
    /// This could indicate data corruption, invalid BLTE format, or missing encryption keys.
    #[error("BLTE error during streaming: {source}")]
    BlteError {
        /// The underlying BLTE error
        #[source]
        source: crate::blte::BlteError,
    },

    /// Server is unavailable for requests
    ///
    /// The server is temporarily or permanently unavailable due to health checks,
    /// circuit breaker activation, or manual removal from the pool.
    #[error("Server {server} is unavailable: {reason}")]
    ServerUnavailable {
        /// The server that is unavailable
        server: String,
        /// Reason for unavailability
        reason: String,
    },

    /// Connection limit exceeded
    ///
    /// The maximum number of concurrent connections to a server has been reached.
    /// This prevents overwhelming the CDN server with too many requests.
    #[error("Connection limit exceeded for server {server}: {limit} concurrent connections")]
    ConnectionLimit {
        /// The server that has reached its connection limit
        server: String,
        /// The connection limit that was exceeded
        limit: usize,
    },
}

impl StreamingError {
    /// Create a network request error with additional context
    pub fn network_with_context(source: reqwest::Error, context: &str) -> Self {
        if let Some(status) = source.status() {
            let url = source
                .url()
                .map_or_else(|| context.to_string(), std::string::ToString::to_string);

            Self::HttpStatus {
                status_code: status.as_u16(),
                url,
            }
        } else {
            Self::NetworkRequest { source }
        }
    }

    /// Create a configuration error with detailed context
    pub fn configuration_with_details(reason: impl Into<String>, details: &str) -> Self {
        Self::Configuration {
            reason: format!("{}: {}", reason.into(), details),
        }
    }

    /// Create an archive format error with context
    pub fn archive_format_with_context(source: super::super::ArchiveError, context: &str) -> Self {
        // Log the context for debugging but don't expose in error message
        debug!(
            "Archive format error context: {} - underlying error: {:?}",
            context, source
        );
        Self::ArchiveFormat { source }
    }

    /// Create an IO error with additional context
    pub fn io_with_context(source: std::io::Error, operation: &str, path: Option<&str>) -> Self {
        debug!(
            "IO error during {}: kind={:?}, path={:?}, error={}",
            operation,
            source.kind(),
            path,
            source
        );
        Self::Io { source }
    }

    /// Create a BLTE error with streaming context
    pub fn blte_with_context(
        source: crate::blte::BlteError,
        url: &str,
        chunk_info: Option<&str>,
    ) -> Self {
        debug!(
            "BLTE error during streaming from {}: chunk_info={:?}, error={:?}",
            url, chunk_info, source
        );
        Self::BlteError { source }
    }

    /// Create a timeout error with additional context
    pub fn timeout_with_context(timeout_ms: u64, url: String, operation: &str) -> Self {
        debug!(
            "Timeout during {} after {}ms for URL: {}",
            operation, timeout_ms, url
        );
        Self::Timeout { timeout_ms, url }
    }
    /// Determine if the error is transient and may succeed on retry
    ///
    /// Returns true for errors that are likely temporary, such as network timeouts,
    /// server errors (5xx), or connection issues.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::NetworkRequest { source } => {
                // Retry on timeout or connection errors
                source.is_timeout() || source.is_connect()
            }
            Self::HttpStatus { status_code, .. } => {
                // Retry on server errors and some client errors
                matches!(*status_code, 429 | 500..=599)
            }
            Self::Timeout { .. }
            | Self::ConnectionPoolExhausted { .. }
            | Self::CdnFailover { .. }
            | Self::RateLimitExceeded { .. }
            | Self::MirrorSyncLag { .. }
            | Self::ServerUnavailable { .. }
            | Self::ConnectionLimit { .. } => true,
            // BLTE errors are usually not retryable, handled by wildcard
            _ => false,
        }
    }

    /// Get suggested retry delay in milliseconds
    ///
    /// Provides exponential backoff suggestions based on error type.
    /// Returns None for non-retryable errors.
    pub fn retry_delay_ms(&self, attempt: u32) -> Option<u64> {
        if !self.is_retryable() {
            return None;
        }

        let base_delay = match self {
            Self::HttpStatus { status_code, .. } if *status_code == 429 => 1000, // Rate limited
            Self::HttpStatus { status_code, .. } if (500..=599).contains(status_code) => 500,
            Self::NetworkRequest { .. } => 200,
            Self::Timeout { .. } => 300,
            Self::ConnectionPoolExhausted { .. } => 100,
            Self::CdnFailover { .. } => 150, // Quick failover to next server
            Self::RateLimitExceeded { retry_after_ms, .. } => return Some(*retry_after_ms),
            Self::MirrorSyncLag { .. } => 5000, // Longer delay for mirror sync
            _ => 250,
        };

        // Exponential backoff with jitter (capped at 30 seconds)
        let delay = base_delay * 2_u64.pow(attempt.min(6));
        Some(delay.min(30_000))
    }

    /// Get recovery suggestions for the error
    ///
    /// Provides human-readable suggestions for resolving the error condition.
    pub fn recovery_suggestion(&self) -> String {
        match self {
            Self::NetworkRequest { .. } => {
                "Check network connectivity and DNS resolution. Consider retrying with exponential backoff.".to_string()
            }
            Self::HttpStatus { status_code, .. } => match *status_code {
                404 => "Verify the URL is correct and the resource exists on the CDN.".to_string(),
                403 => "Check authentication credentials and access permissions.".to_string(),
                416 => "Verify the range request is within the file bounds.".to_string(),
                429 => "Reduce request rate and implement exponential backoff.".to_string(),
                500..=599 => "Server error - retry with exponential backoff.".to_string(),
                _ => format!("HTTP error {status_code} - check server logs for details."),
            },
            Self::RangeNotSupported { .. } => {
                "Use a CDN that supports HTTP range requests, or fall back to full file downloads.".to_string()
            }
            Self::InvalidRange { .. } => {
                "Verify range calculations and ensure start <= end within file bounds.".to_string()
            }
            Self::MissingContentLength { .. } => {
                "Contact CDN administrator to ensure Content-Length headers are provided.".to_string()
            }
            Self::Timeout { .. } => {
                "Increase timeout values or check for network congestion.".to_string()
            }
            Self::ConnectionPoolExhausted { .. } => {
                "Increase connection pool size or investigate connection leaks.".to_string()
            }
            Self::RangeCoalescingFailed { .. } => {
                "Reduce the number of concurrent range requests or increase coalescing thresholds.".to_string()
            }
            Self::BufferOverflow { .. } => {
                "Increase buffer sizes or implement streaming with smaller chunks.".to_string()
            }
            Self::ArchiveFormat { .. } => {
                "Verify the archive URL and check for data corruption during transfer.".to_string()
            }
            Self::Io { .. } => {
                "Check file system permissions and available disk space.".to_string()
            }
            Self::Configuration { .. } => {
                "Review streaming configuration settings for validity and compatibility.".to_string()
            }
            Self::HttpClientSetup { .. } => {
                "Check system resources and HTTP client configuration parameters.".to_string()
            }
            Self::CdnFailover { server, .. } => {
                format!("CDN server {server} is temporarily unavailable. Trying next server in failover list.")
            }
            Self::AllCdnServersFailed { .. } => {
                "All CDN servers failed. Check network connectivity or try again later. Consider using community mirrors.".to_string()
            }
            Self::CdnPathNotCached { product } => {
                format!("Query Ribbit API to resolve CDN path for product '{product}' before accessing content.")
            }
            Self::CdnPathResolution { product, .. } => {
                format!("Verify product name '{product}' is correct and Ribbit API is accessible.")
            }
            Self::InvalidHashFormat { .. } => {
                "Ensure content hashes are 32-character hexadecimal strings (MD5 format).".to_string()
            }
            Self::CdnRegionUnavailable { region, .. } => {
                format!("Try a different CDN region. Region '{region}' may have access restrictions.")
            }
            Self::RateLimitExceeded { .. } => {
                "Implement exponential backoff and reduce request rate to stay within CDN limits.".to_string()
            }
            Self::ContentVerificationFailed { .. } => {
                "Retry download from a different CDN server. Content may be corrupted during transfer.".to_string()
            }
            Self::MirrorSyncLag { mirror, .. } => {
                format!("Mirror '{mirror}' is behind. Use official Blizzard CDN or wait for synchronization.")
            }
            Self::BlteError { .. } => {
                "Verify BLTE content integrity. Check if decryption keys are available. Try downloading from different CDN server.".to_string()
            }
            Self::ServerUnavailable { server, reason } => {
                format!("Server '{server}' is unavailable ({reason}). Wait for health check recovery or try other servers.")
            }
            Self::ConnectionLimit { server, limit } => {
                format!("Server '{server}' has reached connection limit ({limit}). Wait for connections to close or try other servers.")
            }
        }
    }

    /// Get CDN-specific failover recommendations
    ///
    /// Provides suggestions for which CDN servers to try next based on the error type.
    pub fn cdn_failover_suggestion(&self) -> Option<String> {
        match self {
            Self::HttpStatus { status_code, .. } => match *status_code {
                404 => Some(
                    "Try community mirrors - content may not be available on this CDN".to_string(),
                ),
                403 => Some("Try different region or community mirrors for access".to_string()),
                429 => Some("Switch to different CDN server to avoid rate limiting".to_string()),
                500..=599 => Some("Failover to backup CDN servers".to_string()),
                _ => None,
            },
            Self::Timeout { .. } => Some("Try geographically closer CDN servers".to_string()),
            Self::NetworkRequest { .. } => Some("Try community mirrors as backup".to_string()),
            Self::CdnRegionUnavailable { .. } => {
                Some("Use community mirrors or different region CDN".to_string())
            }
            Self::MirrorSyncLag { .. } => {
                Some("Fallback to official Blizzard CDN servers".to_string())
            }
            _ => None,
        }
    }

    /// Check if this error suggests trying community mirrors
    pub fn should_try_mirrors(&self) -> bool {
        matches!(
            self,
            Self::HttpStatus {
                status_code: 404 | 403 | 500..=599,
                ..
            } | Self::Timeout { .. }
                | Self::NetworkRequest { .. }
                | Self::AllCdnServersFailed { .. }
                | Self::CdnRegionUnavailable { .. }
        )
    }

    /// Check if this error suggests trying official CDN instead of mirrors
    pub fn should_try_official_cdn(&self) -> bool {
        matches!(
            self,
            Self::MirrorSyncLag { .. } | Self::ContentVerificationFailed { .. }
        )
    }

    /// Get the underlying error chain for logging
    pub fn error_chain(&self) -> Vec<&dyn std::error::Error> {
        use std::error::Error;
        let mut chain = vec![self as &dyn std::error::Error];
        let mut current = self.source();
        while let Some(err) = current {
            chain.push(err);
            current = err.source();
        }
        chain
    }

    /// Get a sanitized error message safe for production logging
    ///
    /// This method removes potentially sensitive information like:
    /// - Full URLs that might contain secrets
    /// - Internal paths or configuration details
    /// - Specific error details that could aid attackers
    pub fn sanitized_message(&self) -> String {
        match self {
            // Remove potentially sensitive information from error messages
            Self::HttpStatus { status_code, .. } => {
                format!("HTTP error {status_code}")
            }
            Self::NetworkRequest { .. } => "Network request failed".to_string(),
            Self::Configuration { .. } => "Configuration error".to_string(),
            Self::Timeout { timeout_ms, .. } => {
                format!("Request timeout after {timeout_ms}ms")
            }
            Self::ArchiveFormat { .. } => "Archive format error".to_string(),
            Self::BlteError { .. } => "Content decompression error".to_string(),
            Self::CdnFailover { server, .. } => {
                // Only expose server hostname, not full error chain
                format!("CDN server {} unavailable", Self::sanitize_hostname(server))
            }
            Self::ServerUnavailable { server, .. } => {
                format!("Server {} unavailable", Self::sanitize_hostname(server))
            }
            _ => {
                // For other errors, use a generic message
                "Streaming operation failed".to_string()
            }
        }
    }

    /// Sanitize hostname to prevent information leakage
    fn sanitize_hostname(hostname: &str) -> String {
        // Only show the domain part, hide subdomains that might be sensitive
        if let Some(domain_start) = hostname.rfind('.') {
            if let Some(prev_dot) = hostname[..domain_start].rfind('.') {
                format!("***.{}", &hostname[prev_dot + 1..])
            } else {
                hostname.to_string()
            }
        } else {
            "<hidden>".to_string()
        }
    }
}

/// Input validation utilities for security
pub struct InputValidator;

impl InputValidator {
    /// Validate content hash format to prevent injection attacks
    pub fn validate_content_hash(hash: &str) -> Result<(), StreamingError> {
        if hash.len() != 32 {
            return Err(StreamingError::InvalidHashFormat {
                hash: "<invalid length>".to_string(),
            });
        }

        if !hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(StreamingError::InvalidHashFormat {
                hash: "<invalid characters>".to_string(),
            });
        }

        Ok(())
    }

    /// Validate URL to prevent SSRF attacks
    pub fn validate_url(url: &str) -> Result<(), StreamingError> {
        if url.len() > 2048 {
            return Err(StreamingError::Configuration {
                reason: "URL too long".to_string(),
            });
        }

        // Check for dangerous schemes
        if url.starts_with("file://") || url.starts_with("ftp://") {
            return Err(StreamingError::Configuration {
                reason: "Unsupported URL scheme".to_string(),
            });
        }

        // Ensure only HTTP/HTTPS
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(StreamingError::Configuration {
                reason: "Only HTTP/HTTPS URLs are supported".to_string(),
            });
        }

        // Prevent localhost/internal network access
        if url.contains("localhost") || url.contains("127.0.0.1") || url.contains("::1") {
            return Err(StreamingError::Configuration {
                reason: "Access to local resources is not permitted".to_string(),
            });
        }

        // Check for private IP ranges (basic check)
        if url.contains("10.") || url.contains("192.168.") || url.contains("172.") {
            return Err(StreamingError::Configuration {
                reason: "Access to private network ranges is not permitted".to_string(),
            });
        }

        Ok(())
    }

    /// Validate server hostname
    pub fn validate_hostname(hostname: &str) -> Result<(), StreamingError> {
        if hostname.is_empty() {
            return Err(StreamingError::Configuration {
                reason: "Empty hostname".to_string(),
            });
        }

        if hostname.len() > 253 {
            return Err(StreamingError::Configuration {
                reason: "Hostname too long".to_string(),
            });
        }

        // Basic hostname validation - only alphanumeric, dots, and hyphens
        if !hostname
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(StreamingError::Configuration {
                reason: "Invalid hostname format".to_string(),
            });
        }

        // Prevent localhost access
        if hostname == "localhost" || hostname.starts_with("127.") {
            return Err(StreamingError::Configuration {
                reason: "Access to localhost is not permitted".to_string(),
            });
        }

        Ok(())
    }
}

/// Result type for streaming operations
pub type StreamingResult<T> = Result<T, StreamingError>;

impl From<super::super::ArchiveError> for StreamingError {
    fn from(error: super::super::ArchiveError) -> Self {
        debug!("Converting ArchiveError to StreamingError: {:?}", error);
        Self::ArchiveFormat { source: error }
    }
}

impl From<std::io::Error> for StreamingError {
    fn from(error: std::io::Error) -> Self {
        debug!(
            "Converting std::io::Error to StreamingError: kind={:?}, message={}",
            error.kind(),
            error
        );
        Self::Io { source: error }
    }
}

impl From<reqwest::Error> for StreamingError {
    fn from(error: reqwest::Error) -> Self {
        debug!(
            "Converting reqwest::Error to StreamingError: is_timeout={}, is_connect={}, status={:?}",
            error.is_timeout(),
            error.is_connect(),
            error.status()
        );

        // Check if this is an HTTP status error and preserve more context
        if let Some(status) = error.status() {
            // Extract URL from the error if available
            let url = error
                .url()
                .map_or_else(|| "<unknown>".to_string(), std::string::ToString::to_string);

            debug!("HTTP status error: {} for URL: {}", status.as_u16(), url);
            Self::HttpStatus {
                status_code: status.as_u16(),
                url,
            }
        } else {
            // Log additional context for network errors
            if error.is_timeout() {
                debug!("Network timeout error");
            } else if error.is_connect() {
                debug!("Network connection error");
            } else if error.is_decode() {
                debug!("Network decode error");
            } else {
                debug!("Other network error: {}", error);
            }

            Self::NetworkRequest { source: error }
        }
    }
}

impl From<crate::blte::BlteError> for StreamingError {
    fn from(error: crate::blte::BlteError) -> Self {
        debug!("Converting BlteError to StreamingError: {:?}", error);
        Self::BlteError { source: error }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;

    #[test]
    fn test_retryable_errors() {
        let server_error = StreamingError::HttpStatus {
            status_code: 500,
            url: "http://example.com".to_string(),
        };
        assert!(server_error.is_retryable());

        let client_error = StreamingError::HttpStatus {
            status_code: 404,
            url: "http://example.com".to_string(),
        };
        assert!(!client_error.is_retryable());

        let timeout_error = StreamingError::Timeout {
            timeout_ms: 5000,
            url: "http://example.com".to_string(),
        };
        assert!(timeout_error.is_retryable());
    }

    #[test]
    fn test_retry_delay_calculation() {
        let error = StreamingError::HttpStatus {
            status_code: 500,
            url: "http://example.com".to_string(),
        };

        assert_eq!(error.retry_delay_ms(0), Some(500));
        assert_eq!(error.retry_delay_ms(1), Some(1000));
        assert_eq!(error.retry_delay_ms(2), Some(2000));
        // Capped at 30 seconds
        assert_eq!(error.retry_delay_ms(10), Some(30_000));

        let non_retryable = StreamingError::InvalidRange {
            reason: "test".to_string(),
        };
        assert_eq!(non_retryable.retry_delay_ms(0), None);
    }

    #[test]
    fn test_recovery_suggestions() {
        let error = StreamingError::HttpStatus {
            status_code: 404,
            url: "http://example.com".to_string(),
        };

        let suggestion = error.recovery_suggestion();
        assert!(suggestion.contains("Verify the URL"));
    }

    #[test]
    fn test_sanitized_messages() {
        let error = StreamingError::HttpStatus {
            status_code: 404,
            url: "http://secret.internal.com/sensitive/path".to_string(),
        };

        let sanitized = error.sanitized_message();
        assert_eq!(sanitized, "HTTP error 404");
        assert!(!sanitized.contains("secret"));
        assert!(!sanitized.contains("internal"));
        assert!(!sanitized.contains("sensitive"));
    }

    #[test]
    fn test_hostname_sanitization() {
        assert_eq!(
            StreamingError::sanitize_hostname("sub.domain.example.com"),
            "***.example.com"
        );
        assert_eq!(
            StreamingError::sanitize_hostname("example.com"),
            "example.com"
        );
        assert_eq!(StreamingError::sanitize_hostname("localhost"), "<hidden>");
    }

    #[test]
    fn test_input_validation() {
        // Valid content hash
        assert!(InputValidator::validate_content_hash("a1b2c3d4e5f6789012345678901234ab").is_ok());

        // Invalid content hash - too short
        assert!(InputValidator::validate_content_hash("short").is_err());

        // Invalid content hash - invalid characters
        assert!(InputValidator::validate_content_hash("g1b2c3d4e5f6789012345678901234ab").is_err());

        // Valid URL
        assert!(InputValidator::validate_url("https://test-cdn.example.com/path").is_ok());

        // Invalid URL - localhost
        assert!(InputValidator::validate_url("http://localhost/path").is_err());

        // Invalid URL - private IP
        assert!(InputValidator::validate_url("http://192.168.1.1/path").is_err());

        // Invalid URL - file scheme
        assert!(InputValidator::validate_url("file:///etc/passwd").is_err());

        // Valid hostname
        assert!(InputValidator::validate_hostname("test-cdn.example.com").is_ok());

        // Invalid hostname - empty
        assert!(InputValidator::validate_hostname("").is_err());

        // Invalid hostname - localhost
        assert!(InputValidator::validate_hostname("localhost").is_err());
    }
}
