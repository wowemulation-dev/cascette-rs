#![allow(clippy::expect_used, clippy::panic, clippy::cast_lossless)]
//! CDN archive index example
//!
//! Demonstrates building an archive index with ArchiveIndexBuilder,
//! writing it to a buffer, parsing it back, and performing lookups.
//!
//! ```text
//! cargo run -p cascette-formats --example archive_index
//! ```

use cascette_formats::archive::{ArchiveIndex, ArchiveIndexBuilder, IndexFooter};
use std::io::Cursor;

fn main() {
    println!("=== Building Archive Index ===");

    let mut builder = ArchiveIndexBuilder::new();

    // Add entries with 16-byte encoding keys, sizes, and offsets
    let test_entries: Vec<([u8; 16], u32, u64)> = (0u8..25)
        .map(|i| {
            let mut key = [0u8; 16];
            key[0] = i;
            key[1] = i.wrapping_mul(17);
            key[15] = i.wrapping_mul(31);
            let size = 1024 * (i as u32 + 1);
            let offset = 4096 * i as u64;
            (key, size, offset)
        })
        .collect();

    for (key, size, offset) in &test_entries {
        builder.add_entry(key.to_vec(), *size, *offset);
    }

    println!("Entries added:    {}", test_entries.len());

    // Build the index into a buffer
    let mut output = Cursor::new(Vec::new());
    let index = builder
        .build(&mut output)
        .expect("index build should succeed");

    let index_data = output.into_inner();
    println!("Index size:       {} bytes", index_data.len());
    println!("Entry count:      {}", index.entries.len());
    println!("TOC entries:      {}", index.toc.len());

    println!();
    println!("=== Index Footer ===");
    println!("Version:          {}", index.footer.version);
    println!("Page size:        {} KB", index.footer.page_size_kb);
    println!("EKey length:      {} bytes", index.footer.ekey_length);
    println!("Offset bytes:     {}", index.footer.offset_bytes);
    println!("Size bytes:       {}", index.footer.size_bytes);
    println!("Element count:    {}", index.footer.element_count);
    println!("Footer hash bytes: {}", index.footer.footer_hash_bytes);
    println!("Footer valid:     {}", index.footer.is_valid());
    println!("Is archive-group: {}", index.footer.is_archive_group());

    println!();
    println!("=== Parsing Archive Index ===");

    let mut cursor = Cursor::new(&index_data);
    let parsed = ArchiveIndex::parse(&mut cursor).expect("index parse should succeed");

    println!("Parsed entries:   {}", parsed.entries.len());
    println!("Parsed TOC:       {}", parsed.toc.len());
    println!(
        "Footer matches:   {}",
        parsed.footer.element_count == index.footer.element_count
    );

    // Verify entries are sorted
    let is_sorted = parsed.entries.windows(2).all(|w| w[0] <= w[1]);
    println!("Entries sorted:   {is_sorted}");

    println!();
    println!("=== Entry Lookups ===");

    // Look up specific entries
    for i in [0usize, 5, 12, 24] {
        let (key, expected_size, expected_offset) = &test_entries[i];
        let found = parsed.find_entry(key);
        match found {
            Some(entry) => {
                let size_ok = entry.size == *expected_size;
                let offset_ok = entry.offset == *expected_offset;
                println!(
                    "  Key[{i:>2}]: size={:>6} offset={:>6} (size_ok={size_ok}, offset_ok={offset_ok})",
                    entry.size, entry.offset,
                );
            }
            None => {
                println!("  Key[{i:>2}]: not found");
            }
        }
    }

    // Look up a non-existent key
    let missing_key = [0xFF; 16];
    let not_found = parsed.find_entry(&missing_key);
    println!("  Missing key:    found={}", not_found.is_some());

    println!();
    println!("=== Index Entries (first 10) ===");

    for (i, entry) in parsed.entries.iter().take(10).enumerate() {
        println!(
            "  [{i:>2}] key={} size={:>6} offset={:>6}",
            hex::encode(&entry.encoding_key),
            entry.size,
            entry.offset,
        );
    }

    println!();
    println!("=== Table of Contents ===");

    for (i, toc_key) in parsed.toc.iter().enumerate() {
        println!("  Chunk [{i}] last_key={}", hex::encode(toc_key));
    }

    println!();
    println!("=== Footer Validation ===");

    // Create a footer manually and validate
    let footer = IndexFooter::new(vec![0xAA; 8], 100);
    println!("Manual footer version:  {}", footer.version);
    println!("Manual footer valid:    {}", footer.is_valid());
    match footer.validate_format() {
        Ok(()) => println!("Format validation:      passed"),
        Err(e) => println!("Format validation:      failed ({e})"),
    }

    println!();
    println!("=== Rebuild from Existing Index ===");

    let mut rebuilt_builder = ArchiveIndexBuilder::from_archive_index(&parsed);

    // Add an extra entry
    let extra_key = [0xFE; 16];
    rebuilt_builder.add_entry(extra_key.to_vec(), 9999, 88888);

    let mut rebuilt_output = Cursor::new(Vec::new());
    let rebuilt_index = rebuilt_builder
        .build(&mut rebuilt_output)
        .expect("rebuilt index should succeed");

    println!(
        "Rebuilt entries:  {} (was {})",
        rebuilt_index.entries.len(),
        parsed.entries.len()
    );

    // Verify the new entry exists
    let found_extra = rebuilt_index.find_entry(&extra_key);
    println!(
        "Extra entry found: {} (size={})",
        found_extra.is_some(),
        found_extra.map_or(0, |e| e.size),
    );
}
