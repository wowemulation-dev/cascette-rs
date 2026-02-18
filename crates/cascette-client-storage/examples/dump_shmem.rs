#![allow(clippy::expect_used, clippy::panic)]

//! Dump the shmem control block from a local WoW installation.
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow cargo run --example dump_shmem \
//!       -p cascette-client-storage --features local-install

mod common;

use cascette_client_storage::shmem::control_block::ShmemControlBlock;
use std::fs;

fn main() {
    let data = common::data_path();

    // Look for shmem files in the data directory
    let dir = fs::read_dir(&data).expect("failed to read data directory");
    let mut shmem_files: Vec<std::path::PathBuf> = Vec::new();

    for entry in dir {
        let entry = entry.expect("failed to read dir entry");
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.ends_with(".shmem") && !name_str.ends_with(".shmem.lock") {
            shmem_files.push(entry.path());
        }
    }

    if shmem_files.is_empty() {
        // Try the parent Data directory
        let parent = data.parent().expect("data path has no parent");
        let dir = fs::read_dir(parent).expect("failed to read Data directory");
        for entry in dir {
            let entry = entry.expect("failed to read dir entry");
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".shmem") && !name_str.ends_with(".shmem.lock") {
                shmem_files.push(entry.path());
            }
        }
    }

    if shmem_files.is_empty() {
        println!("No .shmem files found in {} or parent", data.display());
        println!("The shmem file is only present while the game client or Agent.exe is running.");
        return;
    }

    shmem_files.sort();

    for path in &shmem_files {
        println!("Reading: {}", path.display());
        let raw = fs::read(path).expect("failed to read shmem file");
        println!("  File size: {} bytes", raw.len());

        let Some(cb) = ShmemControlBlock::from_mapped(&raw) else {
            println!("  Failed to parse control block (invalid or unsupported version)\n");
            continue;
        };

        println!("  Version:     {}", cb.version());
        println!("  Initialized: {}", cb.is_initialized());
        println!("  Data size:   {}", cb.data_size());
        println!("  Exclusive:   {}", cb.is_exclusive());
        println!("  File size:   {} (computed)", cb.file_size());

        if let Some(pid) = cb.pid_tracking() {
            println!("  --- PID Tracking (v5) ---");
            println!("    State:        {}", pid.state);
            println!("    Writer count: {}", pid.writer_count);
            println!("    Total count:  {}", pid.total_count);
            println!("    Generation:   {}", pid.generation);
            println!("    Max slots:    {}", pid.max_slots);
            let active: Vec<_> = pid
                .pids
                .iter()
                .enumerate()
                .filter(|(_, p)| **p != 0)
                .collect();
            if active.is_empty() {
                println!("    PIDs:         (none active)");
            } else {
                for (slot, pid_val) in &active {
                    let mode = pid.modes.get(*slot).copied().unwrap_or(0);
                    let mode_str = if mode == 2 { "read-only" } else { "read-write" };
                    println!("    Slot {slot:>2}: PID={pid_val:<8} mode={mode_str}");
                }
            }
        }

        // Round-trip verification
        let mut rt_buf = vec![0u8; raw.len()];
        cb.to_mapped(&mut rt_buf);

        // Compare only the header portion (the rest is free space table data)
        let header_size = match cb.version() {
            5 => {
                if cb.pid_tracking().is_some() {
                    cascette_client_storage::shmem::control_block::V5_EXTENDED_HEADER_SIZE
                } else {
                    cascette_client_storage::shmem::control_block::V5_BASE_HEADER_SIZE
                }
            }
            _ => cascette_client_storage::shmem::control_block::V4_HEADER_SIZE,
        };

        let cmp_len = header_size.min(raw.len()).min(rt_buf.len());
        if raw[..cmp_len] == rt_buf[..cmp_len] {
            println!("  Round-trip: PASS (header {cmp_len} bytes match)");
        } else {
            println!("  Round-trip: FAIL");
            for (i, (a, b)) in raw[..cmp_len].iter().zip(rt_buf[..cmp_len].iter()).enumerate() {
                if a != b {
                    println!(
                        "    first diff at offset {i:#x}: original=0x{a:02x} roundtrip=0x{b:02x}"
                    );
                    break;
                }
            }
        }
        println!();
    }
}
