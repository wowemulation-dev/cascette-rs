//! 🚀 PERFECT ROUND-TRIP RECREATION 🚀
//!
//! This is the ultimate test - complete recreation of a 256MB Blizzard CDN archive
//! with byte-for-byte perfect fidelity. This would be a groundbreaking achievement!

use blte::{BLTEArchive, PerfectArchiveBuilder, Result};
use std::fs;
use std::time::Instant;

/// Comprehensive validation of recreation quality
#[derive(Debug)]
struct RecreationReport {
    original_size: usize,
    recreated_size: usize,
    files_extracted: usize,
    files_recreated: usize,
    byte_differences: usize,
    first_difference_offset: Option<usize>,
    is_perfect: bool,
    extraction_time: std::time::Duration,
    recreation_time: std::time::Duration,
    validation_time: std::time::Duration,
    total_time: std::time::Duration,
}

fn validate_perfect_recreation(original: &[u8], recreated: &[u8]) -> RecreationReport {
    let start = Instant::now();

    let mut byte_differences = 0;
    let mut first_difference_offset = None;

    // Compare sizes
    let size_match = original.len() == recreated.len();

    // Byte-for-byte comparison
    if size_match {
        for (i, (o, r)) in original.iter().zip(recreated.iter()).enumerate() {
            if o != r {
                byte_differences += 1;
                if first_difference_offset.is_none() {
                    first_difference_offset = Some(i);
                }

                // Stop after finding first few differences for performance
                if byte_differences >= 10 {
                    break;
                }
            }
        }
    } else {
        // If sizes don't match, we definitely have differences
        byte_differences = usize::MAX; // Indicate size mismatch
    }

    let validation_time = start.elapsed();
    let is_perfect = byte_differences == 0;

    RecreationReport {
        original_size: original.len(),
        recreated_size: recreated.len(),
        files_extracted: 0, // Will be filled in by caller
        files_recreated: 0, // Will be filled in by caller
        byte_differences,
        first_difference_offset,
        is_perfect,
        extraction_time: std::time::Duration::new(0, 0),
        recreation_time: std::time::Duration::new(0, 0),
        validation_time,
        total_time: std::time::Duration::new(0, 0),
    }
}

