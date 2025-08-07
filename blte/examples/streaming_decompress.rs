//! Example showing BLTE streaming decompression
//!
//! This example demonstrates how to use BLTEStream to decompress large BLTE files
//! without loading everything into memory at once.

use blte::{BLTEStream, CompressionMode, create_streaming_reader};
use std::io::{Read, Write};

fn create_sample_blte_data() -> Vec<u8> {
    // Create a simple BLTE file with one uncompressed chunk
    let mut blte_data = Vec::new();

    // BLTE magic
    blte_data.extend_from_slice(b"BLTE");

    // Header size (single chunk, so 0)
    blte_data.extend_from_slice(&0u32.to_be_bytes());

    // Compression mode (N = no compression)
    blte_data.push(CompressionMode::None.as_byte());

    // Sample data
    blte_data.extend_from_slice(
        b"This is a sample BLTE file content that will be streamed chunk by chunk!",
    );

    blte_data
}

fn create_multi_chunk_blte_data() -> Vec<u8> {
    use flate2::{Compression, write::ZlibEncoder};

    // Create two compressed chunks
    let chunk1_data = b"First chunk: ";
    let chunk2_data = b"Second chunk with more data to make compression worthwhile!";

    // Compress both chunks
    let mut encoder1 = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder1.write_all(chunk1_data).unwrap();
    let compressed1 = encoder1.finish().unwrap();

    let mut encoder2 = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder2.write_all(chunk2_data).unwrap();
    let compressed2 = encoder2.finish().unwrap();

    // Build chunk data with compression mode prefixes
    let mut chunk1_full = Vec::new();
    chunk1_full.push(CompressionMode::ZLib.as_byte());
    chunk1_full.extend_from_slice(&compressed1);

    let mut chunk2_full = Vec::new();
    chunk2_full.push(CompressionMode::ZLib.as_byte());
    chunk2_full.extend_from_slice(&compressed2);

    // Calculate header size
    let header_size = 8 + 1 + 3 + 2 * 24; // magic + header_size + flags + chunk_count + 2 * chunk_info

    // Build BLTE file
    let mut blte_data = Vec::new();

    // Header
    blte_data.extend_from_slice(b"BLTE");
    blte_data.extend_from_slice(&(header_size as u32).to_be_bytes());

    // Chunk table
    blte_data.push(0x0F); // Flags
    blte_data.extend_from_slice(&[0x00, 0x00, 0x02]); // 2 chunks

    // Chunk 1 info
    blte_data.extend_from_slice(&(chunk1_full.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&(chunk1_data.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

    // Chunk 2 info
    blte_data.extend_from_slice(&(chunk2_full.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&(chunk2_data.len() as u32).to_be_bytes());
    blte_data.extend_from_slice(&[0; 16]); // Zero checksum to skip verification

    // Chunk data
    blte_data.extend_from_slice(&chunk1_full);
    blte_data.extend_from_slice(&chunk2_full);

    blte_data
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("BLTE Streaming Decompression Example");
    println!("====================================");

    // Example 1: Simple single chunk streaming
    println!("\n1. Single Chunk Streaming:");
    let simple_data = create_sample_blte_data();
    println!("   Original BLTE data size: {} bytes", simple_data.len());

    let mut stream = BLTEStream::new(simple_data, None)?;
    println!("   Stream created with {} chunks", stream.chunk_count());

    let mut decompressed = String::new();
    stream.read_to_string(&mut decompressed)?;
    println!("   Decompressed content: \"{decompressed}\"");
    println!("   Decompressed size: {} bytes", decompressed.len());

    // Example 2: Multi-chunk streaming
    println!("\n2. Multi-Chunk Streaming:");
    let multi_data = create_multi_chunk_blte_data();
    println!("   Original BLTE data size: {} bytes", multi_data.len());

    let mut multi_stream = BLTEStream::new(multi_data, None)?;
    println!(
        "   Stream created with {} chunks",
        multi_stream.chunk_count()
    );

    let mut multi_decompressed = String::new();
    multi_stream.read_to_string(&mut multi_decompressed)?;
    println!("   Decompressed content: \"{multi_decompressed}\"");
    println!("   Decompressed size: {} bytes", multi_decompressed.len());

    // Example 3: Streaming with small buffer reads (simulates processing large files)
    println!("\n3. Small Buffer Streaming (simulating large file processing):");
    let buffer_data = create_multi_chunk_blte_data();
    let mut buffer_stream = BLTEStream::new(buffer_data, None)?;

    let mut buffer = [0u8; 8]; // Very small buffer to demonstrate streaming
    let mut total_bytes = 0;
    let mut chunks_read = 0;

    println!("   Reading in 8-byte chunks:");
    loop {
        let bytes_read = buffer_stream.read(&mut buffer)?;
        if bytes_read == 0 {
            break; // End of stream
        }

        chunks_read += 1;
        total_bytes += bytes_read;
        let chunk_str = String::from_utf8_lossy(&buffer[..bytes_read]);
        println!("     Chunk {chunks_read}: {bytes_read} bytes -> \"{chunk_str}\"");
    }

    println!("   Total bytes read: {total_bytes}");
    println!("   Total read operations: {chunks_read}");

    // Example 4: Using the convenience function
    println!("\n4. Using convenience function:");
    let convenience_data = create_sample_blte_data();
    let mut convenience_stream = create_streaming_reader(convenience_data, None)?;

    let mut convenience_result = String::new();
    convenience_stream.read_to_string(&mut convenience_result)?;
    println!("   Result: \"{convenience_result}\"");

    println!("\n✅ All streaming examples completed successfully!");
    println!("\nStreaming benefits:");
    println!("• Memory efficient for large BLTE files");
    println!("• Can process data as it becomes available");
    println!("• Supports all BLTE compression modes");
    println!("• Compatible with standard Read trait");

    Ok(())
}
