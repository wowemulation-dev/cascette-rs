#![allow(clippy::expect_used, clippy::panic)]

//! Full CAS container storage dump: ordered walk of every data.NNN file,
//! cross-referenced against the IDX index (KMT).
//!
//! Unlike the sampling examples (dump_index, read_archives, dump_local_headers),
//! this walks EVERY entry in on-disk order and validates the contract between
//! the two layers:
//!
//!   1. For each data.NNN, read the 480-byte segment header (16 reconstruction
//!      headers, one per KMT bucket) and validate checksum_a.
//!   2. Collect all IDX entries (sorted section), group by archive_id, sort
//!      by archive_offset — this is the client's on-disk order.
//!   3. For each IDX entry, read the 30-byte LocalHeader at (archive, offset):
//!      verify the reversed key matches, flags, and checksum_a.
//!   4. Detect gaps/overlaps between consecutive entries (contiguity check).
//!   5. Report per-bucket stats, reconstruction-header entries (size=30).
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow cargo run -p cascette-client-storage \
//!       --features local-install --example dump_cas_container

mod common;

use std::collections::HashMap;
use std::fs;

use cascette_client_storage::index::IndexManager;
use cascette_client_storage::storage::{
    LocalHeader, SEGMENT_HEADER_SIZE, local_header::LOCAL_HEADER_SIZE,
};

/// Read a byte range from a data.NNN file using a cached in-memory copy.
/// Each data file is read once (they are up to 1 GiB each, so repeated
/// `fs::read` per entry would be catastrophic: 130k entries x 1 GiB).
struct DataFiles {
    /// archive_id -> full file bytes
    files: HashMap<u16, Vec<u8>>,
}

impl DataFiles {
    fn load(data_dir: &std::path::Path) -> Self {
        let mut files = HashMap::new();
        for entry in fs::read_dir(data_dir).expect("read data dir") {
            let entry = entry.expect("dir entry");
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(idx) = parse_data_name(&name) {
                files.insert(idx, fs::read(entry.path()).expect("read data file"));
            }
        }
        Self { files }
    }

    fn read_range(&self, archive_id: u16, offset: u32, len: usize) -> Option<Vec<u8>> {
        let file = self.files.get(&archive_id)?;
        let start = offset as usize;
        if start + len > file.len() {
            return None;
        }
        Some(file[start..start + len].to_vec())
    }
}

