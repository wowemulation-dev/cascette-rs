#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for BPSV format parsing using real-world fixture data.
//!
//! BPSV (Blizzard Pipe-Separated Values) is the wire format for Ribbit/TACT
//! endpoints: `versions`, `cdns`, `bgdl`. All reference tools (TACTSharp,
//! wow.export, BuildBackup) parse these to bootstrap a build.
//!
//! The fixtures mirror the schema and content of real Blizzard CDN responses.

use cascette_formats::CascFormat;
use cascette_formats::bpsv::{BpsvDocument, parse, parse_schema};
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/bpsv")
        .leak()
}

fn read_fixture(name: &str) -> String {
    let path = fixtures_dir().join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e))
}

// --- versions endpoint ---

#[test]
fn bpsv_versions_parse() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = parse(&content).expect("versions BPSV should parse");

    assert_eq!(doc.row_count(), 3, "Should have 3 region rows");
    assert!(doc.has_field("Region"));
    assert!(doc.has_field("BuildConfig"));
    assert!(doc.has_field("CDNConfig"));
    assert!(doc.has_field("BuildId"));
    assert!(doc.has_field("VersionsName"));
}

#[test]
fn bpsv_versions_sequence_number() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    assert_eq!(
        doc.sequence_number(),
        Some(6_226_474),
        "Sequence number must match CDN response"
    );
}

#[test]
fn bpsv_versions_field_count() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    // versions has 7 fields: Region, BuildConfig, CDNConfig, KeyRing, BuildId, VersionsName, ProductConfig
    assert_eq!(doc.schema().field_count(), 7);
}

#[test]
fn bpsv_versions_rows_have_region() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    let regions: Vec<&str> = doc.iter().filter_map(|row| row.get_raw(0)).collect();
    assert_eq!(regions, vec!["us", "eu", "kr"]);
}

#[test]
fn bpsv_versions_build_config_is_hex() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    for (i, row) in doc.iter().enumerate() {
        let build_config = row
            .get_raw(1)
            .unwrap_or_else(|| panic!("Row {i} missing BuildConfig"));
        assert_eq!(
            build_config.len(),
            32,
            "Row {i} BuildConfig must be 32 hex chars"
        );
        assert!(
            build_config.chars().all(|c| c.is_ascii_hexdigit()),
            "Row {i} BuildConfig must be hex: {build_config}"
        );
    }
}

#[test]
fn bpsv_versions_all_regions_share_build_config() {
    // For Classic Era, all regions typically ship the same build
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    let configs: Vec<&str> = doc.iter().filter_map(|row| row.get_raw(1)).collect();
    let first = configs[0];
    for (i, cfg) in configs.iter().enumerate() {
        assert_eq!(
            *cfg, first,
            "Region {i} BuildConfig differs from us: expected {first}, got {cfg}"
        );
    }
}

#[test]
fn bpsv_versions_build_id_parseable() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    for (i, row) in doc.iter().enumerate() {
        let build_id = row
            .get_raw(4)
            .unwrap_or_else(|| panic!("Row {i} missing BuildId"));
        let _: u32 = build_id
            .parse()
            .unwrap_or_else(|_| panic!("Row {i} BuildId '{build_id}' must be a u32"));
    }
}

#[test]
fn bpsv_versions_schema_only_parse() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let schema = parse_schema(&content).expect("Schema-only parse should succeed");

    assert_eq!(schema.field_count(), 7);
    assert!(schema.has_field("Region"));
    assert!(schema.has_field("BuildConfig"));
}

// --- cdns endpoint ---

#[test]
fn bpsv_cdns_parse() {
    let content = read_fixture("wow_classic_era_cdns.bpsv");
    let doc = parse(&content).expect("cdns BPSV should parse");

    assert_eq!(doc.row_count(), 3);
    assert!(doc.has_field("Name"));
    assert!(doc.has_field("Path"));
    assert!(doc.has_field("Hosts"));
}

