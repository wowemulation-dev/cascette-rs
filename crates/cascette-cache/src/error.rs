//! Error types for cache operations

use cascette_crypto::ContentKey;
use thiserror::Error;

/// Errors that can occur during cache operations
#[derive(Debug, Error)]
pub enum CacheError {
    /// Key not found in cache
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// Cache entry has expired
    #[error("Cache entry expired: {0}")]
    EntryExpired(String),

    /// Cache is at capacity and cannot store new entries
    #[error("Cache capacity exceeded")]
    CapacityExceeded,

    /// Invalid cache configuration
    #[error("Invalid cache configuration: {0}")]
    InvalidConfiguration(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// IO error during cache operations
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Cache backend-specific error
    #[error("Backend error: {0}")]
    Backend(String),

    /// Cache invalidation error
    #[error("Invalidation error: {0}")]
    Invalidation(String),

    /// Lock acquisition timeout
    #[error("Lock timeout: {0}")]
    LockTimeout(String),

    /// Cache corruption detected
    #[error("Cache corruption detected: {0}")]
    Corruption(String),

    /// Content validation failed
    #[error("Content validation failed: {0}")]
    ContentValidationFailed(String),

    /// Content parsing failed
    #[error("Content parsing failed: {0}")]
    ContentParsingFailed(String),

    /// Invalid content key format
    #[error("Invalid content key format: {0}")]
    InvalidContentKey(String),

    /// Storage configuration error (e.g., unavailable storage backend)
    #[error("Configuration error: {0}")]
    Config(String),

    /// Storage quota exceeded (e.g., browser localStorage limit)
    #[error("Storage quota exceeded")]
    StorageQuotaExceeded,
}

impl From<hex::FromHexError> for CacheError {
    fn from(err: hex::FromHexError) -> Self {
        Self::Deserialization(err.to_string())
    }
}

// Add support for cascette-crypto error types when they become available
// This is a placeholder for future integration
// impl From<cascette_crypto::CryptoError> for CacheError {
//     fn from(err: cascette_crypto::CryptoError) -> Self {
//         Self::ContentValidationFailed(err.to_string())
//     }
// }

/// NGDP-specific cache errors with enhanced context for content distribution operations
///
/// This enum extends basic cache errors with NGDP/CASC-specific error types that provide
/// meaningful context for content validation, BLTE operations, CDN interactions, and
/// streaming operations commonly used in NGDP workloads.
#[derive(Debug, Error)]
pub enum NgdpCacheError {
    /// Content validation failed for a specific content key
    ///
    /// This indicates MD5 hash mismatch or other content integrity issues.
    /// The ContentKey provides the expected hash for debugging.
    #[error("Content validation failed for key: {0:?}")]
    ContentValidationFailed(ContentKey),

    /// BLTE decompression operation failed
    ///
    /// This covers errors in BLTE block decompression, invalid block headers,
    /// or corrupted compressed data streams.
    #[error("BLTE decompression failed: {0}")]
    BlteDecompressionFailed(String),

    /// Archive range request failed
    ///
    /// This covers HTTP range request failures, invalid range specifications,
    /// or archive index corruption preventing range access.
    #[error("Archive range request failed: {0}")]
    ArchiveRangeRequestFailed(String),

    /// CDN fetch operation failed
    ///
    /// This covers network failures, CDN server errors, or timeout conditions
    /// when fetching content from Blizzard's content delivery network.
    #[error("CDN fetch failed: {0}")]
    CdnFetchFailed(String),

    /// Stream processing error during large file operations
    ///
    /// This covers errors in streaming large files, chunk processing failures,
    /// or async stream coordination issues.
    #[error("Stream processing error: {0}")]
    StreamProcessingError(String),

    /// Format parsing failed
    ///
    /// This covers failures when parsing NGDP formats like root files,
    /// encoding files, or other binary formats.
    #[error("Format parsing failed: {0}")]
    ParseFailed(String),

    /// Serialization failed for parsed content
    ///
    /// This covers failures when serializing parsed structures for caching.
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),

    /// Network error during CDN operations
    ///
    /// This covers general network errors like connection failures,
    /// DNS resolution errors, or HTTP errors.
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Cache is full and cannot accept new entries
    ///
    /// This occurs when cache limits are reached and eviction
    /// policies cannot free enough space.
    #[error("Cache is full")]
    CacheFull,

