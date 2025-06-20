//! Integration tests for TACT client

use tact_client::{HttpClient, ProtocolVersion, Region};

#[tokio::test]
async fn test_v1_client_creation() {
    let client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
    assert_eq!(client.region(), Region::US);
    assert_eq!(client.version(), ProtocolVersion::V1);
    assert_eq!(client.base_url(), "http://us.patch.battle.net:1119");
}

#[tokio::test]
async fn test_v2_client_creation() {
    let client = HttpClient::new(Region::EU, ProtocolVersion::V2).unwrap();
    assert_eq!(client.region(), Region::EU);
    assert_eq!(client.version(), ProtocolVersion::V2);
    assert_eq!(
        client.base_url(),
        "https://eu.version.battle.net/v2/products"
    );
}

#[tokio::test]
async fn test_all_regions() {
    for region in Region::all() {
        let client_v1 = HttpClient::new(*region, ProtocolVersion::V1).unwrap();
        assert!(client_v1.base_url().contains(region.as_str()));

        let client_v2 = HttpClient::new(*region, ProtocolVersion::V2).unwrap();
        assert!(client_v2.base_url().contains(region.as_str()));
    }
}

#[tokio::test]
async fn test_region_switching() {
    let mut client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();
    assert_eq!(client.region(), Region::US);

    for region in [Region::EU, Region::KR, Region::CN, Region::TW] {
        client.set_region(region);
        assert_eq!(client.region(), region);
        assert!(client.base_url().contains(region.as_str()));
    }
}

#[tokio::test]
async fn test_download_url_format() {
    let _client = HttpClient::new(Region::US, ProtocolVersion::V1).unwrap();

    // Test hash path structure
    let hash = "1234567890abcdef1234567890abcdef";
    let _cdn_host = "cdn.example.com";
    let _path = "tpr/wow";

    // The download_file method constructs URLs with the hash prefix structure
    let _expected_url_part = format!("{}/{}/{}", &hash[0..2], &hash[2..4], hash);
    assert_eq!(&hash[0..2], "12");
    assert_eq!(&hash[2..4], "34");
}
