#![allow(clippy::expect_used, clippy::panic, clippy::cast_precision_loss)]
//! BLTE container format example
//!
//! Demonstrates creating single-chunk and multi-chunk BLTE files with
//! different compression modes and verifying round-trip correctness.
//!
//! ```text
//! cargo run -p cascette-formats --example blte_encoding
//! ```

use cascette_formats::CascFormat;
use cascette_formats::blte::{BlteBuilder, BlteFile, CompressionMode};

fn main() {
    println!("=== Single-Chunk BLTE (No Compression) ===");

    let data = b"Hello, BLTE! This is uncompressed content stored in a single chunk.";
    let blte = BlteFile::single_chunk(data.to_vec(), CompressionMode::None)
        .expect("single_chunk should succeed");

    println!("Is single chunk:  {}", blte.header.is_single_chunk());
    println!("Chunk count:      {}", blte.chunks.len());
    println!("Data offset:      {} bytes", blte.header.data_offset());

    let decompressed = blte.decompress().expect("decompress should succeed");
    println!("Decompressed size: {} bytes", decompressed.len());
    println!("Content matches:  {}", decompressed == data);

    // Serialize and parse back using CascFormat trait
    let built = blte.build().expect("CascFormat::build should succeed");
    println!("Serialized size:  {} bytes", built.len());

    let parsed = BlteFile::parse(&built).expect("CascFormat::parse should succeed");
    let re_decompressed = parsed.decompress().expect("re-decompress should succeed");
    println!("Round-trip OK:    {}", re_decompressed == data);

    println!();
    println!("=== Single-Chunk BLTE (ZLib Compression) ===");

    let text = b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA repetitive data compresses well";
    let blte_zlib = BlteFile::single_chunk(text.to_vec(), CompressionMode::ZLib)
        .expect("zlib single_chunk should succeed");

    let zlib_built = blte_zlib.build().expect("build should succeed");
    println!("Original size:    {} bytes", text.len());
    println!("BLTE size:        {} bytes", zlib_built.len());
    println!(
        "Compression:      ZLib (saved {} bytes)",
        text.len().saturating_sub(zlib_built.len())
    );

    let zlib_parsed = BlteFile::parse(&zlib_built).expect("parse should succeed");
    let zlib_decompressed = zlib_parsed.decompress().expect("decompress should succeed");
    println!("Decompressed OK:  {}", zlib_decompressed == text);

    println!();
    println!("=== Multi-Chunk BLTE (Automatic Chunking) ===");

    let large_data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
    let blte_multi = BlteFile::compress(&large_data, 4096, CompressionMode::ZLib)
        .expect("compress should succeed");

    println!("Input size:       {} bytes", large_data.len());
    println!("Is single chunk:  {}", blte_multi.header.is_single_chunk());
    println!("Chunk count:      {}", blte_multi.chunks.len());

    for (i, chunk) in blte_multi.chunks.iter().enumerate() {
        println!(
            "  Chunk {i}: mode={:?}, compressed={} bytes",
            chunk.mode,
            chunk.data.len()
        );
    }

    let multi_decompressed = blte_multi.decompress().expect("decompress should succeed");
    println!("Decompressed OK:  {}", multi_decompressed == large_data);

    // Round-trip the multi-chunk file
    let multi_built = blte_multi.build().expect("build should succeed");
    let multi_parsed = BlteFile::parse(&multi_built).expect("parse should succeed");
    let multi_re_decomp = multi_parsed
        .decompress()
        .expect("re-decompress should succeed");
    println!("Round-trip OK:    {}", multi_re_decomp == large_data);

    println!();
    println!("=== BlteBuilder API ===");

    let builder_data = b"Built with the BlteBuilder API for fine-grained control.";
    let built_blte = BlteBuilder::new()
        .with_compression(CompressionMode::ZLib)
        .add_data(builder_data)
        .expect("add_data should succeed")
        .build()
        .expect("builder build should succeed");

    println!("Builder chunk count: {}", built_blte.chunks.len());
    let builder_decomp = built_blte.decompress().expect("decompress should succeed");
    println!("Content matches:     {}", builder_decomp == builder_data);

    println!();
    println!("=== LZ4 Compression Mode ===");

    let lz4_data: Vec<u8> = (0..5_000).map(|i| (i % 128) as u8).collect();
    let blte_lz4 = BlteFile::single_chunk(lz4_data.clone(), CompressionMode::LZ4)
        .expect("lz4 single_chunk should succeed");

    let lz4_built = blte_lz4.build().expect("build should succeed");
    let lz4_parsed = BlteFile::parse(&lz4_built).expect("parse should succeed");
    let lz4_decomp = lz4_parsed.decompress().expect("decompress should succeed");
    println!("LZ4 original:    {} bytes", lz4_data.len());
    println!("LZ4 BLTE size:   {} bytes", lz4_built.len());
    println!("LZ4 round-trip:  {}", lz4_decomp == lz4_data);

    println!();
    println!("=== Compression Mode Comparison ===");

    let test_data: Vec<u8> = (0..8_000).map(|i| (i % 200) as u8).collect();

    for mode in [
        CompressionMode::None,
        CompressionMode::ZLib,
        CompressionMode::LZ4,
    ] {
        let blte_cmp =
            BlteFile::single_chunk(test_data.clone(), mode).expect("single_chunk should succeed");
        let cmp_built = blte_cmp.build().expect("build should succeed");
        println!(
            "  {:?}: {} bytes (ratio {:.2}x)",
            mode,
            cmp_built.len(),
            cmp_built.len() as f64 / test_data.len() as f64
        );
    }
}
