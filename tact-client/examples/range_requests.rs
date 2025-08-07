//! Example demonstrating HTTP range requests for partial file downloads
//!
//! This example shows how to use range requests to download only specific
//! byte ranges from CDN files, which can be useful for:
//! - Downloading file headers to parse metadata
//! - Implementing streaming or progressive downloads
//! - Reducing bandwidth when only part of a file is needed

use std::error::Error;
use tact_client::{HttpClient, ProtocolVersion, Region};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("TACT HTTP Range Request Example");
    println!("===============================");

    // Create HTTP client
    let client = HttpClient::new(Region::US, ProtocolVersion::V1)?;

    // Example CDN parameters (these would come from a real CDN configuration)
    let cdn_host = "blzddist1-a.akamaihd.net";
    let path = "tpr/wow/data";

    // This is a real content hash from WoW Classic Era - a small config file
    let test_hash = "a9dcee49ab3f952d69441eb3fd91c159"; // This is from install manifest

    println!("\n1. Testing Range Request Support");
    println!("================================");
    println!("CDN Host: {}", cdn_host);
    println!("Path: {}", path);
    println!("Hash: {}", test_hash);

    // First, let's try a small range request to test server support
    match client
        .download_file_range(cdn_host, path, test_hash, (0, Some(255)))
        .await
    {
        Ok(response) => {
            println!("\n✅ Range request successful!");
            let status = response.status();
            println!("Status: {}", status);
            println!("Content-Length: {:?}", response.content_length());

            // Check for range-related headers
            if let Some(content_range) = response.headers().get("content-range") {
                println!("Content-Range: {:?}", content_range);
            }
            if let Some(accept_ranges) = response.headers().get("accept-ranges") {
                println!("Accept-Ranges: {:?}", accept_ranges);
            }

            let partial_data = response.bytes().await?;
            println!(
                "Downloaded {} bytes (requested first 256 bytes)",
                partial_data.len()
            );

            if status == 206 {
                println!("🎉 Server supports HTTP range requests (206 Partial Content)");
            } else if status == 200 {
                println!("⚠️  Server returned full file (200 OK) - range requests not supported");
                println!("Full file size: {} bytes", partial_data.len());
            }
        }
        Err(e) => {
            eprintln!("❌ Range request failed: {}", e);
        }
    }

    println!("\n2. Comparing Full vs Partial Download");
    println!("====================================");

    // Download full file for comparison
    match client.download_file(cdn_host, path, test_hash).await {
        Ok(response) => {
            let full_data = response.bytes().await?;
            println!("Full file size: {} bytes", full_data.len());

            // Now download just the first part
            match client
                .download_file_range(cdn_host, path, test_hash, (0, Some(127)))
                .await
            {
                Ok(range_response) => {
                    let range_status = range_response.status();
                    let partial_data = range_response.bytes().await?;
                    println!(
                        "Partial download: {} bytes (requested first 128 bytes)",
                        partial_data.len()
                    );

                    // Verify the partial data matches the beginning of the full file
                    if partial_data.len() <= full_data.len() {
                        let matches = if range_status == 206 {
                            // Server supported range requests
                            partial_data == full_data[..partial_data.len()]
                        } else {
                            // Server returned full file despite range request
                            partial_data == full_data
                        };

                        if matches {
                            println!("✅ Partial data matches beginning of full file");
                        } else {
                            println!("❌ Partial data does not match full file!");
                        }
                    }
                }
                Err(e) => eprintln!("Failed to download partial content: {}", e),
            }
        }
        Err(e) => eprintln!("Failed to download full file: {}", e),
    }

    println!("\n3. Multiple Range Requests");
    println!("==========================");

    // Example of downloading different ranges
    let ranges = [
        (0, Some(63)),   // First 64 bytes
        (64, Some(127)), // Next 64 bytes
        (128, None),     // Rest of file from byte 128
    ];

    for (i, &range) in ranges.iter().enumerate() {
        match client
            .download_file_range(cdn_host, path, test_hash, range)
            .await
        {
            Ok(response) => {
                let data = response.bytes().await?;
                match range {
                    (start, Some(end)) => {
                        println!(
                            "Range {}: bytes {}-{} = {} bytes downloaded",
                            i + 1,
                            start,
                            end,
                            data.len()
                        );
                    }
                    (start, None) => {
                        println!(
                            "Range {}: bytes {}- (to end) = {} bytes downloaded",
                            i + 1,
                            start,
                            data.len()
                        );
                    }
                }
            }
            Err(e) => eprintln!("Range {} failed: {}", i + 1, e),
        }
    }

    println!("\n4. Multi-Range Request (Advanced)");
    println!("==================================");

    // Try a multi-range request (not all servers support this)
    let multi_ranges = [(0, Some(31)), (64, Some(95))]; // Two 32-byte ranges

    match client
        .download_file_multirange(cdn_host, path, test_hash, &multi_ranges)
        .await
    {
        Ok(response) => {
            println!("Multi-range request status: {}", response.status());

            if let Some(content_type) = response.headers().get("content-type") {
                println!("Content-Type: {:?}", content_type);

                if content_type
                    .to_str()
                    .unwrap_or("")
                    .starts_with("multipart/byteranges")
                {
                    println!("✅ Server supports multi-range requests!");
                    println!("⚠️  Response contains multipart data that needs special parsing");
                } else {
                    println!("🔍 Single content type - server may not support multi-range");
                }
            }

            let data = response.bytes().await?;
            println!("Multi-range response size: {} bytes", data.len());
        }
        Err(e) => eprintln!("Multi-range request failed: {}", e),
    }

    println!("\n📋 Range Request Use Cases:");
    println!("• Download file headers to check formats before full download");
    println!("• Implement pause/resume functionality");
    println!("• Stream large files by downloading in chunks");
    println!("• Reduce bandwidth when only specific parts are needed");
    println!("• Parallel downloading of different file sections");

    println!("\n⚠️  Important Notes:");
    println!("• Not all CDN servers support range requests");
    println!("• Some servers ignore range headers and return full content (200 OK)");
    println!("• Multi-range requests are less commonly supported");
    println!("• Always check response status code (206 = partial, 200 = full)");

    Ok(())
}
