//! BPSV building example
//!
//! This example demonstrates how to create BPSV documents programmatically.

use ngdp_bpsv::{BpsvBuilder, BpsvFieldType, BpsvValue, Error};

fn main() -> Result<(), Error> {
    println!("=== BPSV Building Example ===\n");

    // Example 1: Basic document creation
    println!("1. Creating a basic BPSV document...");

    let mut builder = BpsvBuilder::new();

    // Define schema
    builder
        .add_field("Region", BpsvFieldType::String(0))?
        .add_field("BuildConfig", BpsvFieldType::Hex(16))?
        .add_field("BuildId", BpsvFieldType::Decimal(4))?
        .set_sequence_number(12345);

    // Add data using typed values
    builder.add_row(vec![
        BpsvValue::String("us".to_string()),
        BpsvValue::Hex("abcd1234abcd1234".to_string()),
        BpsvValue::Decimal(61491),
    ])?;

    builder.add_row(vec![
        BpsvValue::String("eu".to_string()),
        BpsvValue::Hex("1234abcd1234abcd".to_string()),
        BpsvValue::Decimal(61491),
    ])?;

    let document1 = builder.build()?;
    println!("   ✅ Created document with {} rows", document1.row_count());

    println!("\n   Generated BPSV:");
    println!("{}", document1.to_bpsv_string());

    // Example 2: Using raw string values
    println!("\n2. Creating document from raw strings...");

    let mut builder2 = BpsvBuilder::new();
    builder2
        .add_field("Product", BpsvFieldType::String(0))?
        .add_field("Seqn", BpsvFieldType::Decimal(4))?
        .add_field("Flags", BpsvFieldType::String(0))?;

    // Add data using raw string values (will be parsed)
    builder2.add_raw_row(&[
        "wow".to_string(),
        "3016450".to_string(),
        "cdn".to_string(),
    ])?;
    builder2.add_raw_row(&[
        "agent".to_string(),
        "3011139".to_string(),
        "".to_string(),
    ])?;
    builder2.add_raw_row(&[
        "d3".to_string(),
        "2985234".to_string(),
        "cdn".to_string(),
    ])?;

    let document2 = builder2.build()?;
    println!("   ✅ Created document with {} rows", document2.row_count());

    println!("\n   Generated BPSV:");
    println!("{}", document2.to_bpsv_string());

    // Example 3: Using the convenience values method
    println!("\n3. Creating document with mixed value types...");

    let mut builder3 = BpsvBuilder::new();
    builder3
        .add_field("Name", BpsvFieldType::String(0))?
        .add_field("Port", BpsvFieldType::Decimal(4))?
        .add_field("Hash", BpsvFieldType::Hex(8))?
        .set_sequence_number(999);

    // Can use different Rust types that convert to BpsvValue
    builder3.add_row(vec![
        BpsvValue::String("server1".to_string()),
        BpsvValue::Decimal(1119),
        BpsvValue::Hex("deadbeef".to_string()),
    ])?;
    builder3.add_row(vec![
        BpsvValue::String("server2".to_string()),
        BpsvValue::Decimal(1120),
        BpsvValue::Hex("cafebabe".to_string()),
    ])?;

    let document3 = builder3.build()?;
    println!("   ✅ Created document with {} rows", document3.row_count());

    println!("\n   Generated BPSV:");
    println!("{}", document3.to_bpsv_string());

    // Example 4: Building from existing BPSV
    println!("\n4. Modifying existing BPSV data...");

    let existing_bpsv = r#"Region!STRING:0|BuildId!DEC:4
us|1234
eu|5678"#;

    let mut builder4 = BpsvBuilder::from_bpsv(existing_bpsv)?;

    // Add more rows to the existing data
    builder4.add_raw_row(&["kr".to_string(), "9999".to_string()])?;
    builder4.add_raw_row(&["cn".to_string(), "8888".to_string()])?;
    builder4.set_sequence_number(42);

    let document4 = builder4.build()?;
    println!(
        "   ✅ Extended document now has {} rows",
        document4.row_count()
    );

    println!("\n   Extended BPSV:");
    println!("{}", document4.to_bpsv_string());

    // Example 5: CDN configuration example (realistic use case)
    println!("\n5. Creating CDN configuration document...");

    let mut cdn_builder = BpsvBuilder::new();
    cdn_builder
        .add_field("Name", BpsvFieldType::String(0))?
        .add_field("Path", BpsvFieldType::String(0))?
        .add_field("Hosts", BpsvFieldType::String(0))?
        .add_field("ConfigPath", BpsvFieldType::String(0))?
        .set_sequence_number(2241282);

    cdn_builder.add_raw_row(&[
        "us".to_string(),
        "tpr/wow".to_string(),
        "us.cdn.blizzard.com level3.blizzard.com".to_string(),
        "tpr/configs/data".to_string(),
    ])?;

    cdn_builder.add_raw_row(&[
        "eu".to_string(),
        "tpr/wow".to_string(),
        "eu.cdn.blizzard.com level3.blizzard.com".to_string(),
        "tpr/configs/data".to_string(),
    ])?;

    let cdn_document = cdn_builder.build()?;
    println!(
        "   ✅ Created CDN config with {} rows",
        cdn_document.row_count()
    );

    println!("\n   CDN Configuration BPSV:");
    println!("{}", cdn_document.to_bpsv_string());

    // Example 6: Validation and error handling
    println!("\n6. Demonstrating validation...");

    let mut error_builder = BpsvBuilder::new();
    error_builder.add_field("TestField", BpsvFieldType::Hex(4))?;

    // This should succeed
    match error_builder.add_raw_row(&["abcd".to_string()]) {
        Ok(_) => println!("   ✅ Valid hex value accepted"),
        Err(e) => println!("   ❌ Unexpected error: {}", e),
    }

    // This should fail - invalid hex
    match error_builder.add_raw_row(&["xyz".to_string()]) {
        Ok(_) => println!("   ❌ Invalid hex should have been rejected!"),
        Err(e) => println!("   ✅ Invalid hex correctly rejected: {}", e),
    }

    println!("\n✅ All building examples completed successfully!");

    Ok(())
}
