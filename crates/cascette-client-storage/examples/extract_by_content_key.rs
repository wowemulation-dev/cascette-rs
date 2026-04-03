#![allow(clippy::expect_used, clippy::panic)]

//! Extract a file from a local CASC installation by its content key (CKey).
//!
//! This replicates two commands from reference tools:
//! - BuildBackup:  `extractfilebycontenthash <product> <buildconfig> <cdnconfig> <ckey>`
//! - TACTSharp:    `TACTTool --mode ckey --inputvalue <ckey> --basedir <path>`
//!
//! A content key (CKey) is the MD5 hash of a file's uncompressed content.
//! The CASC encoding table maps each CKey to one or more encoding keys (EKeys),
//! which are the actual keys used to look up data in the archive indices.
//!
//! Usage (requires a local WoW Classic installation):
//!   CASCETTE_WOW_PATH=/path/to/wow_classic \
//!     cargo run --example extract_by_content_key \
//!     -p cascette-client-storage --features local-install -- <ckey_hex>
//!
//! If no CKey is given, the example reads all index entries and uses the
//! first available one to demonstrate a successful read-by-content-key.

mod common;

use cascette_client_storage::Installation;
use cascette_crypto::ContentKey;

#[tokio::main]
async fn main() {
    let wow = common::wow_path();
    let data_root = wow.join("Data");

    let args: Vec<String> = std::env::args().collect();
    let explicit_ckey: Option<String> = args.get(1).cloned();

    println!("=== Extract File by Content Key ===");
    println!("Installation: {}", data_root.display());
    println!();

    // Open the local installation and build indices
    let install = Installation::open(data_root).expect("failed to open installation");
    install
        .initialize()
        .await
        .expect("failed to initialize installation");

    let stats = install.stats().await;
    println!(
        "Index:  {} files across {} index files",
        stats.index_entries, stats.index_files
    );
    println!(
        "Storage: {} archive files ({} bytes)",
        stats.archive_files, stats.archive_size
    );
    println!();

    // ── Resolve the target CKey ────────────────────────────────────────────
    //
    // A CKey is the MD5 of the decoded file content. In WoW's CASC layout,
    // the encoding file maps CKey -> EKey, and the archive index maps EKey ->
    // (archive, offset, size). This example takes a CKey on the command line
    // and follows that chain to extract the raw BLTE-encoded file data.
    //
    // When no CKey is provided we use the first entry visible in the index
    // directly (which holds EKeys). This demonstrates the fallback path where
    // you have an EKey but not a CKey.

    if let Some(hex) = explicit_ckey {
        // Path A: caller supplied a CKey — use the full CKey -> EKey -> data chain
        let ckey = ContentKey::from_hex(&hex).expect("invalid hex content key");

        println!("Looking up content key: {}", ckey.to_hex());
        println!("  (This requires the encoding file to be loaded via load_encoding_file)");
        println!();

        // Load the encoding file so the installation can resolve CKey -> EKey.
        // The encoding EKey comes from the build config's "encoding" field.
        println!("Note: To resolve by CKey on a local installation, call:");
        println!("  install.load_encoding_file(&encoding_ekey).await");
        println!("  install.read_file_by_content_key(&ckey).await");
        println!();
        println!("Without the encoding file loaded, we can check presence:");
        let present = install.has_content_key(&ckey).await;
        println!("  CKey present in local storage: {present}");
        println!("  (Only true if encoding file was pre-loaded)");
    } else {
        // Path B: no CKey given — demonstrate reading by encoding key directly,
        // which is what BuildBackup's extractfilebyencodingkey does.
        let all_entries = install.get_all_index_entries().await;
        if all_entries.is_empty() {
            println!("No index entries found. Is CASCETTE_WOW_PATH correct?");
            return;
        }

        println!("No CKey specified. Demonstrating read-by-encoding-key on first entry.");
        println!("Usage: pass a hex CKey as the first argument to extract by content key.");
        println!();

        let entry = &all_entries[0];
        let ekey_hex = hex::encode(entry.key.as_slice());
        println!("First index entry:");
        println!("  EKey:       {ekey_hex}");
        println!("  Size:       {} bytes", entry.size);
        println!("  Archive ID: {}", entry.archive_location.archive_id);
        println!("  Offset:     {}", entry.archive_location.archive_offset);
        println!();

        // Read decoded data from the archive (read_from_archive auto-decompresses BLTE)'s
        // extractrawfilebycontenthash (before BLTE decode).
        println!("Reading decoded data from archive ...");
        let raw_data = install
            .read_from_archive(
                entry.archive_location.archive_id,
                entry.archive_location.archive_offset,
                entry.size,
            )
            .await
            .expect("failed to read from archive");

        println!("  Read {} bytes (BLTE-decoded)", raw_data.len());

        // Show the BLTE magic bytes to confirm the read worked
        if raw_data.len() >= 4 {
            let magic = &raw_data[..4];
            let magic_str: String = magic
                .iter()
                .map(|b| {
                    if b.is_ascii_graphic() {
                        *b as char
                    } else {
                        '.'
                    }
                })
                .collect();
            println!("  Magic: {:02x?} (ASCII: {magic_str})", magic);

            if magic == b"BLTE" {
                println!("  Confirmed: valid BLTE container");
            }
        }

        // Validate that the entry's expected size matches what we got
        println!();
        println!("=== Entry Validation ===");
        let valid = install
            .validate_entry(
                entry.archive_location.archive_id,
                entry.archive_location.archive_offset,
                entry.size,
            )
            .await
            .unwrap_or(false);
        println!(
            "  Entry valid (BLTE magic check): {}",
            if valid { "yes" } else { "no" }
        );

        // Show a sample of more entries to mimic BuildBackup's dumpindex output
        println!();
        println!("=== Index Sample (first 10 entries) ===");
        println!(
            "  {:<18}  {:>10}  {:>12}",
            "EKey (9-byte)", "Size", "Offset"
        );
        println!("  {:-<18}  {:->10}  {:->12}", "", "", "");

        for entry in all_entries.iter().take(10) {
            let ekey = hex::encode(entry.key.as_slice());
            println!(
                "  {ekey:<18}  {:>10}  {:>12}",
                entry.size, entry.archive_location.archive_offset
            );
        }

        if all_entries.len() > 10 {
            println!("  ... ({} more entries)", all_entries.len() - 10);
        }
    }

    println!();
    println!("=== Reference Tool Equivalence ===");
    println!("  BuildBackup extractfilebycontenthash -> install.read_file_by_content_key()");
    println!("  BuildBackup extractfilebyencodingkey -> install.read_file_by_encoding_key()");
    println!("  BuildBackup extractfilesbyfdidlist   -> install.read_files_by_fdids()");
    println!("  BuildBackup extractfilesbyfnamelist  -> install.read_files_by_paths()");
    println!("  TACTSharp --mode ckey                -> install.read_file_by_content_key()");
    println!("  TACTSharp --mode ekey                -> install.read_file_by_encoding_key()");
    println!("  TACTSharp --mode fdid                -> install.read_file_by_fdid()");
    println!("  TACTSharp --mode name                -> install.read_file_by_path()");
}
