//! Tests for enhanced error handling

use tact_client::Error;

#[test]
fn test_error_constructors() {
    // Test invalid manifest error
    let err = Error::invalid_manifest(42, "unexpected token");
    assert_eq!(
        err.to_string(),
        "Invalid manifest format at line 42: unexpected token"
    );

    // Test missing field error
    let err = Error::missing_field("BuildConfig");
    assert_eq!(err.to_string(), "Missing required field: BuildConfig");

    // Test CDN exhausted error
    let err = Error::cdn_exhausted("wow/config/1234");
    assert_eq!(
        err.to_string(),
        "All CDN hosts exhausted for wow/config/1234"
    );

    // Test file not found error
    let err = Error::file_not_found("/path/to/file.dat");
    assert_eq!(err.to_string(), "File not found: /path/to/file.dat");

    // Test invalid hash error
    let err = Error::invalid_hash("not-a-valid-hash");
    assert_eq!(err.to_string(), "Invalid hash format: not-a-valid-hash");

    // Test checksum mismatch error
    let err = Error::checksum_mismatch("abc123", "def456");
    assert_eq!(
        err.to_string(),
        "Checksum verification failed: expected abc123, got def456"
    );
}

#[test]
fn test_error_variants() {
    // Test that each error variant can be created
    let errors = vec![
        Error::InvalidRegion("xyz".to_string()),
        Error::UnsupportedProduct("unknown".to_string()),
        Error::InvalidProtocolVersion,
        Error::InvalidResponse,
        Error::ConnectionTimeout {
            host: "example.com".to_string(),
        },
    ];

    // Verify all errors have proper Display implementations
    for err in errors {
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn test_error_debug() {
    let err = Error::missing_field("test");
    let debug_str = format!("{err:?}");
    assert!(debug_str.contains("MissingField"));
    assert!(debug_str.contains("test"));
}
