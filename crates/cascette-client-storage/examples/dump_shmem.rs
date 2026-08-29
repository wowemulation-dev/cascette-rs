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
    // Look for shmem files in the data directory. The 1.13.x client writes a
    // plain `shmem` file (no dot) at Data/data/shmem; some builds/agents use
    // `.shmem` or `Data/shmem/`. Match all three, skipping lock files.
    let mut shmem_files: Vec<std::path::PathBuf> = Vec::new();

    fn is_shmem_file(name: &str) -> bool {
        let ext_is_shmem = std::path::Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("shmem"));
        (name == "shmem" || ext_is_shmem) && !name.ends_with(".shmem.lock")
    }

    let dir = fs::read_dir(&data).expect("failed to read data directory");
    for entry in dir {
        let entry = entry.expect("failed to read dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if is_shmem_file(&name) {
            shmem_files.push(entry.path());
        }
    }

    if shmem_files.is_empty() {
        // Try the parent Data directory
        let parent = data.parent().expect("data path has no parent");
        let dir = fs::read_dir(parent).expect("failed to read Data directory");
        for entry in dir {
            let entry = entry.expect("failed to read dir entry");
            let name = entry.file_name().to_string_lossy().to_string();
            if is_shmem_file(&name) {
                shmem_files.push(entry.path());
            }
        }
    }

    if shmem_files.is_empty() {
        println!("No shmem file found in {} or parent", data.display());
        println!(
            "The shmem file is only present while the game client or the Blizzard Agent is running."
        );
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

        // Round-trip verification of the fields cascette-rs models.
        //
        // The 1.13.2 client also writes fields this crate does not model yet:
        //   - 0x04: u32 header size (0x150 = 336)
        //   - 0x08: path string ("Global\\../Data/data", relative)
        //   - 0x110: 16 x u32 per-bucket generations
        //   - 0x150/0x154: v4 tail words (1, 0xFE)
        // These are printed below and excluded from the round-trip compare so
        // the check validates the modeled contract only.
        let mut rt_buf = vec![0u8; raw.len()];
        cb.to_mapped(&mut rt_buf);

        // Modeled fields: version (0x00), init (0x02), free-space format
        // (0x108), data size (0x10C). Compare them field by field.
        let version_ok = raw[0x00] == rt_buf[0x00];
        let init_ok = raw[0x02] == rt_buf[0x02];
        let fst_ok = raw[0x108..0x10C] == rt_buf[0x108..0x10C];
        let ds_ok = raw[0x10C..0x110] == rt_buf[0x10C..0x110];
        let modeled_ok = version_ok && init_ok && fst_ok && ds_ok;
        println!(
            "  Round-trip (modeled fields): {}",
            if modeled_ok { "PASS" } else { "FAIL" }
        );
        if !version_ok {
            println!("    version: raw={:02x} rt={:02x}", raw[0x00], rt_buf[0x00]);
        }
        if !init_ok {
            println!("    init: raw={:02x} rt={:02x}", raw[0x02], rt_buf[0x02]);
        }
        if !fst_ok {
            println!(
                "    free-space-format: raw={:02x?} rt={:02x?}",
                &raw[0x108..0x10C],
                &rt_buf[0x108..0x10C]
            );
        }
        if !ds_ok {
            println!(
                "    data-size: raw={:02x?} rt={:02x?}",
                &raw[0x10C..0x110],
                &rt_buf[0x10C..0x110]
            );
        }

        // Client-specific fields (informational, not yet modeled)
        if raw.len() >= 0x154 {
            let hdr_size = u32::from_le_bytes(raw[0x04..0x08].try_into().expect("4 bytes"));
            let path_end = raw[0x08..0x108]
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(0x108 - 0x08);
            let path = String::from_utf8_lossy(&raw[0x08..0x08 + path_end]);
            let tail_150 = u32::from_le_bytes(raw[0x150..0x154].try_into().expect("4 bytes"));
            println!("  Client fields (not modeled):");
            println!("    header size: {hdr_size} (0x{hdr_size:x})");
            println!("    path:        {path}");
            let mut gens: Vec<u32> = Vec::new();
            for i in 0..16 {
                gens.push(u32::from_le_bytes(
                    raw[0x110 + i * 4..0x114 + i * 4]
                        .try_into()
                        .expect("4 bytes"),
                ));
            }
            println!("    generations: {gens:?}");
            println!("    tail[0x150]: 0x{tail_150:08x}");
        }
        println!();
    }
}
