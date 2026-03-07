//! Listfile name lookup: resolve file names to FileDataIDs using Jenkins96.
//!
//! This replicates two reference tool workflows:
//! - BuildBackup `calchashlistfile`: hashes filenames to Jenkins96 lookup keys
//! - wow.export listfile: maps FileDataID <-> filename for UI browsing
//!
//! The WoW root file maps FileDataID -> ContentKey. The listfile maps
//! FileDataID -> human-readable filename. Together they allow resolving
//! a filename like "Interface\\Icons\\INV_Misc_QuestionMark.blp" to its
//! content key and eventually to archive data.
//!
//! Reference tools:
//! - BuildBackup: `calchashlistfile <listfile>` — hash names for lookup
//! - wow.export:  listfile.js — FDID -> filename mapping for UI
//! - CascLib:     Jenkins96 hash used in CascFindFile()
//!
//! Usage:
//!   cargo run -p cascette-import --example listfile_name_lookup \
//!     --features listfile
//!
//! Optional: pass file paths as arguments to hash them:
//!   cargo run -p cascette-import --example listfile_name_lookup \
//!     --features listfile -- "Interface\\Icons\\INV_Misc_QuestionMark.blp"

use cascette_crypto::jenkins::Jenkins96;

/// Calculate name hash for a file path using Jenkins96.
///
/// Normalizes to uppercase with backslashes, matching CascLib CalcNormNameHash.
fn calculate_name_hash(path: &str) -> u64 {
    let normalized = path.to_uppercase().replace('/', "\\");
    let hash = Jenkins96::hash(normalized.as_bytes());
    hash.hash64
}
use cascette_import::ListfileProvider;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    println!("=== Listfile Name Lookup ===");
    println!("(Replicates BuildBackup calchashlistfile + wow.export listfile)");
    println!();

    // ── Part 1: Jenkins96 hash demonstration ──────────────────────────────
    // BuildBackup's calchashlistfile hashes each filename so CASC can locate
    // it in the root file's sorted hash table. The same hash is used by
    // CascFindFile() in CascLib and by wow.export for UI navigation.
    //
    // calculate_name_hash() normalizes the path (uppercase, backslash) then
    // runs Jenkins hashlittle2, returning a 64-bit composite hash.
    println!("=== Jenkins96 Hash Calculation ===");
    println!("(Equivalent to BuildBackup calchashlistfile, CascLib CalcFileNameHash)");
    println!();

    let demo_paths: Vec<&str> = if args.is_empty() {
        vec![
            "Interface\\Icons\\INV_Misc_QuestionMark.blp",
            "interface/icons/inv_misc_questionmark.blp", // same file, different separators
            "World\\Maps\\Azeroth\\Azeroth.wdt",
            "Sound\\Music\\ZoneMusic\\DMF_L70ETC01.mp3",
            "DBFilesClient\\ItemDisplayInfo.dbc",
        ]
    } else {
        args.iter().map(String::as_str).collect()
    };

    for path in &demo_paths {
        let hash = calculate_name_hash(path);
        println!("  {path}");
        println!("    Hash: 0x{hash:016x}");
    }

    // Show that path normalization makes slash-variant paths identical
    if args.is_empty() {
        let h1 = calculate_name_hash("Interface\\Icons\\INV_Misc_QuestionMark.blp");
        let h2 = calculate_name_hash("interface/icons/inv_misc_questionmark.blp");
        println!();
        println!(
            "  Slash normalization: {} (hash match: {})",
            if h1 == h2 { "consistent" } else { "MISMATCH" },
            h1 == h2
        );
    }

    // ── Part 2: ListfileProvider overview ─────────────────────────────────
    // The ListfileProvider fetches the WoWDev community listfile and provides
    // FDID -> filename mappings. wow.export uses this to show file names in its
    // tree view; BuildBackup uses it for extractfilesbyfnamelist.
    println!();
    println!("=== ListfileProvider (Community Listfile) ===");
    println!("Source: https://github.com/wowdev/wow-listfile");
    println!();

    let Ok(cache_dir) = tempfile::tempdir() else {
        eprintln!("failed to create temp dir");
        return;
    };
    let Ok(_provider) = ListfileProvider::new(cache_dir.path().join("listfile")) else {
        eprintln!("failed to create ListfileProvider");
        return;
    };

    println!("ListfileProvider created.");
    println!("  Cache dir: {}", cache_dir.path().display());
    println!();
    println!("To use in production:");
    println!("  1. let mgr = ImportManager::new(cache_dir);");
    println!("  2. mgr.add_provider(Box::new(provider)).await?;   // fetches listfile");
    println!("  3. let fdid = mgr.lookup_fdid(\"Interface/Icons/...\")");
    println!("  4. Use fdid with install.read_file_by_fdid(fdid).await");
    println!();

    // ── Part 3: Listfile format and FDID->hash table ───────────────────────
    println!("=== Listfile Format (CSV) ===");
    println!();

    // Manually parse a small listfile sample to show FDID -> hash relationship
    let sample = "53020;Interface/Icons/INV_Misc_QuestionMark.blp\n\
                  892176;World/Maps/Azeroth/Azeroth.wdt\n\
                  781764;Sound/Music/ZoneMusic/DMF_L70ETC01.mp3\n";

    println!("  {:<8}  {:<18}  Path", "FDID", "Name hash");
    println!("  {:-<8}  {:-<18}  {:-<50}", "", "", "");
    for line in sample.lines() {
        if let Some((id_str, path)) = line.split_once(';')
            && let Ok(fdid) = id_str.parse::<u32>()
        {
            let hash = calculate_name_hash(path);
            println!("  {fdid:<8}  0x{hash:016x}  {path}");
        }
    }

    // ── Part 4: Reference tool comparison ─────────────────────────────────
    println!();
    println!("=== Reference Tool Equivalence ===");
    println!("  BuildBackup calchashlistfile  -> calculate_name_hash() per line");
    println!("  BuildBackup extractfilesbyf.  -> mgr.lookup_fdid() + read_file_by_fdid()");
    println!("  wow.export listfile.js         -> ListfileProvider FDID<->name mapping");
    println!("  CascLib CascFindFile()         -> Jenkins96 hash match in root file");
    println!("  TACTSharp --mode name          -> install.read_file_by_path()");
}