    /// Wrapper for existing cache errors to maintain backward compatibility
    ///
    /// All existing CacheError variants are accessible through this wrapper,
    /// ensuring no breaking changes for existing code.
    #[error("Cache error: {0}")]
    Cache(#[from] CacheError),
}

/// Convenience function to convert CacheResult to NgdpCacheResult
///
/// This allows seamless integration between existing cache code and new NGDP-specific code.
pub fn to_ngdp_result<T>(result: CacheResult<T>) -> NgdpCacheResult<T> {
    result.map_err(NgdpCacheError::from)
}

/// Result type alias for cache operations
pub type CacheResult<T> = Result<T, CacheError>;

/// Result type alias for NGDP-specific cache operations
///
/// Use this type alias for functions that perform NGDP-specific operations
/// such as content validation, BLTE processing, or CDN interactions.
pub type NgdpCacheResult<T> = Result<T, NgdpCacheError>;

#[cfg(test)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::ContentKey;
    use std::io::{Error as IoError, ErrorKind};

    #[allow(clippy::cognitive_complexity)]
    fn verify_error_message(error: &CacheError, error_str: &str) {
        match error {
            CacheError::KeyNotFound(key) => {
                assert!(error_str.contains("Key not found"));
                assert!(error_str.contains(key));
            }
            CacheError::EntryExpired(key) => {
                assert!(error_str.contains("Cache entry expired"));
                assert!(error_str.contains(key));
            }
            CacheError::CapacityExceeded => {
                assert!(error_str.contains("Cache capacity exceeded"));
            }
            CacheError::InvalidConfiguration(msg) => {
                assert!(error_str.contains("Invalid cache configuration"));
                assert!(error_str.contains(msg));
            }
            CacheError::Serialization(msg) => {
                assert!(error_str.contains("Serialization error"));
                assert!(error_str.contains(msg));
            }
            CacheError::Deserialization(msg) => {
                assert!(error_str.contains("Deserialization error"));
                assert!(error_str.contains(msg));
            }
            CacheError::Io(_) => {
                assert!(error_str.contains("IO error"));
            }
            CacheError::Backend(msg) => {
                assert!(error_str.contains("Backend error"));
                assert!(error_str.contains(msg));
            }
            CacheError::Invalidation(msg) => {
                assert!(error_str.contains("Invalidation error"));
                assert!(error_str.contains(msg));
            }
            CacheError::LockTimeout(msg) => {
                assert!(error_str.contains("Lock timeout"));
                assert!(error_str.contains(msg));
            }
            CacheError::Corruption(msg) => {
                assert!(error_str.contains("Cache corruption detected"));
                assert!(error_str.contains(msg));
            }
            CacheError::ContentValidationFailed(msg) => {
                assert!(error_str.contains("Content validation failed"));
                assert!(error_str.contains(msg));
            }
            CacheError::ContentParsingFailed(msg) => {
                assert!(error_str.contains("Content parsing failed"));
                assert!(error_str.contains(msg));
            }
            CacheError::InvalidContentKey(msg) => {
                assert!(error_str.contains("Invalid content key format"));
                assert!(error_str.contains(msg));
            }
            CacheError::Config(msg) => {
                assert!(error_str.contains("Configuration error"));
                assert!(error_str.contains(msg));
            }
            CacheError::StorageQuotaExceeded => {
                assert!(error_str.contains("Storage quota exceeded"));
            }
        }
    }

    #[test]
    fn test_cache_error_display() {
        let errors = vec![
            CacheError::KeyNotFound("test-key".to_string()),
            CacheError::EntryExpired("expired-key".to_string()),
            CacheError::CapacityExceeded,
            CacheError::InvalidConfiguration("bad config".to_string()),
            CacheError::Serialization("serialize failed".to_string()),
            CacheError::Deserialization("deserialize failed".to_string()),
            CacheError::Backend("backend error".to_string()),
            CacheError::Invalidation("invalidation failed".to_string()),
            CacheError::LockTimeout("lock timeout".to_string()),
            CacheError::Corruption("data corrupted".to_string()),
            CacheError::ContentValidationFailed("validation failed".to_string()),
            CacheError::ContentParsingFailed("parsing failed".to_string()),
            CacheError::InvalidContentKey("invalid key".to_string()),
        ];

        for error in errors {
            let error_str = error.to_string();
            assert!(!error_str.is_empty(), "Error message should not be empty");
            verify_error_message(&error, &error_str);
        }
    }

    #[test]
    fn test_cache_error_from_io_error() {
        let io_error = IoError::new(ErrorKind::PermissionDenied, "Access denied");
        let cache_error = CacheError::from(io_error);

        match cache_error {
            CacheError::Io(ref io_err) => {
                assert_eq!(io_err.kind(), ErrorKind::PermissionDenied);
                assert_eq!(io_err.to_string(), "Access denied");
            }
            _ => unreachable!("Expected IO error variant"),
        }
    }

    #[test]
    fn test_cache_error_from_hex_error() {
        let hex_error = hex::FromHexError::InvalidHexCharacter { c: 'g', index: 0 };
        let cache_error = CacheError::from(hex_error);

        match cache_error {
            CacheError::Deserialization(msg) => {
                // Just verify it contains some hex-related error
                assert!(!msg.is_empty());
                assert!(msg.to_lowercase().contains("hex") || msg.contains('g'));
            }
            _ => unreachable!("Expected Deserialization error variant"),
        }
    }

