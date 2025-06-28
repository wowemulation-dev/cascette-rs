//! Integration tests for the Ribbit client

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::test]
async fn test_ribbit_summary_v1() {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);
    let result = client.request(&Endpoint::Summary).await;

    // We expect either success or connection failure in tests
    match result {
        Ok(response) => {
            assert!(!response.raw.is_empty());

            // V1 responses should have MIME parts
            assert!(response.mime_parts.is_some());
            let mime_parts = response.mime_parts.unwrap();

            // Should have data content
            assert!(!mime_parts.data.is_empty());

            // Should have checksum
            assert!(mime_parts.checksum.is_some());

            // Data should contain product information
            assert!(response.data.is_some());
            let data = response.data.unwrap();
            assert!(data.contains("Product") || data.contains("product"));
        }
        Err(e) => {
            // In CI/test environments, connection might fail
            eprintln!("Connection failed (expected in offline tests): {e}");
        }
    }
}

#[tokio::test]
async fn test_ribbit_summary_v2() {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    let result = client.request_raw(&Endpoint::Summary).await;

    match result {
        Ok(data) => {
            assert!(!data.is_empty());
            // V2 responses should NOT contain MIME headers
            let response_str = String::from_utf8_lossy(&data);
            assert!(!response_str.contains("MIME-Version"));
        }
        Err(e) => {
            eprintln!("Connection failed (expected in offline tests): {e}");
        }
    }
}

#[tokio::test]
async fn test_ribbit_product_versions() {
    let products = ["agent", "wow", "wow_classic", "wow_classic_era"];
    let client = RibbitClient::new(Region::US);

    for product in products {
        println!("Testing product: {product}");
        let endpoint = Endpoint::ProductVersions(product.to_string());

        let result = client.request_raw(&endpoint).await;

        match result {
            Ok(data) => {
                assert!(!data.is_empty());
                let response_str = String::from_utf8_lossy(&data);
                // Should contain version information
                assert!(response_str.contains("Region") || response_str.contains("region"));
                println!("  Product {} returned {} bytes", product, data.len());
            }
            Err(e) => {
                eprintln!("  Product {product} failed (expected in offline tests): {e}");
            }
        }
    }
}

#[tokio::test]
async fn test_ribbit_different_regions() {
    let regions = [Region::US, Region::EU, Region::KR];

    for region in regions {
        let client = RibbitClient::new(region);
        let result = client.request_raw(&Endpoint::Summary).await;

        // Just verify we can attempt connection to different regions
        match result {
            Ok(data) => assert!(!data.is_empty()),
            Err(e) => eprintln!("Region {region} failed (expected in offline tests): {e}"),
        }
    }
}

#[tokio::test]
async fn test_ribbit_product_cdns() {
    let products = ["agent", "wow", "wow_classic", "wow_classic_era"];
    let client = RibbitClient::new(Region::US);

    for product in products {
        println!("Testing CDN for product: {product}");
        let endpoint = Endpoint::ProductCdns(product.to_string());

        let result = client.request_raw(&endpoint).await;

        match result {
            Ok(data) => {
                assert!(!data.is_empty());
                let response_str = String::from_utf8_lossy(&data);
                // Should contain CDN information
                assert!(response_str.contains("Name") || response_str.contains("Hosts"));
                println!("  Product {} CDN returned {} bytes", product, data.len());
            }
            Err(e) => {
                eprintln!("  Product {product} CDN failed (expected in offline tests): {e}");
            }
        }
    }
}

#[tokio::test]
async fn test_ribbit_product_bgdl() {
    let products = ["agent", "wow", "wow_classic", "wow_classic_era"];
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    for product in products {
        println!("Testing BGDL for product: {product}");
        let endpoint = Endpoint::ProductBgdl(product.to_string());

        let result = client.request_raw(&endpoint).await;

        match result {
            Ok(data) => {
                // BGDL endpoints often return empty or minimal data
                println!("  Product {} BGDL returned {} bytes", product, data.len());
            }
            Err(e) => {
                eprintln!("  Product {product} BGDL failed (expected in offline tests): {e}");
            }
        }
    }
}

#[tokio::test]
async fn test_ribbit_cert_endpoint() {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);
    let endpoint = Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string());

    let result = client.request_raw(&endpoint).await;

    match result {
        Ok(data) => {
            assert!(!data.is_empty());
            let response_str = String::from_utf8_lossy(&data);
            // Certificate responses should contain PEM data
            assert!(response_str.contains("-----BEGIN CERTIFICATE-----"));
        }
        Err(e) => {
            eprintln!("Connection failed (expected in offline tests): {e}");
        }
    }
}

#[tokio::test]
async fn test_ribbit_invalid_product() {
    let invalid_products = ["wow_emulation", "invalid_product", "test123"];
    let client = RibbitClient::new(Region::US);

    for product in invalid_products {
        println!("Testing invalid product: {product}");
        let endpoint = Endpoint::ProductVersions(product.to_string());

        let result = client.request_raw(&endpoint).await;

        match result {
            Ok(data) => {
                // Invalid products should return an error response or empty data
                let response_str = String::from_utf8_lossy(&data);
                println!("  Unexpected success for {}: {} bytes", product, data.len());
                // Check if it's an error response
                assert!(
                    response_str.contains("Error")
                        || response_str.contains("error")
                        || response_str.contains("404")
                        || data.is_empty(),
                    "Expected error response for invalid product {product}"
                );
            }
            Err(e) => {
                println!("  Expected failure for {product}: {e}");
            }
        }
    }
}

