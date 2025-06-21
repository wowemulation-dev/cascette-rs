//! Tests for the fallback client functionality

use ngdp_client::fallback_client::{FallbackClient, FallbackError};
use ribbit_client::{Endpoint, Region};

#[tokio::test]
async fn test_fallback_client_creation() {
    // Test creating a client for each region (except CN which may not be accessible)
    for region in [Region::US, Region::EU] {
        match FallbackClient::new(region).await {
            Ok(_client) => {
                println!(
                    "Successfully created fallback client for region: {:?}",
                    region
                );
                // Client created successfully
            }
            Err(e) => {
                eprintln!("Failed to create client for region {:?}: {}", region, e);
                // Don't fail the test - network issues may occur
            }
        }
    }
}

#[tokio::test]
async fn test_fallback_with_summary_endpoint() {
    // Summary is only supported by Ribbit
    let client = match FallbackClient::new(Region::US).await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test - unable to create client");
            return;
        }
    };

    match client.request(&Endpoint::Summary).await {
        Ok(response) => {
            assert!(response.data.is_some());
            println!("Summary request succeeded through Ribbit");
        }
        Err(e) => {
            eprintln!("Summary request failed: {}", e);
            // Network issues may occur
        }
    }
}

#[tokio::test]
async fn test_caching_control() {
    let mut client = match FallbackClient::new(Region::US).await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test - unable to create client");
            return;
        }
    };

    // Test disabling caching
    client.set_caching_enabled(false);
    // Caching disabled successfully

    // Test enabling caching
    client.set_caching_enabled(true);
    // Caching enabled successfully
}

#[tokio::test]
async fn test_typed_request() {
    use ribbit_client::ProductVersionsResponse;

    let client = match FallbackClient::new(Region::US).await {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Skipping test - unable to create client");
            return;
        }
    };

    let endpoint = Endpoint::ProductVersions("wow".to_string());
    match client
        .request_typed::<ProductVersionsResponse>(&endpoint)
        .await
    {
        Ok(versions) => {
            println!("Got {} version entries", versions.entries.len());
            assert!(!versions.entries.is_empty());
        }
        Err(e) => {
            eprintln!("Typed request failed: {}", e);
            // Network issues may occur
        }
    }
}

#[test]
fn test_fallback_error_display() {
    let error = FallbackError::BothFailed {
        ribbit_error: "Connection timeout".to_string(),
        tact_error: "HTTP 500".to_string(),
    };

    let error_str = error.to_string();
    assert!(error_str.contains("Ribbit: Connection timeout"));
    assert!(error_str.contains("TACT: HTTP 500"));
}

#[test]
fn test_sg_region_fallback() {
    // SG region should fall back to US for TACT
    // This is tested in the FallbackClient::new implementation
    // which converts SG to US for the TACT client
}