    #[test]
    fn test_cache_error_debug() {
        let error = CacheError::KeyNotFound("debug-test".to_string());
        let debug_str = format!("{error:?}");
        assert!(debug_str.contains("KeyNotFound"));
        assert!(debug_str.contains("debug-test"));
    }

    #[test]
    fn test_cache_result_ok() {
        let result: CacheResult<String> = Ok("success".to_string());
        assert!(result.is_ok());
        assert_eq!("success".to_string(), "success");
    }

    #[test]
    fn test_cache_result_err() {
        let result: CacheResult<String> = Err(CacheError::CapacityExceeded);
        assert!(result.is_err());
        match CacheError::CapacityExceeded {
            CacheError::CapacityExceeded => {} // Expected
            _ => unreachable!("Expected CapacityExceeded error"),
        }
    }

    #[test]
    fn test_error_chaining() {
        // Test that errors can be properly chained and maintain their context
        let io_error = IoError::new(ErrorKind::NotFound, "File not found");
        let cache_error = CacheError::from(io_error);

        // Test error source chain
        match &cache_error {
            CacheError::Io(io_err) => {
                assert_eq!(io_err.kind(), ErrorKind::NotFound);
            }
            _ => unreachable!("Expected IO error"),
        }

        // Test that the error displays correctly
        let error_string = cache_error.to_string();
        assert!(error_string.contains("IO error"));
        assert!(error_string.contains("File not found"));
    }

    #[test]
    fn test_error_variants_completeness() {
        // Ensure we can construct all error variants without panicking
        let _errors = [
            CacheError::KeyNotFound(String::new()),
            CacheError::EntryExpired(String::new()),
            CacheError::CapacityExceeded,
            CacheError::InvalidConfiguration(String::new()),
            CacheError::Serialization(String::new()),
            CacheError::Deserialization(String::new()),
            CacheError::Io(IoError::other("test")),
            CacheError::Backend(String::new()),
            CacheError::Invalidation(String::new()),
            CacheError::LockTimeout(String::new()),
            CacheError::Corruption(String::new()),
            CacheError::ContentValidationFailed(String::new()),
            CacheError::ContentParsingFailed(String::new()),
            CacheError::InvalidContentKey(String::new()),
            CacheError::Config(String::new()),
            CacheError::StorageQuotaExceeded,
        ];
    }

    #[test]
    fn test_error_with_empty_strings() {
        // Test edge case with empty strings
        let errors = vec![
            CacheError::KeyNotFound(String::new()),
            CacheError::EntryExpired(String::new()),
            CacheError::InvalidConfiguration(String::new()),
            CacheError::Serialization(String::new()),
            CacheError::Deserialization(String::new()),
            CacheError::Backend(String::new()),
            CacheError::Invalidation(String::new()),
            CacheError::LockTimeout(String::new()),
            CacheError::Corruption(String::new()),
            CacheError::ContentValidationFailed(String::new()),
            CacheError::ContentParsingFailed(String::new()),
            CacheError::InvalidContentKey(String::new()),
            CacheError::Config(String::new()),
        ];

        for error in errors {
            let error_str = error.to_string();
            assert!(
                !error_str.is_empty(),
                "Error message should not be empty even with empty input"
            );
        }
    }

    #[test]
    fn test_error_with_unicode_strings() {
        // Test with Unicode characters to ensure proper handling
        let unicode_key = "测试键🔑";
        let error = CacheError::KeyNotFound(unicode_key.to_string());
        let error_str = error.to_string();
        assert!(error_str.contains(unicode_key));
        assert!(error_str.contains("Key not found"));
    }

    #[test]
    fn test_error_with_very_long_strings() {
        // Test with long strings that might occur with large NGDP keys
        let long_key = "a".repeat(10000);
        let error = CacheError::KeyNotFound(long_key.clone());
        let error_str = error.to_string();
        assert!(error_str.contains(&long_key));
        assert!(error_str.len() > 10000); // Should contain the full key
    }

