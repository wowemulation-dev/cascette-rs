#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Full CDN chain verification: download, BLTE-decode, parse, and resolve.
//!
//! Tests the full download-parse-decode chain across cascette-protocol,
//! cascette-formats, and cascette-crypto using pinned hashes from
//! WoW Classic 1.13.2.31650.
//!
//! Environment variables:
//!   CASCETTE_CDN_HOSTS  Comma-separated CDN hostnames
//!                       (default: casc.wago.tools,cdn.arctium.tools,archive.wow.tools)
//!   CASCETTE_CDN_PATH   CDN product path (default: tpr/wow)
//!
//! Usage:
//!   cargo run -p cascette-protocol --example cdn_chain_verification

use cascette_crypto::{ContentKey, FileDataId};
use cascette_formats::CascFormat;
use cascette_formats::blte::BlteFile;
use cascette_formats::config::{BuildConfig, CdnConfig};
use cascette_formats::encoding::EncodingFile;
use cascette_formats::install::InstallManifest;
use cascette_protocol::{
    CacheConfig, CdnClient, CdnConfig as ProtoCdnConfig, CdnEndpoint, ClientConfig, ContentType,
    RibbitTactClient,
};

// ---------------------------------------------------------------------------
// Pinned hashes from WoW Classic 1.13.2.31650
// ---------------------------------------------------------------------------

const BUILD_CONFIG: &str = "2c915a9a226a3f35af6c65fcc7b6ca4a";
const CDN_CONFIG: &str = "c54b41b3195b9482ce0d3c6bf0b86cdb";
const ROOT_HASH: &str = "6edece184a23ac1bad0ea96b7512b9fc";
const ENCODING_CKEY: &str = "9029b103d5b6e6e750d9ecc23e4e27d4";
const KNOWN_FILE_FDID: u32 = 804916;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cdn_endpoints() -> Vec<CdnEndpoint> {
    let hosts = std::env::var("CASCETTE_CDN_HOSTS")
        .unwrap_or_else(|_| "casc.wago.tools,cdn.arctium.tools,archive.wow.tools".into());
    let path = std::env::var("CASCETTE_CDN_PATH").unwrap_or_else(|_| "tpr/wow".into());
    hosts
        .split(',')
        .map(|h| CdnEndpoint {
            host: h.trim().into(),
            path: path.clone(),
            product_path: None,
            scheme: Some("https".to_string()),
            is_fallback: false,
            strict: false,
            max_hosts: None,
        })
        .collect()
}

fn community_client_config() -> ClientConfig {
    ClientConfig {
        tact_https_url: "https://us.version.battle.net".to_string(),
        tact_http_url: String::new(),
        ribbit_url: "tcp://127.0.0.1:1".to_string(),
        cache_config: CacheConfig::memory_optimized(),
        ..Default::default()
    }
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex).unwrap_or_else(|e| panic!("invalid hex '{hex}': {e}"))
}

