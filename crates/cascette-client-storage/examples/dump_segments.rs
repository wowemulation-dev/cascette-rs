#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

//! Dump segment headers from data.NNN files in a local WoW installation.
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow cargo run --example dump_segments \
//!       -p cascette-client-storage --features local-install

mod common;

use cascette_client_storage::storage::{
    SegmentHeader, parse_data_filename, segment_data_path, SEGMENT_HEADER_SIZE,
};
use std::fs;

fn main() {
    let data = common::data_path();
    println!("Scanning segments in: {}", data.display());

    let mut entries: Vec<(u16, std::path::PathBuf)> = Vec::new();

    let dir = fs::read_dir(&data).expect("failed to read data directory");
    for entry in dir {
        let entry = entry.expect("failed to read dir entry");
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if let Some(idx) = parse_data_filename(&name_str) {
            entries.push((idx, entry.path()));
        }
    }
    entries.sort_by_key(|(idx, _)| *idx);

    println!("Found {} data files\n", entries.len());

    let mut rt_pass = 0u32;
    let mut rt_fail = 0u32;

    for &(idx, ref path) in &entries {
        let meta = fs::metadata(path).expect("failed to stat data file");
        let expected_path = segment_data_path(&data, idx);
        assert_eq!(
            path.file_name(),
            expected_path.file_name(),
            "path mismatch for segment {idx}"
        );

        println!(
            "Segment {idx:>4} ({}): {} bytes",
            path.file_name().unwrap().to_string_lossy(),
            meta.len()
        );

        if meta.len() < SEGMENT_HEADER_SIZE as u64 {
            println!("  (too small for header, skipping)\n");
            continue;
        }

        let raw = fs::read(path).expect("failed to read data file");
        let header_bytes = &raw[..SEGMENT_HEADER_SIZE];

        let header = SegmentHeader::from_bytes(header_bytes)
            .expect("failed to parse segment header");

        // Print 16 local headers
        for bucket in 0..16u8 {
            let lh = header.bucket_header(bucket);
            let ekey = lh.original_encoding_key();
            let all_zero = ekey.iter().all(|&b| b == 0);
            if all_zero {
                continue;
            }
            println!(
                "  bucket={bucket:>2} EKey={} size_hdr={:>8} flags=0x{:04x} ckA=0x{:08x} ckB=0x{:08x}",
                common::hex_str(&ekey),
                lh.size_with_header,
                lh.flags,
                lh.checksum_a,
                lh.checksum_b,
            );
        }

        // Round-trip
        let rt_bytes = header.to_bytes();
        if rt_bytes[..] == header_bytes[..] {
            rt_pass += 1;
        } else {
            rt_fail += 1;
            println!("  ROUND-TRIP MISMATCH for segment {idx}");
            // Show first difference
            for (i, (a, b)) in header_bytes.iter().zip(rt_bytes.iter()).enumerate() {
                if a != b {
                    println!("    first diff at offset {i:#x}: original=0x{a:02x} roundtrip=0x{b:02x}");
                    break;
                }
            }
        }
        println!();
    }

    let total = rt_pass + rt_fail;
    println!("=== Round-Trip Summary ===");
    if rt_fail == 0 {
        println!("  PASS: {total}/{total} segment headers round-tripped");
    } else {
        println!("  FAIL: {rt_fail}/{total} segment headers did not round-trip");
    }
}
