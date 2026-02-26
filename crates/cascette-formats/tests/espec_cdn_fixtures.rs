#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Integration tests for ESpec parsing using real CDN data
//!
//! Tests parse real ESpec strings extracted from WoW CDN encoding files
//! via cascette-py. Covers Classic Era, Classic, and Retail patterns
//! including 4-byte and 8-byte IVs in encrypted blocks.

use cascette_formats::espec::{ESpec, ESpecError};
use std::path::Path;

fn fixtures_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("test_fixtures/espec")
        .leak()
}

/// Load all representative ESpec strings from the fixture file
fn load_representative_especs() -> Vec<String> {
    let path = fixtures_dir().join("representative_especs.json");
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e));
    let json: serde_json::Value =
        serde_json::from_str(&data).expect("Failed to parse fixture JSON");
    json["all_representative"]
        .as_array()
        .expect("all_representative should be an array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("each entry should be a string")
                .to_string()
        })
        .collect()
}

/// Load Classic Era ESpec strings from the fixture file
fn load_classic_era_especs() -> Vec<String> {
    let path = fixtures_dir().join("wow_classic_era_especs.json");
    let data = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("Failed to read fixture {}: {}", path.display(), e));
    let json: serde_json::Value =
        serde_json::from_str(&data).expect("Failed to parse fixture JSON");
    json["especs"]
        .as_array()
        .expect("especs should be an array")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("each entry should be a string")
                .to_string()
        })
        .collect()
}

// --- Parse all representative ESpecs ---

#[test]
fn espec_cdn_parse_all_representative() {
    let especs = load_representative_especs();
    assert!(!especs.is_empty(), "Should have representative ESpecs");

    let mut failures = Vec::new();
    for input in &especs {
        if let Err(e) = ESpec::parse(input) {
            failures.push(format!("  {input}: {e}"));
        }
    }

    assert!(
        failures.is_empty(),
        "Failed to parse {} ESpecs:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn espec_cdn_parse_all_classic_era() {
    let especs = load_classic_era_especs();
    assert_eq!(especs.len(), 56, "Classic Era should have 56 ESpecs");

    let mut failures = Vec::new();
    for input in &especs {
        if let Err(e) = ESpec::parse(input) {
            failures.push(format!("  {input}: {e}"));
        }
    }

    assert!(
        failures.is_empty(),
        "Failed to parse {} Classic Era ESpecs:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// --- Round-trip tests ---

#[test]
fn espec_cdn_round_trip_representative() {
    let especs = load_representative_especs();

    for input in &especs {
        let parsed =
            ESpec::parse(input).unwrap_or_else(|e| panic!("Parse failed for {input}: {e}"));
        let output = parsed.to_string();
        let reparsed = ESpec::parse(&output)
            .unwrap_or_else(|e| panic!("Re-parse failed for {output} (from {input}): {e}"));
        assert_eq!(
            parsed, reparsed,
            "Round-trip mismatch for {input} -> {output}"
        );
    }
}

#[test]
fn espec_cdn_round_trip_classic_era() {
    let especs = load_classic_era_especs();

    for input in &especs {
        let parsed =
            ESpec::parse(input).unwrap_or_else(|e| panic!("Parse failed for {input}: {e}"));
        let output = parsed.to_string();
        let reparsed = ESpec::parse(&output)
            .unwrap_or_else(|e| panic!("Re-parse failed for {output} (from {input}): {e}"));
        assert_eq!(
            parsed, reparsed,
            "Round-trip mismatch for {input} -> {output}"
        );
    }
}

// --- 4-byte IV tests ---

#[test]
fn espec_cdn_encrypted_iv4() {
    // Classic Era pattern: 4-byte IV (8 hex chars)
    let input = "b:{256K*=e:{DFEBCAC54990E8C3,42E93D9C,z}}";
    let parsed = ESpec::parse(input).expect("4-byte IV should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => {
            assert_eq!(chunks.len(), 1);
            match &chunks[0].spec {
                ESpec::Encrypted { key, iv, spec } => {
                    assert_eq!(key, "DFEBCAC54990E8C3");
                    assert_eq!(iv.len(), 4);
                    assert_eq!(iv, &[0x42, 0xE9, 0x3D, 0x9C]);
                    assert!(matches!(**spec, ESpec::ZLib { .. }));
                }
                other => panic!("Expected Encrypted, got {other:?}"),
            }
        }
        other => panic!("Expected BlockTable, got {other:?}"),
    }
}

// --- 8-byte IV tests ---

#[test]
fn espec_cdn_encrypted_iv8() {
    // Retail pattern: 8-byte IV (16 hex chars)
    let input = "b:{256K*=e:{000684749764DCBE,5379955308151E04,n}}";
    let parsed = ESpec::parse(input).expect("8-byte IV should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => {
            assert_eq!(chunks.len(), 1);
            match &chunks[0].spec {
                ESpec::Encrypted { key, iv, spec } => {
                    assert_eq!(key, "000684749764DCBE");
                    assert_eq!(iv.len(), 8);
                    assert_eq!(iv, &[0x53, 0x79, 0x95, 0x53, 0x08, 0x15, 0x1E, 0x04]);
                    assert_eq!(**spec, ESpec::None);
                }
                other => panic!("Expected Encrypted, got {other:?}"),
            }
        }
        other => panic!("Expected BlockTable, got {other:?}"),
    }
}

#[test]
fn espec_cdn_encrypted_iv8_with_zlib() {
    // Retail pattern: 8-byte IV with zlib compression
    let input = "b:{256K*=e:{059B862A6E78A076,32A6F926AB325B07,z}}";
    let parsed = ESpec::parse(input).expect("8-byte IV with zlib should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => match &chunks[0].spec {
            ESpec::Encrypted { iv, spec, .. } => {
                assert_eq!(iv.len(), 8);
                assert!(matches!(**spec, ESpec::ZLib { .. }));
            }
            other => panic!("Expected Encrypted, got {other:?}"),
        },
        other => panic!("Expected BlockTable, got {other:?}"),
    }
}