fn primary_endpoint() -> CdnEndpoint {
    cdn_endpoints().into_iter().next().unwrap()
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let config = community_client_config();
    let client = RibbitTactClient::new(config).expect("client creation");
    let cdn =
        CdnClient::new(client.cache().clone(), ProtoCdnConfig::default()).expect("cdn client");
    let endpoints = cdn_endpoints();

    // B1: Download build config -> parse -> extract encoding/install info
    println!("=== B1: Build config -> encoding info ===");
    let bc_data = cdn
        .download_from_endpoints(&endpoints, ContentType::Config, &hex_bytes(BUILD_CONFIG))
        .await
        .expect("build config download");
    let bc = BuildConfig::parse(bc_data.as_slice()).expect("build config parse");

    let enc_info = bc.encoding().expect("build config should have encoding");
    assert_eq!(
        enc_info.content_key, ENCODING_CKEY,
        "encoding ckey should match pinned value"
    );

    let enc_ekey = enc_info.encoding_key.expect("encoding should have ekey");
    println!(
        "  encoding ckey={}, ekey={}",
        enc_info.content_key, enc_ekey
    );

    // B2: Download encoding table -> BLTE-decode -> parse
    println!("\n=== B2: Encoding table download and parse ===");
    let enc_blte_data = cdn
        .download_from_endpoints(&endpoints, ContentType::Data, &hex_bytes(&enc_ekey))
        .await
        .expect("encoding table download");

    let blte = BlteFile::parse(&enc_blte_data).expect("BLTE parse");
    let enc_raw = blte.decompress().expect("BLTE decompress");

    let enc = EncodingFile::parse(&enc_raw).expect("encoding file parse");
    assert_eq!(&enc.header.magic, b"EN", "encoding magic should be EN");
    assert!(enc.ckey_count() > 0, "should have ckey entries");
    assert!(enc.ekey_count() > 0, "should have ekey entries");

    println!(
        "  {} ckey entries, {} ekey entries, {} espec entries",
        enc.ckey_count(),
        enc.ekey_count(),
        enc.espec_table.entries.len(),
    );

    // B3: Download install manifest -> BLTE-decode -> parse
    println!("\n=== B3: Install manifest download and parse ===");
    let install_infos = bc.install();
    assert!(
        !install_infos.is_empty(),
        "build config should have install entries"
    );

    let install_ekey = install_infos[0]
        .encoding_key
        .as_ref()
        .expect("install should have ekey");

    let install_blte = cdn
        .download_from_endpoints(&endpoints, ContentType::Data, &hex_bytes(install_ekey))
        .await
        .expect("install manifest download");

    let blte = BlteFile::parse(&install_blte).expect("BLTE parse");
    let install_raw = blte.decompress().expect("BLTE decompress");

    let manifest = InstallManifest::parse(&install_raw).expect("install manifest parse");
    assert!(
        !manifest.entries.is_empty(),
        "install manifest should have file entries"
    );
    assert!(
        !manifest.tags.is_empty(),
        "install manifest should have tags"
    );

    println!(
        "  {} entries, {} tags, version {}",
        manifest.entries.len(),
        manifest.tags.len(),
        manifest.header.version,
    );

    // B4: Download root manifest via encoding lookup -> parse
    println!("\n=== B4: Root manifest download and parse ===");
    let root_ckey_bytes: [u8; 16] = hex_bytes(ROOT_HASH)
        .try_into()
        .expect("root hash should be 16 bytes");
    let root_ckey = ContentKey::from_bytes(root_ckey_bytes);
    let root_ekey = enc
        .find_encoding(&root_ckey)
        .expect("root ckey should be in encoding table");

    let root_blte = cdn
        .download_from_endpoints(&endpoints, ContentType::Data, root_ekey.as_bytes())
        .await
        .expect("root download");
    let root_raw = BlteFile::parse(&root_blte)
        .expect("BLTE parse")
        .decompress()
        .expect("BLTE decompress");

    let root = cascette_formats::root::RootFile::parse(&root_raw).expect("root parse");
    assert!(root.total_files() > 0, "root should contain file entries");

    println!(
        "  {} total files, version {:?}",
        root.total_files(),
        root.version,
    );

    // B5: Resolve FDID 804916 through full chain
    println!("\n=== B5: FDID resolution chain ===");
    let fdid = FileDataId::new(KNOWN_FILE_FDID);
    let file_ckey = root
        .resolve_by_id(
            fdid,
            cascette_formats::root::LocaleFlags::new(0xFFFF_FFFF),
            cascette_formats::root::ContentFlags::new(0),
        )
        .expect("known FDID should resolve in root");

    let file_ekey = enc
        .find_encoding(&file_ckey)
        .expect("file ckey should be in encoding");

    assert_ne!(
        file_ckey.as_bytes(),
        &[0u8; 16],
        "content key should be non-zero"
    );
    assert_ne!(
        file_ekey.as_bytes(),
        &[0u8; 16],
        "encoding key should be non-zero"
    );

    let resolved_count = root
        .iter_records()
        .take(1000)
        .filter(|r| enc.find_encoding(&r.content_key).is_some())
        .count();

    println!(
        "  FDID {fdid}: ckey={}, ekey={}",
        hex::encode(file_ckey.as_bytes()),
        hex::encode(file_ekey.as_bytes()),
    );
    println!("  {resolved_count}/1000 sampled records resolve through encoding");

    // B6: Download and parse one archive index
    println!("\n=== B6: Archive index download and parse ===");
    let cc_data = cdn
        .download_from_endpoints(&endpoints, ContentType::Config, &hex_bytes(CDN_CONFIG))
        .await
        .expect("CDN config download");
    let cc = CdnConfig::parse(cc_data.as_slice()).expect("CDN config parse");

    let archives = cc.archives();
    assert!(
        !archives.is_empty(),
        "CDN config should list archive entries"
    );

    let first_archive = &archives[0];
    let endpoint = primary_endpoint();
    let index_data = cdn
        .download_archive_index(&endpoint, &first_archive.content_key)
        .await
        .expect("archive index download");

    assert!(!index_data.is_empty(), "archive index should have content");

    let mut cursor = std::io::Cursor::new(&index_data[..]);
    let index =
        cascette_formats::archive::ArchiveIndex::parse(&mut cursor).expect("archive index parse");

    assert!(
        !index.entries.is_empty(),
        "archive index should have entries"
    );

    println!(
        "  archive {}: {} entries, {} bytes",
        &first_archive.content_key[..8],
        index.entries.len(),
        index_data.len(),
    );

    println!("\nAll CDN chain checks passed.");
}
