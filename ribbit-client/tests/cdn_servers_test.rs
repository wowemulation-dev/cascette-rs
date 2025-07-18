//! Tests for CDN servers field parsing

use ngdp_bpsv::BpsvDocument;
use ribbit_client::response_types::{ProductCdnsResponse, TypedBpsvResponse};

#[test]
fn test_cdn_servers_parsing() {
    // Create a test BPSV document with servers field
    let content = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com|http://blzddist1-a.akamaihd.net/?maxhosts=4 http://level3.blizzard.com/?maxhosts=4|tpr/configs/data
eu|tpr/wow|eu.cdn.blizzard.com level3.blizzard.com|http://eu.cdn.blizzard.com/?maxhosts=4 https://blzddist1-a.akamaihd.net/?fallback=1&maxhosts=4|tpr/configs/data"#;

    let doc = BpsvDocument::parse(content).expect("Failed to parse BPSV");
    let cdns = ProductCdnsResponse::from_bpsv(&doc).expect("Failed to parse CDN response");

    assert_eq!(cdns.entries.len(), 2);

    // Check US CDN entry
    let us_cdn = &cdns.entries[0];
    assert_eq!(us_cdn.name, "us");
    assert_eq!(us_cdn.path, "tpr/wow");
    assert_eq!(us_cdn.hosts.len(), 2);
    assert_eq!(us_cdn.hosts[0], "blzddist1-a.akamaihd.net");
    assert_eq!(us_cdn.hosts[1], "level3.blizzard.com");

    // Check servers are now parsed as a vector
    assert_eq!(us_cdn.servers.len(), 2);
    assert_eq!(
        us_cdn.servers[0],
        "http://blzddist1-a.akamaihd.net/?maxhosts=4"
    );
    assert_eq!(us_cdn.servers[1], "http://level3.blizzard.com/?maxhosts=4");

    // Check EU CDN entry
    let eu_cdn = &cdns.entries[1];
    assert_eq!(eu_cdn.servers.len(), 2);
    assert_eq!(eu_cdn.servers[0], "http://eu.cdn.blizzard.com/?maxhosts=4");
    assert_eq!(
        eu_cdn.servers[1],
        "https://blzddist1-a.akamaihd.net/?fallback=1&maxhosts=4"
    );
}

#[test]
fn test_cdn_empty_servers() {
    // Test with empty servers field
    let content = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|host1.com host2.com||tpr/configs/data"#;

    let doc = BpsvDocument::parse(content).expect("Failed to parse BPSV");
    let cdns = ProductCdnsResponse::from_bpsv(&doc).expect("Failed to parse CDN response");

    assert_eq!(cdns.entries.len(), 1);

    let entry = &cdns.entries[0];
    assert_eq!(entry.hosts, vec!["host1.com", "host2.com"]);
    assert_eq!(entry.servers, Vec::<String>::new()); // Should be empty vector
}

#[test]
fn test_cdn_servers_with_complex_urls() {
    // Test with complex server URLs containing multiple query parameters
    let content = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
cn|tpr/wow|cdn.blizzard.cn|https://cdn.blizzard.cn/?maxhosts=8&fallback=1&region=cn https://backup.cdn.cn/?priority=2|tpr/configs/data"#;

    let doc = BpsvDocument::parse(content).expect("Failed to parse BPSV");
    let cdns = ProductCdnsResponse::from_bpsv(&doc).expect("Failed to parse CDN response");

    let cn_cdn = &cdns.entries[0];
    assert_eq!(cn_cdn.servers.len(), 2);
    assert_eq!(
        cn_cdn.servers[0],
        "https://cdn.blizzard.cn/?maxhosts=8&fallback=1&region=cn"
    );
    assert_eq!(cn_cdn.servers[1], "https://backup.cdn.cn/?priority=2");
}

#[test]
fn test_consistency_with_tact_client() {
    // This test ensures that the Ribbit client parses servers the same way as TACT client
    let content = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|level3.blizzard.com us.cdn.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://us.cdn.blizzard.com/?maxhosts=4|tpr/configs/data"#;

    let doc = BpsvDocument::parse(content).expect("Failed to parse BPSV");
    let cdns = ProductCdnsResponse::from_bpsv(&doc).expect("Failed to parse CDN response");

    let entry = &cdns.entries[0];

    // Both hosts and servers should be vectors
    assert_eq!(entry.hosts.len(), 2);
    assert_eq!(entry.servers.len(), 2);

    // Verify the data matches what we expect
    assert_eq!(entry.hosts[0], "level3.blizzard.com");
    assert_eq!(entry.hosts[1], "us.cdn.blizzard.com");
    assert_eq!(entry.servers[0], "http://level3.blizzard.com/?maxhosts=4");
    assert_eq!(entry.servers[1], "http://us.cdn.blizzard.com/?maxhosts=4");
}
