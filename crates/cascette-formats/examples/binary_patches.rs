#![allow(clippy::expect_used, clippy::panic, clippy::cast_precision_loss)]
//! ZBSDIFF1 binary patching example
//!
//! Demonstrates creating binary differential patches with ZbsdiffBuilder,
//! applying them with apply_patch_memory, and analyzing patch characteristics.
//!
//! ```text
//! cargo run -p cascette-formats --example binary_patches
//! ```

use cascette_formats::zbsdiff::{ZbsDiff, ZbsdiffBuilder, ZbsdiffPatcher, apply_patch_memory};
use std::io::Cursor;

fn main() {
    println!("=== Simple Patch (text change) ===");

    let old_data = b"Hello, World! This is version 1.0 of our game data file.";
    let new_data = b"Hello, Rust!  This is version 2.0 of our game data file.";

    let builder = ZbsdiffBuilder::new(old_data.to_vec(), new_data.to_vec());
    let patch_data = builder.build().expect("patch build should succeed");

    println!("Old size:         {} bytes", old_data.len());
    println!("New size:         {} bytes", new_data.len());
    println!("Patch size:       {} bytes", patch_data.len());
    println!(
        "Compression:      {:.1}% of new size",
        (patch_data.len() as f64 / new_data.len() as f64) * 100.0
    );

    // Apply the patch
    let result = apply_patch_memory(old_data, &patch_data).expect("apply_patch should succeed");
    println!("Result matches:   {}", result == new_data);

    println!();
    println!("=== Patch Analysis ===");

    let analysis = builder.analyze_patch();
    println!("Old size:         {} bytes", analysis.old_size);
    println!("New size:         {} bytes", analysis.new_size);
    println!("Diff bytes:       {}", analysis.total_diff_bytes);
    println!("Extra bytes:      {}", analysis.total_extra_bytes);
    println!("Match bytes:      {}", analysis.total_match_bytes);
    println!("Match ratio:      {:.1}%", analysis.match_percentage());
    println!("Extra ratio:      {:.1}%", analysis.extra_percentage());
    println!("Compression:      {:.3}", analysis.compression_ratio);

    println!();
    println!("=== Parse Patch Structure ===");

    let patch = ZbsDiff::parse(&patch_data).expect("patch parse should succeed");
    println!("Output size:      {} bytes", patch.output_size());
    println!(
        "Control data:     {} bytes (compressed)",
        patch.control_data.len()
    );
    println!(
        "Diff data:        {} bytes (compressed)",
        patch.diff_data.len()
    );
    println!(
        "Extra data:       {} bytes (compressed)",
        patch.extra_data.len()
    );

    // Decompress and inspect control block
    let control = patch
        .control_block()
        .expect("control block should decompress");
    println!("Control entries:  {}", control.entries.len());
    for (i, entry) in control.entries.iter().enumerate() {
        println!(
            "  [{i}] diff_size={} extra_size={} seek={}",
            entry.diff_size, entry.extra_size, entry.seek_offset
        );
    }

    // Round-trip: build the patch back to bytes
    let rebuilt_patch = patch.build().expect("patch rebuild should succeed");
    println!("Rebuild matches:  {}", rebuilt_patch == patch_data);

    // Apply using the ZbsDiff::apply method
    let applied = patch.apply(old_data).expect("apply should succeed");
    println!("Apply matches:    {}", applied == new_data);

    println!();
    println!("=== Large Binary Patch ===");

    // Simulate a version update of a binary file
    let old_binary: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
    let mut new_binary = old_binary.clone();
    // Modify a section in the middle
    for byte in new_binary.iter_mut().take(4500).skip(4000) {
        *byte = byte.wrapping_add(42);
    }
    // Append new data
    new_binary.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    let large_builder = ZbsdiffBuilder::new(old_binary.clone(), new_binary.clone());
    let large_patch = large_builder.build().expect("large patch should succeed");

    println!("Old size:         {} bytes", old_binary.len());
    println!("New size:         {} bytes", new_binary.len());
    println!("Patch size:       {} bytes", large_patch.len());
    println!(
        "Compression:      {:.1}% of new size",
        (large_patch.len() as f64 / new_binary.len() as f64) * 100.0
    );

    let large_result =
        apply_patch_memory(&old_binary, &large_patch).expect("large apply should succeed");
    println!("Result matches:   {}", large_result == new_binary);

    let large_analysis = large_builder.analyze_patch();
    println!(
        "Match ratio:      {:.1}%",
        large_analysis.match_percentage()
    );

    println!();
    println!("=== Streaming Patcher ===");

    // Use the streaming patcher with a Cursor (simulating file I/O)
    let old_cursor = Cursor::new(old_data.to_vec());
    let patcher = ZbsdiffPatcher::new(old_cursor, new_data.len());
    let stream_result = patcher
        .apply_patch_from_data(&patch_data)
        .expect("streaming apply should succeed");
    println!("Streaming result: {}", stream_result == new_data);

    println!();
    println!("=== Identical Files (zero diff) ===");

    let same_data = b"This file has not changed between versions.";
    let same_builder = ZbsdiffBuilder::new(same_data.to_vec(), same_data.to_vec());
    let same_patch = same_builder.build().expect("same patch should succeed");
    println!("Same data size:   {} bytes", same_data.len());
    println!("Patch size:       {} bytes", same_patch.len());

    let same_result =
        apply_patch_memory(same_data, &same_patch).expect("same apply should succeed");
    println!("Result matches:   {}", same_result == same_data);

    println!();
    println!("=== Empty to Non-Empty ===");

    let empty_builder = ZbsdiffBuilder::new(vec![], b"Brand new file content!".to_vec());
    let empty_patch = empty_builder.build().expect("empty patch should succeed");
    println!("Patch size:       {} bytes", empty_patch.len());

    let empty_result = apply_patch_memory(&[], &empty_patch).expect("empty apply should succeed");
    println!("Result:           {:?}", std::str::from_utf8(&empty_result));

    println!();
    println!("=== Algorithm Comparison ===");

    let cmp_old = b"The quick brown fox jumps over the lazy dog. Version 1.";
    let cmp_new = b"The quick brown cat jumps over the lazy dog. Version 2.";
    let cmp_builder = ZbsdiffBuilder::new(cmp_old.to_vec(), cmp_new.to_vec());

    let simple_patch = cmp_builder
        .build_simple_patch()
        .expect("simple patch should succeed");
    let optimized_patch = cmp_builder.build().expect("optimized patch should succeed");

    println!("Simple patch:     {} bytes", simple_patch.len());
    println!("Optimized patch:  {} bytes", optimized_patch.len());

    // Both patches produce the same result
    let simple_result =
        apply_patch_memory(cmp_old, &simple_patch).expect("simple apply should succeed");
    let opt_result =
        apply_patch_memory(cmp_old, &optimized_patch).expect("optimized apply should succeed");
    println!("Simple correct:   {}", simple_result == cmp_new);
    println!("Optimized correct: {}", opt_result == cmp_new);
}
