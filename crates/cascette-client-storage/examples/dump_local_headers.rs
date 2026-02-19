#![allow(clippy::expect_used, clippy::panic)]

//! Dump individual 30-byte LocalHeader entries from data.NNN files.
//!
//! Reads the first 480 bytes (16 x 30-byte headers) from each segment file
//! and prints each non-zero header with round-trip verification.
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow cargo run --example dump_local_headers \
//!       -p cascette-client-storage --features local-install

mod common;

use cascette_client_storage::storage::{
    LocalHeader, SEGMENT_HEADER_SIZE, local_header::LOCAL_HEADER_SIZE, parse_data_filename,
};
use std::fs;

fn main() {
    let data = common::data_path();
    println!("Scanning local headers in: {}\n", data.display());

    let mut files: Vec<(u16, std::path::PathBuf)> = Vec::new();
    let dir = fs::read_dir(&data).expect("failed to read data directory");
    for entry in dir {
        let entry = entry.expect("failed to read dir entry");
        let name = entry.file_name();
        if let Some(idx) = parse_data_filename(&name.to_string_lossy()) {
            files.push((idx, entry.path()));
        }
    }
    files.sort_by_key(|(idx, _)| *idx);

    let mut rt_fail = 0u32;
    let mut rt_total = 0u32;

    for &(seg_idx, ref path) in &files {
        let meta = fs::metadata(path).expect("failed to stat file");
        if meta.len() < SEGMENT_HEADER_SIZE as u64 {
            continue;
        }

        let raw = {
            let full = fs::read(path).expect("failed to read file");
            full[..SEGMENT_HEADER_SIZE].to_vec()
        };

        let mut has_nonzero = false;
        for bucket in 0..16u8 {
            let offset = bucket as usize * LOCAL_HEADER_SIZE;
            let slice = &raw[offset..offset + LOCAL_HEADER_SIZE];

            let Some(lh) = LocalHeader::from_bytes(slice) else {
                continue;
            };

            let ekey = lh.original_encoding_key();
            if ekey.iter().all(|&b| b == 0) {
                continue;
            }

            if !has_nonzero {
                println!("Segment {seg_idx}:");
                has_nonzero = true;
            }

            println!(
                "  [{bucket:>2}] EKey={} blte_size={:>8} flags=0x{:04x} ckA=0x{:08x} ckB=0x{:08x}",
                common::hex_str(&ekey),
                lh.blte_size(),
                lh.flags,
                lh.checksum_a,
                lh.checksum_b,
            );

            // Round-trip: from_bytes -> to_bytes -> compare
            rt_total += 1;
            let rt = lh.to_bytes();
            if rt[..] != slice[..] {
                rt_fail += 1;
                println!("    ROUND-TRIP MISMATCH at segment {seg_idx} bucket {bucket}");
                for (i, (a, b)) in slice.iter().zip(rt.iter()).enumerate() {
                    if a != b {
                        println!(
                            "      first diff at byte {i}: original=0x{a:02x} roundtrip=0x{b:02x}"
                        );
                        break;
                    }
                }
            }
        }
        if has_nonzero {
            println!();
        }
    }

    println!("=== Round-Trip Summary ===");
    if rt_total == 0 {
        println!("  No non-zero local headers found.");
    } else if rt_fail == 0 {
        println!("  PASS: {rt_total}/{rt_total} local headers round-tripped");
    } else {
        println!("  FAIL: {rt_fail}/{rt_total} local headers did not round-trip");
    }
}