#[tokio::main]
async fn main() {
    let data = common::data_path();
    let data_files = DataFiles::load(&data);
    println!("CAS container dump: {}\n", data.display());

    // ------------------------------------------------------------------
    // 1. Segment headers from every data.NNN
    // ------------------------------------------------------------------
    println!("=== 1. Segment headers (16 reconstruction headers per data.NNN) ===");
    let mut seg_files: Vec<(u16, std::path::PathBuf)> = Vec::new();
    for entry in fs::read_dir(&data).expect("failed to read data dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(idx) = parse_data_name(&name) {
            seg_files.push((idx, entry.path()));
        }
    }
    seg_files.sort_by_key(|(idx, _)| *idx);

    let mut recon_total = 0u32;
    let mut recon_ck_fail = 0u32;
    let mut seg_data_sizes: HashMap<u16, u64> = HashMap::new();
    for &(seg_idx, ref path) in &seg_files {
        let meta = fs::metadata(path).expect("stat");
        seg_data_sizes.insert(seg_idx, meta.len());
        let raw = fs::read(path).expect("read");
        let seg = &raw[..SEGMENT_HEADER_SIZE.min(raw.len())];
        println!(
            "  data.{seg_idx:03}: {} bytes ({} MiB)",
            meta.len(),
            meta.len() / (1024 * 1024)
        );
        let mut non_zero = 0;
        for bucket in 0..16u8 {
            let off = bucket as usize * LOCAL_HEADER_SIZE;
            let slice = &seg[off..off + LOCAL_HEADER_SIZE];
            let Some(lh) = LocalHeader::from_bytes(slice) else {
                continue;
            };
            let ekey = lh.original_encoding_key();
            if ekey.iter().all(|&b| b == 0) {
                continue;
            }
            non_zero += 1;
            recon_total += 1;
            let arr: [u8; LOCAL_HEADER_SIZE] = slice.try_into().expect("30-byte header");
            let calc_a = LocalHeader::compute_checksum_a(&arr);
            let ok = calc_a == lh.checksum_a;
            if !ok {
                recon_ck_fail += 1;
            }
            if bucket == 0 || bucket == 15 {
                println!(
                    "    bucket={bucket:>2} EKey={} size={} flags=0x{:04x} ckA={:08x}{}",
                    common::hex_str(&ekey),
                    lh.encoded_size,
                    lh.flags,
                    lh.checksum_a,
                    if ok { "" } else { " CHECKSUM_A MISMATCH" }
                );
            }
        }
        if non_zero != 16 {
            println!("    WARNING: only {non_zero}/16 reconstruction headers");
        }
    }
    println!("  reconstruction headers: {recon_total}, checksum_a failures: {recon_ck_fail}\n");

    // ------------------------------------------------------------------
    // 2. Load IDX (KMT)
    // ------------------------------------------------------------------
    let mut mgr = IndexManager::new(&data);
    mgr.load_all().await.expect("failed to load indices");
    let stats = mgr.stats();
    println!(
        "=== 2. IDX index: {} files, {} total entries ===\n",
        stats.index_count, stats.total_entries
    );

    println!("  per-bucket generations:");
    for bucket in 0..16u8 {
        let mut gens: Vec<u32> = Vec::new();
        for entry in fs::read_dir(&data).expect("read_dir") {
            let entry = entry.expect("dir entry");
            let name = entry.file_name().to_string_lossy().to_string();
            if name.len() == 12
                && std::path::Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("idx"))
            {
                let b = u8::from_str_radix(&name[..2], 16).expect("bucket hex");
                if b == bucket {
                    let g = u32::from_str_radix(&name[2..10], 16).expect("gen hex");
                    gens.push(g);
                }
            }
        }
        gens.sort_unstable();
        if !gens.is_empty() {
            println!("    bucket {bucket:02x}: generations {gens:?}");
        }
    }
    println!();

    // ------------------------------------------------------------------
    // 3. Full ordered walk
    // ------------------------------------------------------------------
    let mut entries: Vec<(u8, cascette_client_storage::IndexEntry)> = mgr.iter_entries().collect();
    // On-disk order: by archive file, then offset within the file
    entries.sort_by_key(|(_, e)| (e.archive_id(), e.archive_offset()));

    println!(
        "=== 3. Ordered walk: {} entries (on-disk order) ===",
        entries.len()
    );
    println!(
        "{:>5} {:>6} {:>12} {:>10} {:>5} {:>7}  key[:9]  header-decode",
        "#", "arch", "offset", "size", "bkt", "recon"
    );

    let mut ck_fail = 0u32;
    let mut key_mismatch = 0u32;
    let mut recon_entries = 0u32;
    let mut gap_count = 0u32;
    let mut prev_end: Option<(u16, u32)> = None;

    let total = entries.len();
    for (i, (bucket, entry)) in entries.iter().enumerate() {
        let arch = entry.archive_id();
        let off = entry.archive_offset();
        let size = entry.size;
        let is_recon = size == 30 && off < SEGMENT_HEADER_SIZE as u32;

        // Decode the 30-byte header at the idx-specified location
        let mut hdr_str: String;
        if let Some(raw) = data_files.read_range(arch, off, LOCAL_HEADER_SIZE) {
            if let Some(lh) = LocalHeader::from_bytes(&raw[..LOCAL_HEADER_SIZE]) {
                let on_disk_key = lh.original_encoding_key();
                let arr: [u8; LOCAL_HEADER_SIZE] =
                    raw[..LOCAL_HEADER_SIZE].try_into().expect("30-byte header");
                let calc_a = LocalHeader::compute_checksum_a(&arr);
                let key_ok = on_disk_key[..9] == entry.key;
                let ck_ok = calc_a == lh.checksum_a;
                if !ck_ok {
                    ck_fail += 1;
                }
                if !key_ok {
                    key_mismatch += 1;
                }
                if is_recon {
                    recon_entries += 1;
                }
                hdr_str = format!(
                    "flags=0x{:04x} ckA={:08x}{}{}",
                    lh.flags,
                    lh.checksum_a,
                    if ck_ok { "" } else { " BAD_CKA" },
                    if key_ok { "" } else { " KEY_MISMATCH" }
                );
            } else {
                hdr_str = "BAD_HEADER".to_string();
                ck_fail += 1;
            }
        } else {
            hdr_str = "NO_HEADER".to_string();
            ck_fail += 1;
        }

        // Contiguity: consecutive entries in the same file must be adjacent
        if let Some((pa, pe)) = prev_end {
            if pa == arch && off > pe {
                gap_count += 1;
                use std::fmt::Write as _;
                let _ = write!(hdr_str, " GAP_FROM_{pe}");
            }
        }
        prev_end = Some((arch, off + size));

        let is_interesting = i < 5 || i >= total - 5 || is_recon;
        if is_interesting {
            println!(
                "{i:>5} {arch:>6} {off:>12} {size:>10} {bucket:>5} {:>7}  {}  {}",
                if is_recon { "RECON" } else { "" },
                common::hex_str(&entry.key),
                hdr_str
            );
        }
    }

    println!(
        "\n  walked {total} entries: checksum_a failures={ck_fail}, key mismatches={key_mismatch}, reconstruction entries={recon_entries}, gaps={gap_count}"
    );

    // ------------------------------------------------------------------
    // 4. Per-bucket summary
    // ------------------------------------------------------------------
    println!("\n=== 4. Per-bucket summary ===");
    let mut grand_total = 0u64;
    let mut grand_bytes = 0u64;
    for bucket in 0..16u8 {
        let count: u64 = mgr.bucket_entry_count(bucket) as u64;
        let mut bytes = 0u64;
        for (b, e) in entries.iter() {
            if *b == bucket {
                bytes += u64::from(e.size);
            }
        }
        grand_total += count;
        grand_bytes += bytes;
        println!("  bucket {bucket:02x}: {count:>6} entries, {bytes:>12} bytes");
    }
    println!("\n  TOTAL: {grand_total} entries, {grand_bytes} bytes indexed");
    let disk_bytes: u64 = seg_data_sizes.values().sum();
    println!("  data files on disk: {disk_bytes} bytes");
    // u64 -> f64 is exact here (values ~3.8e9 < 2^52), but clippy's
    // cast-precision-loss fires regardless; ratio is ~1.0 so report as permille.
    let ratio_permille = grand_bytes.saturating_mul(1000) / disk_bytes.max(1);
    println!(
        "  indexed / on-disk ratio: {}.{:03}",
        ratio_permille / 1000,
        ratio_permille % 1000
    );
}

/// Parse a `data.NNN` filename into its segment index.
fn parse_data_name(name: &str) -> Option<u16> {
    let rest = name.strip_prefix("data.")?;
    if rest.chars().all(|c| c.is_ascii_digit()) {
        rest.parse::<u16>().ok()
    } else {
        None
    }
}
