#![allow(clippy::expect_used, clippy::panic)]

//! Read sample entries from the full local storage stack.
//!
//! Opens the installation, loads indices and archives, then reads a
//! handful of sample entries to exercise the full read pipeline.
//!
//! Usage:
//!   CASCETTE_WOW_PATH=/path/to/wow cargo run --example read_archives \
//!       -p cascette-client-storage --features local-install

mod common;

use cascette_client_storage::Installation;

#[tokio::main]
async fn main() {
    let wow = common::wow_path();
    let data_root = wow.join("Data");

    println!("Opening installation: {}", data_root.display());

    let install = Installation::open(data_root).expect("failed to open installation");
    install
        .initialize()
        .await
        .expect("failed to initialize installation");

    let stats = install.stats().await;
    println!("  Index files:   {}", stats.index_files);
    println!("  Index entries: {}", stats.index_entries);
    println!("  Archive files: {}", stats.archive_files);
    println!("  Archive size:  {} bytes", stats.archive_size);
    println!();

    // Get all index entries, pick samples
    let all_entries = install.get_all_index_entries().await;
    if all_entries.is_empty() {
        println!("No index entries found.");
        return;
    }

    println!("Sampling {} of {} entries:\n", 10.min(all_entries.len()), all_entries.len());

    // First 5 + last 5 (or fewer if less than 10 total)
    let sample_count = 5.min(all_entries.len());
    let mut samples: Vec<_> = all_entries[..sample_count].to_vec();
    if all_entries.len() > sample_count {
        let tail_start = all_entries.len().saturating_sub(sample_count);
        // Avoid duplicates if total < 10
        if tail_start >= sample_count {
            samples.extend_from_slice(&all_entries[tail_start..]);
        }
    }

    for (i, entry) in samples.iter().enumerate() {
        println!(
            "[{i}] EKey={} archive={} offset={} size={}",
            common::hex_str(&entry.key),
            entry.archive_id(),
            entry.archive_offset(),
            entry.size,
        );

        match install
            .read_from_archive(entry.archive_id(), entry.archive_offset(), entry.size)
            .await
        {
            Ok(data) => {
                println!("    Read {} bytes:", data.len());
                common::hex_dump(&data, 64);
            }
            Err(e) => {
                println!("    Read error: {e}");
            }
        }
        println!();
    }
}