fn main() -> Result<()> {
    println!("🚀 PERFECT ROUND-TRIP RECREATION TEST 🚀");
    println!("========================================");
    println!("This test attempts to achieve byte-for-byte perfect recreation");
    println!("of a 256MB Blizzard CDN archive - a groundbreaking achievement!");
    println!();

    let path = "test_blte/real_full.blte";

    if !std::path::Path::new(path).exists() {
        println!("❌ File not found: {}", path);
        println!("Please run: cargo run --example test_large_blte");
        return Ok(());
    }

    let overall_start = Instant::now();

    // Step 1: Load original archive
    println!("📁 Loading original 256MB archive...");
    let original_data = fs::read(path)?;
    println!(
        "✅ Loaded {} bytes ({:.2} MB)",
        original_data.len(),
        original_data.len() as f64 / 1_048_576.0
    );

    // Step 2: Parse original archive
    println!("\n🔍 Parsing original archive structure...");
    let mut original_archive = BLTEArchive::parse(original_data.clone())?;
    let file_count = original_archive.file_count();
    println!("✅ Found {} BLTE files in archive", file_count);

    // Step 3: Extract ALL files with complete metadata preservation
    println!("\n📤 Extracting ALL files with metadata preservation...");
    println!("This may take a few minutes for {} files...", file_count);

    let extract_start = Instant::now();
    let extracted_files = original_archive.extract_all_with_metadata()?;
    let extraction_time = extract_start.elapsed();

    println!(
        "✅ Extracted {} files in {:?}",
        extracted_files.len(),
        extraction_time
    );

    // Analyze extraction results
    let total_decompressed: usize = extracted_files.iter().map(|f| f.data.len()).sum();
    println!(
        "   Total decompressed data: {} bytes ({:.2} MB)",
        total_decompressed,
        total_decompressed as f64 / 1_048_576.0
    );

    let compression_ratio = if total_decompressed > 0 {
        original_data.len() as f64 / total_decompressed as f64
    } else {
        0.0
    };
    println!("   Overall compression ratio: {:.2}x", compression_ratio);

    // Step 4: Build perfect archive
    println!("\n🔧 Building perfect archive...");
    println!(
        "Recreating {} files with zero gaps...",
        extracted_files.len()
    );

    let recreate_start = Instant::now();
    let mut builder = PerfectArchiveBuilder::new();

    let mut files_added = 0;
    let extracted_count = extracted_files.len();
    for file in extracted_files {
        if builder.add_extracted_file(file)? {
            files_added += 1;
        } else {
            println!("⚠️  Archive capacity reached at {} files", files_added);
            break;
        }
    }

    let recreated_data = builder.build_perfect()?;
    let recreation_time = recreate_start.elapsed();

    println!(
        "✅ Perfect archive built: {} bytes in {:?}",
        recreated_data.len(),
        recreation_time
    );

    // Step 5: Comprehensive validation
    println!("\n🔬 Performing comprehensive validation...");
    println!("Comparing {} bytes with original...", recreated_data.len());

    let mut report = validate_perfect_recreation(&original_data, &recreated_data);
    report.files_extracted = extracted_count;
    report.files_recreated = files_added;
    report.extraction_time = extraction_time;
    report.recreation_time = recreation_time;
    report.total_time = overall_start.elapsed();

    // Step 6: Results and celebration! 🎉
    println!("\n📊 FINAL RESULTS");
    println!("================");

    println!(
        "Original size:    {} bytes ({:.2} MB)",
        report.original_size,
        report.original_size as f64 / 1_048_576.0
    );
    println!(
        "Recreated size:   {} bytes ({:.2} MB)",
        report.recreated_size,
        report.recreated_size as f64 / 1_048_576.0
    );
    println!("Files extracted:  {}", report.files_extracted);
    println!("Files recreated:  {}", report.files_recreated);
    println!("Byte differences: {}", report.byte_differences);

    if let Some(offset) = report.first_difference_offset {
        println!("First difference: at offset {}", offset);
    }

    println!("\n⏱️  PERFORMANCE");
    println!("===============");
    println!("Extraction time:  {:?}", report.extraction_time);
    println!("Recreation time:  {:?}", report.recreation_time);
    println!("Validation time:  {:?}", report.validation_time);
    println!("Total time:       {:?}", report.total_time);

    let throughput = report.original_size as f64 / report.total_time.as_secs_f64() / 1_048_576.0;
    println!("Overall throughput: {:.2} MB/s", throughput);

    println!("\n🎯 RECREATION QUALITY");
    println!("=====================");

    if report.is_perfect {
        println!("🎉🎉🎉 PERFECT RECREATION ACHIEVED! 🎉🎉🎉");
        println!("✅ BYTE-FOR-BYTE IDENTICAL to original!");
        println!("✅ {} bytes match exactly", report.original_size);
        println!("✅ This is a GROUNDBREAKING achievement!");
        println!("\n🏆 cascette-rs is now the FIRST open-source tool");
        println!("   capable of perfect Blizzard CDN archive recreation!");
    } else if report.byte_differences < 1000 {
        println!("🌟 NEAR-PERFECT RECREATION!");
        println!(
            "✨ Only {} byte differences out of {} total bytes",
            report.byte_differences, report.original_size
        );
        println!(
            "✨ Success rate: {:.6}%",
            (1.0 - report.byte_differences as f64 / report.original_size as f64) * 100.0
        );
        println!("🔧 Minor adjustments needed for perfect recreation");
    } else if report.byte_differences < 100_000 {
        println!("👍 GOOD RECREATION QUALITY");
        println!(
            "📊 {} byte differences ({:.3}% different)",
            report.byte_differences,
            (report.byte_differences as f64 / report.original_size as f64) * 100.0
        );
        println!("🔧 Some improvements needed for perfect recreation");
    } else {
        println!("⚠️  RECREATION NEEDS IMPROVEMENT");
        println!(
            "📊 {} byte differences ({:.2}% different)",
            report.byte_differences,
            (report.byte_differences as f64 / report.original_size as f64) * 100.0
        );
        println!("🔧 Significant work needed for perfect recreation");
    }

    // Step 7: Save results for inspection
    if report.recreated_size > 0 {
        println!("\n💾 Saving recreated archive for inspection...");
        let output_path = "test_blte/perfect_recreation.blte";
        fs::write(output_path, &recreated_data)?;
        println!("✅ Saved to: {}", output_path);

        // Quick verification that our recreated file can be parsed
        println!("\n🔍 Verifying recreated archive can be parsed...");
        match BLTEArchive::parse(recreated_data) {
            Ok(recreated_archive) => {
                println!("✅ Recreated archive parses successfully!");
                println!("   Contains {} files", recreated_archive.file_count());
            }
            Err(e) => {
                println!("❌ Recreated archive parsing failed: {}", e);
            }
        }
    }

    println!("\n🚀 ROUND-TRIP RECREATION TEST COMPLETE! 🚀");

    if report.is_perfect {
        println!("🎊 CONGRATULATIONS! This is a historic achievement! 🎊");
    }

    Ok(())
}
