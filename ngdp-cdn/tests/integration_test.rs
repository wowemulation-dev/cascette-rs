//! Integration tests for ngdp-cdn

use ngdp_cdn::{CdnClient, CdnClientBuilderTrait as _, CdnClientTrait, Error};

#[tokio::test]
async fn test_content_not_found_errors() {
    let client = CdnClient::new().unwrap();

    // Test 1: Using request method with direct URL
    let response = client.request("http://httpbin.org/status/404").await;
    assert!(response.is_err());

    match response.unwrap_err() {
        Error::ContentNotFound { hash } => {
            // The "hash" will be "404" from the URL
            assert_eq!(hash, "404");
        }
        e => panic!("Expected ContentNotFound error, got: {e:?}"),
    }

    // Test 2: Using download method with hash
    let response = client
        .download(
            "httpbin.org",
            "status/404",
            "abcdef1234567890abcdef1234567890",
            "",
        )
        .await;

    assert!(response.is_err());
    match response.unwrap_err() {
        Error::ContentNotFound { hash } => {
            // The hash should be extracted from the URL
            assert!(hash.contains("abcdef") || hash == "404");
        }
        e => panic!("Expected ContentNotFound error, got: {e:?}"),
    }
}

#[tokio::test]
async fn test_retry_on_server_error() {
    let client = CdnClient::builder()
        .max_retries(2)
        .initial_backoff_ms(10)
        .build()
        .await
        .unwrap();

    // httpbin.org/status/500 always returns 500
    let response = client.request("http://httpbin.org/status/500").await;
    assert!(response.is_err());
}

#[tokio::test]
async fn test_custom_configuration() {
    let client = CdnClient::builder()
        .max_retries(1)
        .connect_timeout(5)
        .request_timeout(10)
        .pool_max_idle_per_host(10)
        .build()
        .await
        .unwrap();

    // Should succeed with custom config
    assert!(
        client
            .request("http://httpbin.org/status/200")
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn test_concurrent_requests() {
    let client = CdnClient::new().unwrap();

    let handles: Vec<_> = (0..5)
        .map(|i| {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .request(&format!("http://httpbin.org/status/{}", 200 + i))
                    .await
            })
        })
        .collect();

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }
}

#[tokio::test]
async fn test_rate_limit_handling() {
    let client = CdnClient::builder()
        .max_retries(1)
        .initial_backoff_ms(10)
        .build()
        .await
        .unwrap();

    // httpbin.org/status/429 returns 429 Too Many Requests
    let response = client.request("http://httpbin.org/status/429").await;
    assert!(response.is_err());
}
