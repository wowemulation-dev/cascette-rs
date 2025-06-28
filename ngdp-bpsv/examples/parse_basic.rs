//! Basic BPSV parsing example
//!
//! This example demonstrates how to parse BPSV data from a string.

use ngdp_bpsv::{BpsvDocument, BpsvParser, Error};

fn main() -> Result<(), Error> {
    println!("=== Basic BPSV Parsing Example ===\n");

    // Example BPSV data similar to what Ribbit returns
    let bpsv_data = r#"Region!STRING:0|BuildConfig!HEX:32|CDNConfig!HEX:32|KeyRing!HEX:32|BuildId!DEC:4|VersionsName!String:0|ProductConfig!HEX:32
## seqn = 3016450
us|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
eu|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
cn|dcfc289eea032df214ebba097dc2880d|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61265|11.1.5.61265|53020d32e1a25648c8e1eafd5771935f
kr|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
tw|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
sg|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
xx|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f"#;

    // Parse the BPSV document
    println!("1. Parsing BPSV document...");
    let document = BpsvDocument::parse(bpsv_data)?;

    println!("   ✅ Successfully parsed!");
    println!("   📊 Schema: {} fields", document.schema().field_count());
    println!("   📈 Sequence number: {:?}", document.sequence_number());
    println!("   📋 Data rows: {}", document.row_count());

    // Display schema information
    println!("\n2. Schema Information:");
    for field in document.schema().fields() {
        println!("   • {} ({})", field.name, field.field_type);
    }

    // Display sequence number
    if let Some(seqn) = document.sequence_number() {
        println!("\n3. Sequence Number: {seqn}");
    }

    // Display first few rows
    println!("\n4. First 3 data rows:");
    for (i, row) in document.rows().iter().take(3).enumerate() {
        println!("   Row {}: {}", i + 1, row.to_bpsv_line());

        // Show structured access
        let row_map = row.to_map(document.schema())?;
        println!(
            "      Region: {}",
            row_map.get("Region").unwrap_or(&"N/A".to_string())
        );
        println!(
            "      Version: {}",
            row_map.get("VersionsName").unwrap_or(&"N/A".to_string())
        );
        println!(
            "      Build ID: {}",
            row_map.get("BuildId").unwrap_or(&"N/A".to_string())
        );
    }

    // Demonstrate column access
    println!("\n5. All regions:");
    let regions = document.get_column("Region")?;
    println!("   {regions:?}");

    // Demonstrate searching
    println!("\n6. Finding rows for region 'us':");
    let us_rows = document.find_rows_by_field("Region", "us")?;
    println!(
        "   Found {} row(s) at indices: {:?}",
        us_rows.len(),
        us_rows
    );

    // Display statistics using parser utility
    println!("\n7. Document statistics:");
    let (field_count, row_count, has_seqn) = BpsvParser::get_stats(bpsv_data)?;
    println!(
        "   Fields: {field_count}, Rows: {row_count}, Has sequence: {has_seqn}"
    );

    // Demonstrate round-trip conversion
    println!("\n8. Round-trip test:");
    let regenerated = document.to_bpsv_string();
    let reparsed = BpsvDocument::parse(&regenerated)?;

    println!("   Original rows: {}", document.row_count());
    println!("   Reparsed rows: {}", reparsed.row_count());
    println!(
        "   Round-trip successful: {}",
        document.row_count() == reparsed.row_count()
    );

    println!("\n✅ All examples completed successfully!");

    Ok(())
}