// --- Mixed encrypted + plain blocks ---

#[test]
fn espec_cdn_mixed_encrypted_plain() {
    // Classic pattern: plain zlib blocks followed by encrypted blocks
    let input = "b:{1008=z,24524=z,59=e:{57A612DDA061E38E,b0ea3333,z}}";
    let parsed = ESpec::parse(input).expect("Mixed encrypted/plain should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => {
            assert_eq!(chunks.len(), 3);
            // First two are zlib
            assert!(matches!(chunks[0].spec, ESpec::ZLib { .. }));
            assert!(matches!(chunks[1].spec, ESpec::ZLib { .. }));
            // Third is encrypted
            assert!(matches!(chunks[2].spec, ESpec::Encrypted { .. }));
        }
        other => panic!("Expected BlockTable, got {other:?}"),
    }
}

#[test]
fn espec_cdn_multi_encrypted_shared_iv() {
    // Classic pattern: multiple encrypted blocks with shared IV, different keys
    let input = "b:{1028=z,256K=z,14566=z,120=e:{7C8F55CF7563B121,99ebd8c2,z},93=e:{368D53C220AE5525,99ebd8c2,z},66=e:{5EBB8AFB273BD1EA,99ebd8c2,z}}";
    let parsed = ESpec::parse(input).expect("Multi-encrypted should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => {
            assert_eq!(chunks.len(), 6);
            // Verify all encrypted blocks share the same IV
            let encrypted_ivs: Vec<&Vec<u8>> = chunks
                .iter()
                .filter_map(|c| match &c.spec {
                    ESpec::Encrypted { iv, .. } => Some(iv),
                    _ => None,
                })
                .collect();
            assert_eq!(encrypted_ivs.len(), 3);
            assert!(encrypted_ivs.windows(2).all(|w| w[0] == w[1]));
        }
        other => panic!("Expected BlockTable, got {other:?}"),
    }
}

// --- Shorthand form ---

#[test]
fn espec_cdn_shorthand_without_braces() {
    // CDN shorthand: b:256K*=z (no braces around single size-spec chunk)
    let input = "b:256K*=z";
    let parsed = ESpec::parse(input).expect("Shorthand form should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => {
            assert_eq!(chunks.len(), 1);
            assert_eq!(
                chunks[0].size_spec,
                Some(cascette_formats::espec::BlockSizeSpec {
                    size: 256 * 1024,
                    count: None,
                })
            );
            assert!(matches!(chunks[0].spec, ESpec::ZLib { .. }));
        }
        other => panic!("Expected BlockTable, got {other:?}"),
    }

    // Round-trip normalizes to braced form. The `*` without a count is
    // equivalent to a single block (count: None), so Display omits it.
    let output = parsed.to_string();
    assert_eq!(output, "b:{256K=z}");
}

