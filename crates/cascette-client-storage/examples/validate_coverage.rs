#![allow(clippy::expect_used, clippy::panic, clippy::cast_precision_loss)]

//! CASC content coverage validator.
//!
//! Walks the encoding file to check whether every encoding key (EKey)
//! is resolvable through either the local IDX or CDN archive indices.
//! Reports coverage statistics and lists missing entries.
//!
//! This answers the question: "Can the WoW client find everything it
//! needs in this installation?"
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow \
//!     cargo run -p cascette-client-storage --features local-install \
//!     --example validate_coverage

mod common;

use std::collections::HashSet;
use std::io::BufReader;
use std::path::Path;

use cascette_client_storage::Installation;
use cascette_crypto::EncodingKey;
use cascette_formats::archive::ArchiveIndex;
use cascette_formats::config::{BuildConfig, CdnConfig};
use cascette_formats::encoding::EncodingFile;

const BUILD_CONFIG: &str = "2c915a9a226a3f35af6c65fcc7b6ca4a";
const ENCODING_EKEY: &str = "59cad02d7dc0187413ae485a766f851b";

/// Load all CDN archive indices from Data/indices/ and collect all ekeys.
fn load_cdn_index_ekeys(indices_dir: &Path) -> HashSet<Vec<u8>> {
    let mut cdn_ekeys = HashSet::new();

    let Ok(entries) = std::fs::read_dir(indices_dir) else {
        eprintln!("  Warning: cannot read indices dir");
        return cdn_ekeys;
    };

    let mut index_count = 0u32;
    let mut entry_count = 0u64;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("index") {
            continue;
        }

        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut reader = BufReader::new(file);

        if let Ok(index) = ArchiveIndex::parse(&mut reader) {
            for e in &index.entries {
                if !e.is_zero() {
                    cdn_ekeys.insert(e.encoding_key.clone());
                    entry_count += 1;
                }
            }
            index_count += 1;
        }
    }

    println!("  Loaded {index_count} CDN archive indices with {entry_count} entries");
    cdn_ekeys
}

