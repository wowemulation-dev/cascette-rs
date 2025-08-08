//! Example demonstrating adaptive compression algorithm selection
//!
//! This example shows how the BLTE adaptive compression system analyzes
//! data characteristics to automatically select the best compression algorithm.

use blte::{
    adaptive::{analyze_data, auto_compress, compress_with_best_ratio, select_compression_mode},
    builder::{BLTEBuilder, CompressionStrategy},
    decompress_blte,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BLTE Adaptive Compression Example\n");
    println!("=================================\n");

    // Create various test data types
    let test_cases = vec![
        ("text", create_text_data()),
        ("json", create_json_data()),
        ("binary", create_binary_data()),
        ("zeros", create_zero_data()),
        ("random", create_random_data()),
        ("repetitive", create_repetitive_data()),
    ];

    for (name, data) in test_cases {
        println!("Testing {}: {} bytes", name, data.len());
        println!("{}", "-".repeat(40));

        // Analyze data characteristics
        let analysis = analyze_data(&data);
        println!("  Entropy: {:.3}", analysis.entropy);
        println!("  Zero ratio: {:.1}%", analysis.zero_ratio * 100.0);
        println!("  Repetition: {:.1}%", analysis.repetition_ratio * 100.0);
        println!("  Is text: {}", analysis.is_text);
        println!("  Is compressed: {}", analysis.is_compressed);
        if let Some(file_type) = analysis.file_type {
            println!("  File type: {:?}", file_type);
        }

        // Get compression recommendation
        let recommendation = select_compression_mode(&analysis);
        println!("\n  Recommendation: {:?}", recommendation.mode);
        if let Some(level) = recommendation.level {
            println!("  Level: {}", level);
        }
        println!(
            "  Expected ratio: {:.1}%",
            recommendation.expected_ratio * 100.0
        );
        println!("  Rationale: {}", recommendation.rationale);

        // Auto-compress the data
        let compressed = auto_compress(&data)?;
        let actual_ratio = 1.0 - (compressed.len() as f64 / data.len() as f64);
        println!("\n  Actual compression ratio: {:.1}%", actual_ratio * 100.0);
        println!("  Compressed size: {} bytes", compressed.len());

        // Verify decompression
        let decompressed = decompress_blte(compressed.clone(), None)?;
        assert_eq!(decompressed, data, "Decompression verification failed");
        println!("  ✓ Decompression verified");

        // Test best ratio selection
        let (best_compressed, best_mode) = compress_with_best_ratio(&data)?;
        println!("\n  Best mode: {:?}", best_mode);
        println!("  Best size: {} bytes", best_compressed.len());

        println!();
    }

    // Demonstrate builder with adaptive strategy
    println!("\nBuilder with Adaptive Strategy");
    println!("{}", "=".repeat(40));

    let _mixed_data = create_mixed_data();

    // Create builder with adaptive strategy
    let builder = BLTEBuilder::new()
        .with_compression_strategy(CompressionStrategy::Auto)
        .add_data(create_text_data())
        .add_data(create_binary_data())
        .add_data(create_zero_data());

    let result = builder.build()?;
    println!("Built multi-chunk BLTE with adaptive compression");
    println!("Total size: {} bytes", result.len());

    // Verify it decompresses correctly
    let decompressed = decompress_blte(result.clone(), None)?;
    println!("Decompressed size: {} bytes", decompressed.len());
    println!("✓ Multi-chunk decompression verified");

    Ok(())
}

fn create_text_data() -> Vec<u8> {
    br#"
    Lorem ipsum dolor sit amet, consectetur adipiscing elit. 
    Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.
    Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.
    "#
    .repeat(10)
    .to_vec()
}

fn create_json_data() -> Vec<u8> {
    br#"{
        "name": "Test Object",
        "version": "1.0.0",
        "features": ["compression", "encryption", "streaming"],
        "metadata": {
            "created": "2024-01-01",
            "author": "BLTE System",
            "tags": ["test", "example", "adaptive"]
        }
    }"#
    .repeat(5)
    .to_vec()
}

fn create_binary_data() -> Vec<u8> {
    let mut data = Vec::new();
    for i in 0..1000 {
        data.extend_from_slice(&(i as u32).to_le_bytes());
        data.extend_from_slice(&(i as u16).to_be_bytes());
    }
    data
}

fn create_zero_data() -> Vec<u8> {
    let mut data = vec![0u8; 5000];
    // Add some non-zero data
    for i in (0..data.len()).step_by(100) {
        data[i] = (i % 256) as u8;
    }
    data
}

fn create_random_data() -> Vec<u8> {
    (0..1000)
        .map(|i| {
            // Pseudo-random but deterministic
            ((i * 7919 + 1009) % 256) as u8
        })
        .collect()
}

fn create_repetitive_data() -> Vec<u8> {
    let pattern = b"ABCD";
    pattern.repeat(500).to_vec()
}

fn create_mixed_data() -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&create_text_data());
    data.extend_from_slice(&create_binary_data());
    data.extend_from_slice(&create_zero_data());
    data
}
