//! Integration tests for typed responses

use ribbit_client::{
    Endpoint, ProductCdnsResponse, ProductVersionsResponse, Region, RibbitClient, SummaryResponse,
};

#[tokio::test]
async fn test_typed_product_versions() {
    let client = RibbitClient::new(Region::US);

    // Test the generic typed request
    let versions: ProductVersionsResponse = client
        .request_typed(&Endpoint::ProductVersions("wow".to_string()))
        .await
        .expect("Failed to get WoW versions");

    assert!(!versions.entries.is_empty());
    assert!(versions.sequence_number.is_some());

    // Check that all entries have required fields
    for entry in &versions.entries {
        assert!(!entry.region.is_empty());
        assert!(!entry.versions_name.is_empty());
        assert!(entry.build_id > 0);
        assert!(!entry.build_config.is_empty());
        assert!(!entry.cdn_config.is_empty());
        assert!(!entry.product_config.is_empty());
    }

    // Test convenience method
    let versions2 = client
        .get_product_versions("wow")
        .await
        .expect("Failed with convenience method");

    assert_eq!(versions.entries.len(), versions2.entries.len());
}

#[tokio::test]
async fn test_typed_product_cdns() {
    let client = RibbitClient::new(Region::US);

    let cdns: ProductCdnsResponse = client
        .request_typed(&Endpoint::ProductCdns("wow".to_string()))
        .await
        .expect("Failed to get WoW CDNs");

    assert!(!cdns.entries.is_empty());
    assert!(cdns.sequence_number.is_some());

    // Check CDN entries
    for entry in &cdns.entries {
        assert!(!entry.name.is_empty());
        assert!(!entry.path.is_empty());
        assert!(!entry.hosts.is_empty());
        assert!(!entry.config_path.is_empty());
    }

    // Test all_hosts method
    let all_hosts = cdns.all_hosts();
    assert!(!all_hosts.is_empty());
}

#[tokio::test]
async fn test_typed_summary() {
    let client = RibbitClient::new(Region::US);

    let summary: SummaryResponse = client
        .request_typed(&Endpoint::Summary)
        .await
        .expect("Failed to get summary");

    assert!(!summary.products.is_empty());
    assert!(summary.sequence_number.is_some());

    // Check for known products
    assert!(summary.get_product("wow").is_some());
    assert!(summary.get_product("d3").is_some());

    // Test product_codes method
    let codes = summary.product_codes();
    assert!(codes.contains(&"wow"));
}

#[tokio::test]
async fn test_response_convenience_methods() {
    let client = RibbitClient::new(Region::US);

    // Test raw response methods
    let response = client
        .request(&Endpoint::ProductVersions("wow".to_string()))
        .await
        .expect("Failed to get response");

    // Test as_text
    assert!(response.as_text().is_some());
    let text = response.as_text().unwrap();
    assert!(text.contains("Region"));
    assert!(text.contains("BuildId"));

    // Test Display impl (like Ribbit.NET's ToString)
    let display_text = response.to_string();
    assert_eq!(display_text, text);

    // Test as_bpsv
    let bpsv = response.as_bpsv().expect("Failed to parse as BPSV");
    assert!(!bpsv.schema().field_names().is_empty());
    assert!(!bpsv.rows().is_empty());
}

#[tokio::test]
async fn test_version_entry_methods() {
    let client = RibbitClient::new(Region::US);

    let versions = client
        .get_product_versions("wow")
        .await
        .expect("Failed to get versions");

    // Test get_region method
    if let Some(us_version) = versions.get_region("us") {
        assert_eq!(us_version.region, "us");
    }

    // Test build_ids method
    let build_ids = versions.build_ids();
    assert!(!build_ids.is_empty());
    // Build IDs should be sorted and unique
    for i in 1..build_ids.len() {
        assert!(build_ids[i] >= build_ids[i - 1]);
        assert!(build_ids[i] != build_ids[i - 1]);
    }
}

#[tokio::test]
async fn test_error_handling() {
    let client = RibbitClient::new(Region::US);

    // Test with invalid product
    let result = client
        .get_product_versions("nonexistent_product_12345")
        .await;
    // This might succeed but return empty data, or might fail
    // Either way, we're testing that it doesn't panic

    if let Ok(versions) = result {
        // If it succeeds, it might just be empty
        println!(
            "Got {} entries for nonexistent product",
            versions.entries.len()
        );
    } else {
        // Expected case - error
        println!("Got expected error for nonexistent product");
    }
}
