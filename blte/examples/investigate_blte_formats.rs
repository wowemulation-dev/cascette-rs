//! Investigate BLTE format variations across different Blizzard products

use std::fs;
use std::io::Write;
use std::path::Path;

#[derive(Debug)]
struct BLTEAnalysis {
    product: String,
    url: String,
    file_size: usize,
    magic: [u8; 4],
    header_size_raw: [u8; 4],
    header_size: u32,
    first_data_bytes_at_36: Vec<u8>,
    first_data_bytes_at_44: Vec<u8>,
    checksum_at_36: String,
    checksum_at_44: String,
    stored_checksum: String,
    chunk_count: u32,
    first_chunk_compressed_size: u32,
    data_offset_expected: usize,
    actual_data_offset: Option<usize>,
}

fn download_blte(url: &str, path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if path.exists() {
        println!("  Using cached file: {}", path.display());
        return Ok(fs::read(path)?);
    }

    println!("  Downloading: {}", url);
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(30))
        .call()?;

    let mut data = Vec::new();
    response.into_reader().read_to_end(&mut data)?;

    // Cache for future use
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, &data)?;

    Ok(data)
}

fn analyze_blte(product: &str, url: &str, data: &[u8]) -> BLTEAnalysis {
    let mut analysis = BLTEAnalysis {
        product: product.to_string(),
        url: url.to_string(),
        file_size: data.len(),
        magic: [0; 4],
        header_size_raw: [0; 4],
        header_size: 0,
        first_data_bytes_at_36: vec![],
        first_data_bytes_at_44: vec![],
        checksum_at_36: String::new(),
        checksum_at_44: String::new(),
        stored_checksum: String::new(),
        chunk_count: 0,
        first_chunk_compressed_size: 0,
        data_offset_expected: 0,
        actual_data_offset: None,
    };

    if data.len() < 8 {
        return analysis;
    }

    // Parse basic header
    analysis.magic.copy_from_slice(&data[0..4]);
    analysis.header_size_raw.copy_from_slice(&data[4..8]);
    analysis.header_size = u32::from_be_bytes(analysis.header_size_raw);

    // Get data at different potential offsets
    if data.len() > 36 + 10 {
        analysis.first_data_bytes_at_36 = data[36..46].to_vec();
    }
    if data.len() > 44 + 10 {
        analysis.first_data_bytes_at_44 = data[44..54].to_vec();
    }

    // Parse chunk info if multi-chunk
    if analysis.header_size > 0 && data.len() > 12 {
        // Parse chunk count
        let chunk_count_bytes = [data[9], data[10], data[11]];
        analysis.chunk_count = ((chunk_count_bytes[0] as u32) << 16)
            | ((chunk_count_bytes[1] as u32) << 8)
            | (chunk_count_bytes[2] as u32);

        // Parse first chunk info if available
        if data.len() >= 36 && analysis.chunk_count > 0 {
            analysis.first_chunk_compressed_size =
                u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

            // Get stored checksum
            if data.len() >= 36 {
                let checksum_bytes = &data[20..36];
                analysis.stored_checksum = hex::encode(checksum_bytes);
            }
        }

        // Calculate checksums at different offsets
        if analysis.first_chunk_compressed_size > 0 {
            let chunk_size = analysis.first_chunk_compressed_size as usize;

            // Try offset 36
            if data.len() >= 36 + chunk_size {
                let chunk_data = &data[36..36 + chunk_size];
                analysis.checksum_at_36 = format!("{:x}", md5::compute(chunk_data));
            }

            // Try offset 44 (8 + header_size)
            let offset_44 = 8 + analysis.header_size as usize;
            if data.len() >= offset_44 + chunk_size {
                let chunk_data = &data[offset_44..offset_44 + chunk_size];
                analysis.checksum_at_44 = format!("{:x}", md5::compute(chunk_data));
            }
        }

        analysis.data_offset_expected = 8 + analysis.header_size as usize;
    } else {
        analysis.data_offset_expected = 8;
    }

    // Determine actual data offset by checksum match
    if analysis.checksum_at_36 == analysis.stored_checksum {
        analysis.actual_data_offset = Some(36);
    } else if analysis.checksum_at_44 == analysis.stored_checksum {
        analysis.actual_data_offset = Some(44);
    }

    analysis
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== BLTE Format Investigation ===\n");

    // Test files from different products and scenarios
    let test_cases = vec![
        // WoW Classic Era - from CDN config we found earlier
        (
            "WoW Classic",
            "http://level3.blizzard.com/tpr/wow/data/00/17/0017a402f556fbece46c38dc431a2c9b",
            "wow_classic_archive.blte",
        ),
        // WoW Retail - current expansion data
        (
            "WoW Retail",
            "http://level3.blizzard.com/tpr/wow/data/3b/7f/3b7f0d3418656fd468ad9ae01571af6c",
            "wow_retail_data.blte",
        ),
        // Diablo IV
        (
            "Diablo IV",
            "http://level3.blizzard.com/tpr/fenris/data/68/83/6883ea7ced6cda7841c08c80b0a2649b",
            "d4_data.blte",
        ),
        // Overwatch 2
        (
            "Overwatch 2",
            "http://level3.blizzard.com/tpr/pro/data/7e/3b/7e3b67edb7b8b33a175c9dfa4d780c3a",
            "ow2_data.blte",
        ),
        // Hearthstone
        (
            "Hearthstone",
            "http://level3.blizzard.com/tpr/hsb/data/91/fa/91fa6cc033b9f5d848512d1c15d4ab3b",
            "hs_data.blte",
        ),
        // StarCraft II
        (
            "StarCraft II",
            "http://level3.blizzard.com/tpr/s2/data/27/47/2747e6b6bf90f655c0d2ee2de3f1dfc5",
            "sc2_data.blte",
        ),
        // Heroes of the Storm
        (
            "Heroes",
            "http://level3.blizzard.com/tpr/hero/data/6e/c8/6ec11f75a8dbf328b78c7f2c9a3ea398",
            "hots_data.blte",
        ),
        // Warcraft III Reforged
        (
            "WC3 Reforged",
            "http://level3.blizzard.com/tpr/w3/data/ad/21/ad213af3f8a84539f84304df16f6fdaa",
            "wc3r_data.blte",
        ),
    ];

    let mut analyses = Vec::new();
    let cache_dir = Path::new("test_blte/investigation");
    fs::create_dir_all(cache_dir)?;

    for (product, url, filename) in &test_cases {
        println!("Testing {}", product);
        let cache_path = cache_dir.join(filename);

        match download_blte(url, &cache_path) {
            Ok(data) => {
                // Only analyze first 100KB to avoid memory issues with huge files
                let analyze_size = data.len().min(100_000);
                let analysis_data = &data[..analyze_size];

                let analysis = analyze_blte(product, url, analysis_data);

                // Print immediate findings
                println!(
                    "  File size: {} bytes ({:.2} MB)",
                    analysis.file_size,
                    analysis.file_size as f64 / 1_048_576.0
                );
                println!(
                    "  Header size: {} (0x{:02x})",
                    analysis.header_size, analysis.header_size
                );
                println!("  Expected offset: {}", analysis.data_offset_expected);
                println!("  Actual offset: {:?}", analysis.actual_data_offset);

                if analysis.actual_data_offset.is_some() {
                    let actual = analysis.actual_data_offset.unwrap();
                    if actual != analysis.data_offset_expected {
                        println!(
                            "  ⚠ OFFSET MISMATCH: Expected {} but data at {}",
                            analysis.data_offset_expected, actual
                        );
                    } else {
                        println!("  ✓ Offset matches expectation");
                    }
                }

                analyses.push(analysis);
            }
            Err(e) => {
                println!("  Failed to download: {}", e);
            }
        }
        println!();
    }

    // Summary
    println!("\n=== SUMMARY ===\n");

    let mut offset_matches = 0;
    let mut offset_mismatches = 0;
    let mut unknowns = 0;

    for analysis in &analyses {
        if let Some(actual) = analysis.actual_data_offset {
            if actual == analysis.data_offset_expected {
                offset_matches += 1;
            } else {
                offset_mismatches += 1;
                println!(
                    "MISMATCH in {}: Expected {} but got {}",
                    analysis.product, analysis.data_offset_expected, actual
                );
                println!("  Header size: {}", analysis.header_size);
                println!("  Stored checksum: {}", analysis.stored_checksum);
                println!("  Checksum at 36: {}", analysis.checksum_at_36);
                println!("  Checksum at 44: {}", analysis.checksum_at_44);
                println!();
            }
        } else {
            unknowns += 1;
        }
    }

    println!("Results:");
    println!("  Offset matches: {}", offset_matches);
    println!("  Offset mismatches: {}", offset_mismatches);
    println!("  Unknown: {}", unknowns);

    // Test actual decompression with our library
    println!("\n=== Testing Decompression ===\n");

    for (product, _, filename) in &test_cases {
        let cache_path = cache_dir.join(filename);
        if !cache_path.exists() {
            continue;
        }

        print!("Testing {} decompression... ", product);
        std::io::stdout().flush()?;

        let data = fs::read(&cache_path)?;

        // For large files, just test parsing
        if data.len() > 10_000_000 {
            match blte::BLTEFile::parse(data.clone()) {
                Ok(blte) => {
                    println!("✓ Parsed ({} chunks)", blte.chunk_count());
                }
                Err(e) => {
                    println!("✗ Parse failed: {}", e);
                }
            }
        } else {
            // For smaller files, try full decompression
            match blte::decompress_blte(data.clone(), None) {
                Ok(decompressed) => {
                    println!("✓ Success ({} bytes)", decompressed.len());
                }
                Err(e) => {
                    println!("✗ Failed: {}", e);

                    // If it's a checksum error, try with offset adjustment
                    if e.to_string().contains("ChecksumMismatch") {
                        println!("  Attempting with offset adjustment...");
                        // We would need to implement a fix here
                    }
                }
            }
        }
    }

    Ok(())
}
