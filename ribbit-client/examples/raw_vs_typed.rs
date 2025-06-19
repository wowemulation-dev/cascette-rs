//! Example comparing raw API vs typed API usage
//!
//! This demonstrates the benefits of using typed responses
//! while showing that raw access is still available.

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US);

    println!("🔄 Raw API vs Typed API Comparison\n");

    // ========================================================================
    // RAW API - Manual parsing required
    // ========================================================================
    println!("📋 RAW API APPROACH:");
    println!("{:-<60}", "");

    // Switch to V2 for raw PSV data
    let raw_client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);
    let raw_response = raw_client
        .request_raw(&Endpoint::ProductVersions("wow".to_string()))
        .await?;

    let raw_string = String::from_utf8_lossy(&raw_response);

    // Manual parsing required
    let mut us_version = None;
    let mut us_build = None;

    for line in raw_string.lines() {
        if line.starts_with("us|") {
            let fields: Vec<&str> = line.split('|').collect();
            if fields.len() >= 6 {
                us_build = fields[4].parse::<u32>().ok();
                us_version = Some(fields[5].to_string());
            }
            break;
        }
    }

    println!("Manual parsing results:");
    println!("  US Version: {:?}", us_version);
    println!("  US Build: {:?}", us_build);
    println!("  ❌ Lots of boilerplate code");
    println!("  ❌ Error-prone string parsing");
    println!("  ❌ No type safety");

    // ========================================================================
    // TYPED API - Automatic parsing with type safety
    // ========================================================================
    println!("\n📋 TYPED API APPROACH:");
    println!("{:-<60}", "");

    let versions = client.get_product_versions("wow").await?;

    // Direct, type-safe access
    if let Some(us_entry) = versions.get_region("us") {
        println!("Type-safe access results:");
        println!("  US Version: {}", us_entry.versions_name);
        println!("  US Build: {}", us_entry.build_id);
        println!("  ✅ Clean, readable code");
        println!("  ✅ Compile-time type checking");
        println!("  ✅ IDE autocomplete support");
    }

    // ========================================================================
    // ADDITIONAL BENEFITS OF TYPED API
    // ========================================================================
    println!("\n🎯 TYPED API BENEFITS:");
    println!("{:-<60}", "");

    // 1. Convenience methods
    let builds = versions.build_ids();
    println!("1. Convenience methods:");
    println!("   Unique builds: {:?}", builds);

    // 2. All fields are accessible
    if let Some(entry) = versions.entries.first() {
        println!("\n2. All fields accessible:");
        println!("   Build Config: {}...", &entry.build_config[..8]);
        println!("   CDN Config: {}...", &entry.cdn_config[..8]);
        println!("   Product Config: {}...", &entry.product_config[..8]);
        println!(
            "   Key Ring: {:?}",
            entry.key_ring.as_ref().map(|k| &k[..8])
        );
    }

    // 3. Easy iteration
    println!("\n3. Easy iteration:");
    let region_count = versions.entries.len();
    let same_version_count = versions
        .entries
        .iter()
        .filter(|e| e.versions_name == versions.entries[0].versions_name)
        .count();
    println!(
        "   {} regions, {} with same version",
        region_count, same_version_count
    );

    // ========================================================================
    // BOTH APIS COEXIST
    // ========================================================================
    println!("\n🔀 BOTH APIS AVAILABLE:");
    println!("{:-<60}", "");

    // Get response with V2 protocol for BPSV parsing
    let v2_client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);
    let response = v2_client
        .request(&Endpoint::ProductVersions("wow".to_string()))
        .await?;

    // Access as text (like Ribbit.NET)
    if let Some(text) = response.as_text() {
        println!("1. Text access: {} bytes", text.len());
    }

    // Access as BPSV (works with V2 protocol)
    match response.as_bpsv() {
        Ok(bpsv) => println!("2. BPSV access: {} rows", bpsv.row_count()),
        Err(e) => println!("2. BPSV access: {}", e),
    }

    // Or use Display (ToString equivalent)
    let display = response.to_string();
    println!("3. Display impl: {} chars", display.len());

    println!("\n✅ Choose the right API for your use case!");

    Ok(())
}