    #[test]
    fn test_ngdp_cache_error_display() {
        let test_data = b"test data";
        let content_key = ContentKey::from_data(test_data);

        let errors = vec![
            NgdpCacheError::ContentValidationFailed(content_key),
            NgdpCacheError::BlteDecompressionFailed("invalid header".to_string()),
            NgdpCacheError::ArchiveRangeRequestFailed("range 0-1024 not found".to_string()),
            NgdpCacheError::CdnFetchFailed("network timeout".to_string()),
            NgdpCacheError::StreamProcessingError("chunk corruption".to_string()),
            NgdpCacheError::Cache(CacheError::KeyNotFound("test-key".to_string())),
        ];

        for error in errors {
            let error_str = error.to_string();
            assert!(!error_str.is_empty(), "Error message should not be empty");

            match &error {
                NgdpCacheError::ContentValidationFailed(key) => {
                    assert!(error_str.contains("Content validation failed for key"));
                    assert!(error_str.contains(&format!("{key:?}")));
                }
                NgdpCacheError::BlteDecompressionFailed(msg) => {
                    assert!(error_str.contains("BLTE decompression failed"));
                    assert!(error_str.contains(msg));
                }
                NgdpCacheError::ArchiveRangeRequestFailed(msg) => {
                    assert!(error_str.contains("Archive range request failed"));
                    assert!(error_str.contains(msg));
                }
                NgdpCacheError::CdnFetchFailed(msg) => {
                    assert!(error_str.contains("CDN fetch failed"));
                    assert!(error_str.contains(msg));
                }
                NgdpCacheError::StreamProcessingError(msg) => {
                    assert!(error_str.contains("Stream processing error"));
                    assert!(error_str.contains(msg));
                }
                NgdpCacheError::Cache(cache_err) => {
                    assert!(error_str.contains("Cache error"));
                    // Also check that the underlying error is displayed
                    let cache_str = cache_err.to_string();
                    assert!(error_str.contains(&cache_str));
                }
                NgdpCacheError::ParseFailed(msg) => {
                    assert!(error_str.contains("Format parsing failed"));
                    assert!(error_str.contains(msg));
                }
                NgdpCacheError::SerializationFailed(msg) => {
                    assert!(error_str.contains("Serialization failed"));
                    assert!(error_str.contains(msg));
                }
                NgdpCacheError::NetworkError(msg) => {
                    assert!(error_str.contains("Network error"));
                    assert!(error_str.contains(msg));
                }
                NgdpCacheError::CacheFull => {
                    assert!(error_str.contains("Cache is full"));
                }
            }
        }
    }

    #[test]
    fn test_ngdp_cache_error_from_cache_error() {
        let cache_error = CacheError::KeyNotFound("test".to_string());
        let ngdp_error = NgdpCacheError::from(cache_error);

        match ngdp_error {
            NgdpCacheError::Cache(CacheError::KeyNotFound(key)) => {
                assert_eq!(key, "test");
            }
            _ => unreachable!("Expected Cache variant with KeyNotFound"),
        }
    }

    #[test]
    fn test_ngdp_cache_error_chaining() {
        // Test error source chain for wrapped cache errors
        let io_error = IoError::new(ErrorKind::PermissionDenied, "Access denied");
        let cache_error = CacheError::from(io_error);
        let ngdp_error = NgdpCacheError::from(cache_error);

        let error_string = ngdp_error.to_string();
        assert!(error_string.contains("Cache error"));
        assert!(error_string.contains("IO error"));
        assert!(error_string.contains("Access denied"));
    }

    #[test]
    fn test_ngdp_result_types() {
        // Test that both result types work correctly
        let cache_result: CacheResult<String> = Ok("success".to_string());
        assert!(cache_result.is_ok());

        let ngdp_result: NgdpCacheResult<String> = Ok("ngdp_success".to_string());
        assert!(ngdp_result.is_ok());

        let error_result: NgdpCacheResult<String> = Err(NgdpCacheError::BlteDecompressionFailed(
            "test error".to_string(),
        ));
        assert!(error_result.is_err());
    }

    #[test]
    fn test_ngdp_error_context_preservation() {
        // Test that NGDP errors preserve meaningful context
        let test_data = b"test content";
        let content_key = ContentKey::from_data(test_data);

        let validation_error = NgdpCacheError::ContentValidationFailed(content_key);
        let error_msg = validation_error.to_string();

        // Should contain the full content key in debug format for troubleshooting
        assert!(error_msg.contains("Content validation failed for key:"));

        // Should be able to extract meaningful information for logging
        match validation_error {
            NgdpCacheError::ContentValidationFailed(key) => {
                assert_eq!(key, content_key);
            }
            _ => unreachable!("Expected ContentValidationFailed variant"),
        }
    }

    #[test]
    fn test_to_ngdp_result_conversion() {
        // Test the convenience function for converting results
        let cache_success: CacheResult<i32> = Ok(42);
        let ngdp_success = to_ngdp_result(cache_success);
        assert_eq!(ngdp_success.expect("Operation should succeed"), 42);

        let cache_error: CacheResult<i32> = Err(CacheError::KeyNotFound("missing".to_string()));
        let ngdp_error = to_ngdp_result(cache_error);

        match ngdp_error {
            Err(NgdpCacheError::Cache(CacheError::KeyNotFound(key))) => {
                assert_eq!(key, "missing");
            }
            _ => unreachable!("Expected wrapped KeyNotFound error"),
        }
    }
}