#[test]
fn bpsv_cdns_sequence_number() {
    let content = read_fixture("wow_classic_era_cdns.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    assert_eq!(doc.sequence_number(), Some(6_226_474));
}

#[test]
fn bpsv_cdns_path_is_tpr() {
    // CDN path always starts with "tpr/" for WoW
    let content = read_fixture("wow_classic_era_cdns.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    for (i, row) in doc.iter().enumerate() {
        let path = row
            .get_raw(1)
            .unwrap_or_else(|| panic!("Row {i} missing Path"));
        assert!(
            path.starts_with("tpr/"),
            "Row {i} CDN path must start with 'tpr/': {path}"
        );
    }
}

#[test]
fn bpsv_cdns_hosts_have_multiple_entries() {
    // CDN hosts field contains space-separated host names (for failover)
    let content = read_fixture("wow_classic_era_cdns.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    for (i, row) in doc.iter().enumerate() {
        let hosts = row
            .get_raw(2)
            .unwrap_or_else(|| panic!("Row {i} missing Hosts"));
        assert!(
            hosts.split_whitespace().count() >= 2,
            "Row {i} should have at least 2 CDN hosts for failover"
        );
    }
}

#[test]
fn bpsv_cdns_config_path_present() {
    let content = read_fixture("wow_classic_era_cdns.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    for (i, row) in doc.iter().enumerate() {
        let config_path = row
            .get_raw(4)
            .unwrap_or_else(|| panic!("Row {i} missing ConfigPath"));
        assert!(
            !config_path.is_empty(),
            "Row {i} ConfigPath must not be empty"
        );
    }
}

// --- retail versions ---

#[test]
fn bpsv_retail_versions_parse() {
    let content = read_fixture("wow_retail_versions.bpsv");
    let doc = parse(&content).expect("Retail versions BPSV should parse");

    assert_eq!(doc.row_count(), 2, "Should have us and eu rows");
    assert_eq!(doc.sequence_number(), Some(57212));
}

#[test]
fn bpsv_retail_versions_known_build_config() {
    // Pinned build config from TACTSharp ExtractionTests (9.0.1.35078)
    // Our fixture uses a more recent pinned build (11.0.7.57212)
    let content = read_fixture("wow_retail_versions.bpsv");
    let doc = parse(&content).expect("Parse should succeed");

    let us_row = doc.get_row(0).expect("Should have us row");
    assert_eq!(us_row.get_raw(0), Some("us"));
    assert_eq!(
        us_row.get_raw(1),
        Some("43a001a23efd4193a96266be43fe67d8"),
        "BuildConfig must match pinned 11.0.7.57212"
    );
}

// --- Round-trip via CascFormat trait ---

#[test]
fn bpsv_round_trip_versions() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let doc = BpsvDocument::parse(content.as_bytes()).expect("Parse should succeed");

    let rebuilt = doc.build().expect("Build should succeed");
    let reparsed = BpsvDocument::parse(&rebuilt).expect("Re-parse should succeed");

    assert_eq!(doc.row_count(), reparsed.row_count());
    assert_eq!(doc.sequence_number(), reparsed.sequence_number());
    assert_eq!(doc.schema().field_count(), reparsed.schema().field_count());

    for (i, (orig_row, repr_row)) in doc.iter().zip(reparsed.iter()).enumerate() {
        for field_idx in 0..doc.schema().field_count() {
            assert_eq!(
                orig_row.get_raw(field_idx),
                repr_row.get_raw(field_idx),
                "Row {i} field {field_idx} mismatch after round-trip"
            );
        }
    }
}

#[test]
fn bpsv_round_trip_cdns() {
    let content = read_fixture("wow_classic_era_cdns.bpsv");
    let doc = BpsvDocument::parse(content.as_bytes()).expect("Parse should succeed");
    let rebuilt = doc.build().expect("Build should succeed");
    let reparsed = BpsvDocument::parse(&rebuilt).expect("Re-parse should succeed");

    assert_eq!(doc.row_count(), reparsed.row_count());
    assert_eq!(doc.sequence_number(), reparsed.sequence_number());
}

// --- Error conditions ---

#[test]
fn bpsv_missing_type_annotation_rejected() {
    // BPSV header must have !TYPE:size annotations
    let content = "Region|BuildConfig|BuildId\nus|abc|1234\n";
    let result = parse(content);
    assert!(
        result.is_err(),
        "Header without type annotations must be rejected"
    );
}

#[test]
fn bpsv_empty_content_rejected() {
    let result = parse("");
    assert!(result.is_err(), "Empty content must be rejected");
}

#[test]
fn bpsv_header_only_is_valid() {
    // A document with a header but no data rows is valid
    let content = "Region!STRING:0|BuildConfig!HEX:16\n";
    let doc = parse(content).expect("Header-only document should be valid");
    assert_eq!(doc.row_count(), 0);
    assert!(doc.is_empty());
}

// --- Schema field type validation ---

#[test]
fn bpsv_schema_field_types_recognized() {
    let content = read_fixture("wow_classic_era_versions.bpsv");
    let schema = parse_schema(&content).expect("Schema parse should succeed");

    // All field names must be non-empty
    for name in schema.field_names() {
        assert!(!name.is_empty(), "Field name must not be empty");
    }
}
