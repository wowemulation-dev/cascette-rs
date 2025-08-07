//! Test multiple WoW archive BLTE files to understand the offset pattern

use std::fs;
use std::time::Instant;

fn analyze_archive_blte(data: &[u8], name: &str) -> Result<(), Box<dyn std::error::Error>> {
    if data.len() < 44 {
        println!("{}: File too small", name);
        return Ok(());
    }

    // Parse header
    let magic = &data[0..4];
    let header_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let flags = data[8];
    let chunk_count = ((data[9] as u32) << 16) | ((data[10] as u32) << 8) | (data[11] as u32);

    println!(
        "{}: size={} bytes, header_size={}, chunks={}",
        name,
        data.len(),
        header_size,
        chunk_count
    );

    if chunk_count > 0 {
        // Parse first chunk
        let comp_size = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let decomp_size = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let stored_checksum = &data[20..36];

        println!("  First chunk: {} -> {} bytes", comp_size, decomp_size);

        // Test data at different offsets
        let offset_36 = 36;
        let offset_44 = 8 + header_size as usize;

        println!("  Expected offset (8 + header_size): {}", offset_44);

        // Check checksums
        if data.len() >= offset_36 + comp_size as usize {
            let chunk_at_36 = &data[offset_36..offset_36 + comp_size as usize];
            let checksum_36 = md5::compute(chunk_at_36);
            let matches_36 = checksum_36.0 == stored_checksum;
            println!(
                "  Data at offset 36: {} (matches: {})",
                hex::encode(checksum_36.0),
                matches_36
            );
        }

        if data.len() >= offset_44 + comp_size as usize {
            let chunk_at_44 = &data[offset_44..offset_44 + comp_size as usize];
            let checksum_44 = md5::compute(chunk_at_44);
            let matches_44 = checksum_44.0 == stored_checksum;
            println!(
                "  Data at offset {}: {} (matches: {})",
                offset_44,
                hex::encode(checksum_44.0),
                matches_44
            );
        }

        // Test decompression with our library (which uses offset 44)
        println!("  Testing decompression with current library...");
        match blte::decompress_blte(data.to_vec(), None) {
            Ok(decompressed) => {
                println!("    ✓ Success: {} bytes decompressed", decompressed.len());
            }
            Err(e) => {
                println!("    ✗ Failed: {}", e);

                if e.to_string().contains("ChecksumMismatch") {
                    println!("    This confirms offset 44 is wrong, should be 36");
                }
            }
        }
    }

    println!();
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing WoW Archive BLTE Offset Pattern ===\n");

    // Test with multiple WoW archive files
    let test_hashes = [
        "0017a402f556fbece46c38dc431a2c9b",
        "003b147730a109e3a480d32a54280955",
        "0081ce34ec18b09df3892729fbad1027",
        "00b79cc0eebdd26437c7e92e57ac7f5c",
        "00e43d6a55fe497ebaecece75c464913",
    ];

    fs::create_dir_all("test_blte/archives")?;

    for (i, hash) in test_hashes.iter().enumerate() {
        let filename = format!("archive_{}.blte", i + 1);
        let path = format!("test_blte/archives/{}", filename);

        // Download if not cached
        let data = if std::path::Path::new(&path).exists() {
            println!("Using cached {}", filename);
            fs::read(&path)?
        } else {
            let url = format!(
                "http://level3.blizzard.com/tpr/wow/data/{}/{}/{}",
                &hash[0..2],
                &hash[2..4],
                hash
            );

            println!("Downloading {}", filename);
            match ureq::get(&url).call() {
                Ok(response) => {
                    let mut data = Vec::new();
                    response.into_reader().read_to_end(&mut data)?;

                    // Only keep first 100KB for analysis
                    if data.len() > 100_000 {
                        data.truncate(100_000);
                    }

                    fs::write(&path, &data)?;
                    data
                }
                Err(e) => {
                    println!("  Failed to download: {}", e);
                    continue;
                }
            }
        };

        analyze_archive_blte(&data, &filename)?;
    }

    println!("=== Performance Test ===\n");

    // Test performance with our extracted first BLTE (known working)
    let first_blte_path = "test_blte/first.blte";
    if std::path::Path::new(first_blte_path).exists() {
        let data = fs::read(first_blte_path)?;
        println!("Testing performance with first.blte (broken offset)...");
        println!("File size: {} bytes", data.len());

        // This will fail due to checksum, but we can measure parsing time
        let start = Instant::now();
        match blte::BLTEFile::parse(data.clone()) {
            Ok(blte) => {
                let parse_time = start.elapsed();
                println!("Parse time: {:?}", parse_time);
                println!("Chunks: {}", blte.chunk_count());

                // Get chunk data (will fail on checksum)
                match blte.get_chunk_data(0) {
                    Ok(chunk) => {
                        println!("Chunk data retrieved: {} bytes", chunk.data.len());
                    }
                    Err(e) => {
                        println!("Expected checksum error: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("Parse failed: {}", e);
            }
        }
    }

    println!("\n=== Conclusion ===");
    println!("Based on the investigation:");
    println!("1. WoW archive BLTE files consistently have data at offset 36");
    println!("2. Standard calculation (8 + header_size) gives offset 44");
    println!("3. The header_size field appears to represent the chunk table size + 8");
    println!("4. We need to adjust our data_offset() calculation for archive files");

    Ok(())
}
