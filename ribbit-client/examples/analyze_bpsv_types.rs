//! Analyze all BPSV field types across different endpoints

use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};
use std::collections::HashSet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);

    println!("=== Analyzing BPSV Field Types ===\n");

    let endpoints = [
        (
            "Product Versions (WoW)",
            Endpoint::ProductVersions("wow".to_string()),
        ),
        (
            "Product CDNs (WoW)",
            Endpoint::ProductCdns("wow".to_string()),
        ),
        (
            "Product BGDL (WoW)",
            Endpoint::ProductBgdl("wow".to_string()),
        ),
        ("Summary", Endpoint::Summary),
        (
            "Product Versions (Agent)",
            Endpoint::ProductVersions("agent".to_string()),
        ),
        (
            "Product CDNs (Agent)",
            Endpoint::ProductCdns("agent".to_string()),
        ),
    ];

    let mut all_field_types = HashSet::new();
    let mut field_examples = Vec::new();

    for (name, endpoint) in &endpoints {
        println!("📊 Analyzing: {name}");

        match client.request(endpoint).await {
            Ok(response) => {
                if let Some(data) = &response.data {
                    // Find the header line (contains !)
                    if let Some(header_line) = data.lines().find(|line| line.contains('!')) {
                        println!("   Header: {header_line}");

                        // Parse each field definition
                        let fields: Vec<&str> = header_line.split('|').collect();
                        println!("   Field Types:");

                        for field in fields {
                            if let Some(exclamation_pos) = field.find('!') {
                                let field_name = &field[..exclamation_pos];
                                let type_def = &field[exclamation_pos + 1..];

                                println!("     {field_name} -> {type_def}");
                                all_field_types.insert(type_def.to_string());
                                field_examples.push((field_name.to_string(), type_def.to_string()));
                            }
                        }
                    }

                    // Extract sequence number
                    if let Some(seqn_line) = data.lines().find(|line| line.starts_with("## seqn")) {
                        println!("   {seqn_line}");
                    }
                }
            }
            Err(e) => println!("   Error: {e}"),
        }
        println!();
    }

    println!("🎯 **Comprehensive Field Type Analysis**\n");

    println!("📋 **All Unique Field Types Found:**");
    let mut sorted_types: Vec<_> = all_field_types.iter().collect();
    sorted_types.sort();
    for field_type in sorted_types {
        println!("   • {field_type}");
    }

    println!("\n📋 **Field Type Patterns:**");

    // Group by base type
    let mut string_types = Vec::new();
    let mut hex_types = Vec::new();
    let mut dec_types = Vec::new();
    let mut other_types = Vec::new();

    for field_type in &all_field_types {
        if field_type.starts_with("STRING") {
            string_types.push(field_type);
        } else if field_type.starts_with("String") {
            string_types.push(field_type); // Alternative format
        } else if field_type.starts_with("HEX") {
            hex_types.push(field_type);
        } else if field_type.starts_with("DEC") {
            dec_types.push(field_type);
        } else {
            other_types.push(field_type);
        }
    }

    if !string_types.is_empty() {
        println!("   🔤 String Types: {string_types:?}");
    }
    if !hex_types.is_empty() {
        println!("   🔢 Hex Types: {hex_types:?}");
    }
    if !dec_types.is_empty() {
        println!("   🔢 Decimal Types: {dec_types:?}");
    }
    if !other_types.is_empty() {
        println!("   ❓ Other Types: {other_types:?}");
    }

    println!("\n🏗️ **Suggested Rust Type System:**");
    println!("```rust");
    println!("#[derive(Debug, Clone, PartialEq)]");
    println!("pub enum BpsvFieldType {{");
    println!("    String(u32),    // STRING:0, String:0");
    println!("    Hex(u32),       // HEX:16");
    println!("    Decimal(u32),   // DEC:4");
    println!("}}");
    println!("```");

    Ok(())
}