#[tokio::main]
async fn main() {
    let wow = common::wow_path();

    // Step 1: Open installation and load local indices
    println!("=== Step 1: Open installation ===");
    let install = Installation::open(wow.join("Data")).expect("open installation");
    install.initialize().await.expect("initialize");

    let stats = install.stats().await;
    println!(
        "  Local IDX: {} entries across {} files, {} archives",
        stats.index_entries, stats.index_files, stats.archive_files
    );

    // Step 2: Read encoding file from local CASC
    println!("\n=== Step 2: Load encoding file ===");
    let ekey_bytes: [u8; 16] = hex::decode(ENCODING_EKEY)
        .expect("valid hex")
        .try_into()
        .expect("16 bytes");
    let ekey = EncodingKey::from_bytes(ekey_bytes);

    let enc_raw = install
        .read_file_by_encoding_key(&ekey)
        .await
        .expect("read encoding file from local CASC");

    let encoding = EncodingFile::parse(&enc_raw).expect("parse encoding file");
    println!(
        "  Encoding: {} CKey entries, {} EKey entries",
        encoding.ckey_count(),
        encoding.ekey_count()
    );

    // Step 3: Collect all local IDX ekeys (9-byte truncated)
    println!("\n=== Step 3: Collect local IDX ekeys ===");
    let local_entries = install.get_all_index_entries().await;
    let local_ekeys: HashSet<Vec<u8>> = local_entries.iter().map(|e| e.key.to_vec()).collect();
    println!("  Local IDX: {} unique ekeys", local_ekeys.len());

    // Step 4: Load CDN archive indices
    println!("\n=== Step 4: Load CDN archive indices ===");
    let cdn_ekeys = load_cdn_index_ekeys(&wow.join("Data").join("indices"));

    // Step 5: Check coverage of encoding file ekeys
    println!("\n=== Step 5: Coverage analysis ===");
    let mut in_local = 0u64;
    let mut in_cdn = 0u64;
    let mut missing = 0u64;
    let mut missing_ekeys: Vec<String> = Vec::new();

    for page in &encoding.ckey_pages {
        for entry in &page.entries {
            for enc_key in &entry.encoding_keys {
                let full_ekey = enc_key.as_bytes();
                let truncated: Vec<u8> = full_ekey[..9].to_vec();

                if local_ekeys.contains(&truncated) {
                    in_local += 1;
                } else if cdn_ekeys.contains(full_ekey.as_slice()) {
                    in_cdn += 1;
                } else {
                    missing += 1;
                    if missing_ekeys.len() < 20 {
                        missing_ekeys.push(format!(
                            "ekey={} ckey={}",
                            hex::encode(full_ekey),
                            hex::encode(entry.content_key.as_bytes())
                        ));
                    }
                }
            }
        }
    }

    let total = in_local + in_cdn + missing;
    let pct = |n: u64| n as f64 / total as f64 * 100.0;

    println!("  Total encoding ekeys: {total}");
    println!("  In local IDX:        {in_local} ({:.1}%)", pct(in_local));
    println!("  In CDN indices:      {in_cdn} ({:.1}%)", pct(in_cdn));
    println!("  MISSING:             {missing} ({:.1}%)", pct(missing));

    if !missing_ekeys.is_empty() {
        println!("\n  First {} missing ekeys:", missing_ekeys.len());
        for ekey_hex in &missing_ekeys {
            println!("    {ekey_hex}");
        }
    }

    // Step 6: Parse build config and check critical keys
    println!("\n=== Step 6: Critical key availability ===");
    let config_dir = wow.join("Data").join("config");

    let bc_path = config_dir
        .join(&BUILD_CONFIG[..2])
        .join(&BUILD_CONFIG[2..4])
        .join(BUILD_CONFIG);
    let bc_data = std::fs::read(&bc_path).expect("read build config");
    let bc = BuildConfig::parse(bc_data.as_slice()).expect("parse build config");

    if let Some(root_hex) = bc.root() {
        let root_ckey = cascette_crypto::ContentKey::from_hex(root_hex).expect("valid root hex");
        if let Some(root_ekey) = encoding.find_encoding(&root_ckey) {
            let trunc: Vec<u8> = root_ekey.as_bytes()[..9].to_vec();
            let in_local_idx = local_ekeys.contains(&trunc);
            let in_cdn_idx = cdn_ekeys.contains(root_ekey.as_bytes().as_slice());
            println!(
                "  Root:     ekey={} local={in_local_idx} cdn={in_cdn_idx}",
                hex::encode(root_ekey.as_bytes()),
            );
        } else {
            println!("  Root:     NOT IN ENCODING TABLE");
        }
    }

    {
        let trunc: Vec<u8> = ekey_bytes[..9].to_vec();
        println!(
            "  Encoding: ekey={ENCODING_EKEY} local={}",
            local_ekeys.contains(&trunc)
        );
    }

    let cdn_cfg_hash = "c54b41b3195b9482ce0d3c6bf0b86cdb";
    let cc_path = config_dir
        .join(&cdn_cfg_hash[..2])
        .join(&cdn_cfg_hash[2..4])
        .join(cdn_cfg_hash);
    if let Ok(cc_data) = std::fs::read(&cc_path)
        && let Ok(cc) = CdnConfig::parse(cc_data.as_slice())
    {
        println!(
            "\n  CDN config: {} archives, archive-group={}, file-index={}",
            cc.archives().len(),
            cc.archive_group().unwrap_or("none"),
            cc.file_index().unwrap_or("none"),
        );
    }

    // Summary
    println!("\n=== Summary ===");
    if missing == 0 {
        println!("  ALL encoding keys are resolvable. Installation should work.");
    } else {
        println!("  {missing} encoding keys are NOT resolvable through local IDX or CDN indices.");
        println!("  The client may fail to load assets that map to these keys.");
    }
}
