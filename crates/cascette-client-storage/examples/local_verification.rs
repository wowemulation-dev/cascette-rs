#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Local CASC storage verification against a WoW Classic installation.
//!
//! Exercises `cascette-client-storage` and `cascette-formats` against local
//! index files, archives, and config files using pinned hashes from
//! WoW Classic 1.13.2.31650.
//!
//! Environment variables:
//!   CASCETTE_WOW_PATH  Path to a local WoW installation root (required)
//!
//! Usage:
//!   CASCETTE_WOW_PATH=~/Downloads/wow_classic/1.13.2.31650.windows-win64 \
//!     cargo run -p cascette-client-storage --example local_verification \
//!       --features local-install

mod common;

use cascette_client_storage::{BuildInfoFile, Installation};
use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::config::{BuildConfig, CdnConfig};
use cascette_formats::encoding::EncodingFile;

// ---------------------------------------------------------------------------
// Pinned hashes from WoW Classic 1.13.2.31650
// ---------------------------------------------------------------------------

const BUILD_CONFIG: &str = "2c915a9a226a3f35af6c65fcc7b6ca4a";
const CDN_CONFIG: &str = "c54b41b3195b9482ce0d3c6bf0b86cdb";
const ROOT_HASH: &str = "6edece184a23ac1bad0ea96b7512b9fc";
const ENCODING_EKEY: &str = "59cad02d7dc0187413ae485a766f851b";
const DOWNLOAD_CKEY: &str = "a6966ef4a7427567ff60270f12478969";

fn hex_bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex).unwrap_or_else(|e| panic!("invalid hex '{hex}': {e}"))
}

#[tokio::main]
async fn main() {
    let wow = common::wow_path();

    // C1: Read .build.info
    println!("=== C1: Read .build.info ===");
    let info_path = wow.join(".build.info");
    let info = BuildInfoFile::from_path(&info_path)
        .await
        .expect(".build.info parse should succeed");

    assert!(info.entry_count() > 0, "should have at least one entry");

    let active = info.active_entry().expect("should have an active entry");
    assert!(active.is_active(), "active entry should be active");

    let build_key = active.build_key().expect("should have Build Key");
    assert_eq!(
        build_key, BUILD_CONFIG,
        "build config hash should match pinned value"
    );

    let cdn_key = active.cdn_key().expect("should have CDN Key");
    assert_eq!(
        cdn_key, CDN_CONFIG,
        "CDN config hash should match pinned value"
    );

    println!(
        "  product={:?}, version={:?}, build_key={}",
        active.product(),
        active.version(),
        build_key,
    );

    // C2: Open local .idx indices
    println!("\n=== C2: Open local indices ===");
    let data_path = wow.join("Data");
    let install = Installation::open(data_path.clone()).expect("installation open should succeed");
    install
        .initialize()
        .await
        .expect("initialization should succeed");

    let stats = install.stats().await;
    assert!(
        stats.index_entries > 0,
        "should have loaded index entries (got {})",
        stats.index_entries
    );
    assert!(
        stats.index_files > 0,
        "should have found .idx files (got {})",
        stats.index_files
    );

    println!(
        "  {} index files, {} entries, {} archives",
        stats.index_files, stats.index_entries, stats.archive_files,
    );

    // C3: Read local encoding table from data files
    println!("\n=== C3: Read local encoding table ===");
    let ekey_bytes: [u8; 16] = hex_bytes(ENCODING_EKEY).try_into().expect("16 bytes");
    let ekey = EncodingKey::from_bytes(ekey_bytes);

    let enc_raw = install
        .read_file_by_encoding_key(&ekey)
        .await
        .expect("should read encoding from local archives");

    let enc = EncodingFile::parse(&enc_raw).expect("encoding file parse");
    assert_eq!(&enc.header.magic, b"EN", "encoding magic");
    assert!(enc.ckey_count() > 0, "ckey entries");
    assert!(enc.ekey_count() > 0, "ekey entries");

    println!(
        "  {} ckey entries, {} ekey entries",
        enc.ckey_count(),
        enc.ekey_count(),
    );

    // C4: Verify content key lookups in encoding table
    println!("\n=== C4: Encoding table lookups ===");
    let root_ckey_bytes: [u8; 16] = hex_bytes(ROOT_HASH).try_into().expect("16 bytes");
    let root_ckey = ContentKey::from_bytes(root_ckey_bytes);
    let root_ekey = enc
        .find_encoding(&root_ckey)
        .expect("root ckey should be in encoding table");

    let dl_ckey_bytes: [u8; 16] = hex_bytes(DOWNLOAD_CKEY).try_into().expect("16 bytes");
    let dl_ckey = ContentKey::from_bytes(dl_ckey_bytes);
    let dl_ekey = enc
        .find_encoding(&dl_ckey)
        .expect("download ckey should be in encoding table");

    println!(
        "  root ekey={}, download ekey={}, total={} entries",
        hex::encode(root_ekey.as_bytes()),
        hex::encode(dl_ekey.as_bytes()),
        enc.ckey_count(),
    );

    // C5: Parse local build config and CDN config files
    println!("\n=== C5: Local config file parsing ===");
    let config_dir = wow.join("Data/config");

    let bc_path = config_dir
        .join(&BUILD_CONFIG[..2])
        .join(&BUILD_CONFIG[2..4])
        .join(BUILD_CONFIG);
    let bc_data = std::fs::read(&bc_path).unwrap_or_else(|e| {
        panic!(
            "build config file should exist at {}: {e}",
            bc_path.display()
        )
    });
    let bc = BuildConfig::parse(bc_data.as_slice()).expect("build config parse");

    assert_eq!(
        bc.root().expect("should have root"),
        ROOT_HASH,
        "root hash should match"
    );
    assert!(bc.encoding().is_some(), "should have encoding");

    let cc_path = config_dir
        .join(&CDN_CONFIG[..2])
        .join(&CDN_CONFIG[2..4])
        .join(CDN_CONFIG);
    let cc_data = std::fs::read(&cc_path)
        .unwrap_or_else(|e| panic!("CDN config file should exist at {}: {e}", cc_path.display()));
    let cc = CdnConfig::parse(cc_data.as_slice()).expect("CDN config parse");

    let archives = cc.archives();
    assert!(!archives.is_empty(), "CDN config should list archives");

    println!(
        "  build root={}, {} archives",
        bc.root().unwrap_or("?"),
        archives.len(),
    );

    println!("\nAll local storage checks passed.");
}