// --- MPQ variant ---

#[test]
fn espec_cdn_mpq_variant() {
    let input = "b:{16K*=z:{6,mpq}}";
    let parsed = ESpec::parse(input).expect("MPQ variant should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => {
            assert_eq!(chunks.len(), 1);
            match &chunks[0].spec {
                ESpec::ZLib {
                    level,
                    variant,
                    window_bits,
                } => {
                    assert_eq!(*level, Some(6));
                    assert_eq!(*variant, Some(cascette_formats::espec::ZLibVariant::MPQ));
                    assert_eq!(*window_bits, None);
                }
                other => panic!("Expected ZLib, got {other:?}"),
            }
        }
        other => panic!("Expected BlockTable, got {other:?}"),
    }
}

// --- Validation tests for new error variants ---

#[test]
fn espec_cdn_multiple_variable_blocks_rejected() {
    // Agent.exe rejects multiple variable blocks
    let result = ESpec::parse("b:{*=n,*=z}");
    assert!(
        matches!(result, Err(ESpecError::MultipleVariableBlocks)),
        "Should reject multiple variable blocks, got: {result:?}"
    );
}

#[test]
fn espec_cdn_iv_length_validation() {
    // 0-byte IV should be rejected
    let result = ESpec::parse("e:{0123456789ABCDEF,,z}");
    assert!(result.is_err(), "0-byte IV should be rejected");

    // 1-byte IV is valid
    let parsed = ESpec::parse("e:{0123456789ABCDEF,AB,z}");
    assert!(parsed.is_ok(), "1-byte IV should be accepted");
    match &parsed.unwrap() {
        ESpec::Encrypted { iv, .. } => assert_eq!(iv.len(), 1),
        _ => panic!("Expected Encrypted"),
    }

    // 4-byte IV is valid
    let parsed = ESpec::parse("e:{0123456789ABCDEF,AABBCCDD,z}");
    assert!(parsed.is_ok(), "4-byte IV should be accepted");

    // 8-byte IV is valid
    let parsed = ESpec::parse("e:{0123456789ABCDEF,AABBCCDDEEFF0011,z}");
    assert!(parsed.is_ok(), "8-byte IV should be accepted");
    match &parsed.unwrap() {
        ESpec::Encrypted { iv, .. } => assert_eq!(iv.len(), 8),
        _ => panic!("Expected Encrypted"),
    }

    // 9-byte IV should be rejected
    let result = ESpec::parse("e:{0123456789ABCDEF,AABBCCDDEEFF001122,z}");
    assert!(
        matches!(result, Err(ESpecError::InvalidIvLength(9))),
        "9-byte IV should be rejected, got: {result:?}"
    );
}

// --- Retail mixed IV8 with plain blocks ---

#[test]
fn espec_cdn_retail_mixed_iv8() {
    // Retail pattern: zlib + encrypted IV8 + trailing zlib
    let input = "b:{2K=z,512K*103=e:{4366C29B8DEB645F,2277C4F64A1D302E,n},158898=e:{4366C29B8DEB645F,2277C4F64A1D302E,n},105240=z}";
    let parsed = ESpec::parse(input).expect("Retail mixed IV8 should parse");

    match &parsed {
        ESpec::BlockTable { chunks } => {
            assert_eq!(chunks.len(), 4);
            // First is zlib
            assert!(matches!(chunks[0].spec, ESpec::ZLib { .. }));
            // Middle two are encrypted with 8-byte IV
            for chunk in &chunks[1..3] {
                match &chunk.spec {
                    ESpec::Encrypted { iv, .. } => assert_eq!(iv.len(), 8),
                    other => panic!("Expected Encrypted, got {other:?}"),
                }
            }
            // Last is zlib
            assert!(matches!(chunks[3].spec, ESpec::ZLib { .. }));
        }
        other => panic!("Expected BlockTable, got {other:?}"),
    }
}