#[tokio::test]
async fn test_ribbit_invalid_endpoints() {
    let client = RibbitClient::new(Region::US);
    let invalid_endpoints = [
        Endpoint::Custom("invalid/endpoint".to_string()),
        Endpoint::Custom("products/wow/invalid".to_string()),
        Endpoint::Custom("does/not/exist".to_string()),
    ];

    for endpoint in invalid_endpoints {
        println!("Testing invalid endpoint: {endpoint:?}");

        let result = client.request_raw(&endpoint).await;

        match result {
            Ok(data) => {
                println!("  Unexpected success: {} bytes", data.len());
                let response_str = String::from_utf8_lossy(&data);
                // Should contain error or be empty
                assert!(
                    response_str.contains("Error")
                        || response_str.contains("error")
                        || response_str.contains("404")
                        || data.is_empty(),
                    "Expected error response for invalid endpoint"
                );
            }
            Err(e) => {
                println!("  Expected failure: {e}");
            }
        }
    }
}

#[tokio::test]
async fn test_ribbit_invalid_cert_hash() {
    let client = RibbitClient::new(Region::US);
    let invalid_hashes = [
        "0000000000000000000000000000000000000000",
        "invalid_hash",
        "deadbeef",
    ];

    for hash in invalid_hashes {
        println!("Testing invalid cert hash: {hash}");
        let endpoint = Endpoint::Cert(hash.to_string());

        let result = client.request_raw(&endpoint).await;

        match result {
            Ok(data) => {
                let response_str = String::from_utf8_lossy(&data);
                println!("  Response: {} bytes", data.len());
                // Should not contain a valid certificate
                assert!(
                    !response_str.contains("-----BEGIN CERTIFICATE-----")
                        || response_str.contains("Error")
                        || response_str.contains("error"),
                    "Should not return valid certificate for invalid hash"
                );
            }
            Err(e) => {
                println!("  Expected failure: {e}");
            }
        }
    }
}

#[tokio::test]
async fn test_ribbit_connection_failure() {
    // Since we cannot easily test connection failures without modifying the client,
    // lets at least verify that our error types work correctly
    use ribbit_client::Error;
    use std::io;

    // Test IO error conversion
    let io_error = io::Error::new(io::ErrorKind::ConnectionRefused, "Connection refused");
    let ribbit_error: Error = io_error.into();
    match ribbit_error {
        Error::Io(_) => println!("IO error correctly converted"),
        _ => panic!("Expected IO error variant"),
    }

    // Test connection failed error
    let conn_error = Error::ConnectionFailed {
        host: "invalid.host".to_string(),
        port: 1119,
    };
    assert_eq!(
        conn_error.to_string(),
        "Connection failed to invalid.host:1119"
    );
}

#[tokio::test]
async fn test_ribbit_mixed_valid_invalid() {
    // Test a mix of valid and invalid products in sequence
    let products = [
        ("agent", true),          // valid
        ("wow_emulation", false), // invalid
        ("wow", true),            // valid
        ("invalid_test", false),  // invalid
        ("wow_classic", true),    // valid
    ];

    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    for (product, should_have_data) in products {
        let endpoint = Endpoint::ProductVersions(product.to_string());
        let result = client.request_raw(&endpoint).await;

        match result {
            Ok(data) => {
                if should_have_data {
                    assert!(
                        !data.is_empty(),
                        "Expected data for valid product {product}"
                    );
                    let response = String::from_utf8_lossy(&data);
                    assert!(response.contains("|"), "Expected PSV format for {product}");
                } else {
                    assert!(
                        data.is_empty(),
                        "Expected empty response for invalid product {product}"
                    );
                }
            }
            Err(e) => {
                println!("Unexpected error for {product}: {e}");
                // Connection errors are acceptable in CI
            }
        }
    }
}

#[tokio::test]
async fn test_ribbit_v1_mime_parsing() {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    // Test with a certificate endpoint which has a simple MIME structure
    let endpoint = Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string());
    let result = client.request(&endpoint).await;

    match result {
        Ok(response) => {
            // Should have parsed MIME parts
            assert!(response.mime_parts.is_some());
            let mime_parts = response.mime_parts.unwrap();

            // Certificate data should be in the data field
            assert!(!mime_parts.data.is_empty());
            assert!(mime_parts.data.contains("-----BEGIN CERTIFICATE-----"));

            // Should have checksum
            assert!(mime_parts.checksum.is_some());
            assert_eq!(mime_parts.checksum.unwrap().len(), 64); // SHA-256 hex

            // Response data should also be populated
            assert!(response.data.is_some());
            assert!(
                response
                    .data
                    .unwrap()
                    .contains("-----BEGIN CERTIFICATE-----")
            );
        }
        Err(e) => {
            eprintln!("Connection failed (expected in offline tests): {e}");
        }
    }
}

#[tokio::test]
async fn test_ribbit_v1_multipart_mime() {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    // Test with versions endpoint which has multipart MIME with signature
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    let result = client.request(&endpoint).await;

    match result {
        Ok(response) => {
            assert!(response.mime_parts.is_some());
            let mime_parts = response.mime_parts.unwrap();

            // Should have data
            assert!(!mime_parts.data.is_empty());
            assert!(mime_parts.data.contains("Region") || mime_parts.data.contains("region"));

            // May have signature (multipart responses often do)
            // Note: signature content depends on the response

            // Should have checksum
            assert!(mime_parts.checksum.is_some());
        }
        Err(e) => {
            eprintln!("Connection failed (expected in offline tests): {e}");
        }
    }
}
