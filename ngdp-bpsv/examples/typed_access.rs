//! Typed access example
//!
//! This example demonstrates how to work with typed values in BPSV documents.

use ngdp_bpsv::{BpsvDocument, BpsvValue, Error};

fn main() -> Result<(), Error> {
    println!("=== Typed Access Example ===\n");

    // Example BPSV data with different field types
    let bpsv_data = r#"Region!STRING:0|BuildConfig!HEX:0|BuildId!DEC:4|Active!STRING:0
## seqn = 12345
us|be2bb98dc28aee05bbee519393696cdb|61491|true
eu|1234567890abcdef1234567890abcdef|61492|false
kr||61493|true
cn|deadbeefcafebabedeadbeefcafebabe|61494|"#;

    // Parse the document
    println!("1. Parsing BPSV document with mixed types...");
    let mut document = BpsvDocument::parse(bpsv_data)?;

    println!(
        "   ✅ Parsed {} rows with {} fields",
        document.row_count(),
        document.schema().field_count()
    );

    // Example 2: Working with typed values
    println!("\n2. Accessing typed values...");

    // Get schema first to avoid borrowing conflicts
    let schema = document.schema().clone();

    for (i, row) in document.rows_mut().iter_mut().enumerate() {
        println!("   Row {}:", i + 1);

        // Get typed values - this parses the raw strings according to field types
        let typed_values = row.get_typed_values(&schema)?;

        for (field, value) in schema.fields().iter().zip(typed_values.iter()) {
            println!(
                "     {} ({}) = {} [{}]",
                field.name,
                field.field_type,
                value.to_bpsv_string(),
                value.value_type()
            );
        }

        // Access specific fields by name
        if let Ok(Some(region_value)) = row.get_typed_by_name("Region", &schema) {
            if let Some(region_str) = region_value.as_string() {
                println!("     → Region as string: '{region_str}'");
            }
        }

        if let Ok(Some(build_id_value)) = row.get_typed_by_name("BuildId", &schema) {
            if let Some(build_id) = build_id_value.as_decimal() {
                println!("     → Build ID as number: {build_id}");
            }
        }

        if let Ok(Some(hex_value)) = row.get_typed_by_name("BuildConfig", &schema) {
            match hex_value {
                BpsvValue::Hex(hex_str) => {
                    println!(
                        "     → BuildConfig as hex: '{}' ({} chars)",
                        hex_str,
                        hex_str.len()
                    );
                }
                BpsvValue::Empty => {
                    println!("     → BuildConfig: <empty>");
                }
                _ => println!("     → BuildConfig: unexpected type"),
            }
        }

        println!();
    }

    // Example 3: Converting to typed maps
    println!("3. Converting rows to typed maps...");

    for (i, row) in document.rows_mut().iter_mut().enumerate() {
        let typed_map = row.to_typed_map(&schema)?;

        println!("   Row {} as typed map:", i + 1);
        for (field_name, value) in &typed_map {
            println!("     {field_name}: {value:?}");
        }

        // Type-safe access
        if let Some(BpsvValue::String(region)) = typed_map.get("Region") {
            println!("     → Type-safe region access: '{region}'");
        }

        if let Some(BpsvValue::Decimal(build_id)) = typed_map.get("BuildId") {
            println!("     → Type-safe build ID access: {build_id}");
        }

        println!();
    }

    // Example 4: Value type conversions
    println!("4. Demonstrating value conversions...");

    let string_value = BpsvValue::String("hello".to_string());
    let decimal_value = BpsvValue::Decimal(12345);
    let hex_value = BpsvValue::Hex("deadbeef".to_string());
    let empty_value = BpsvValue::Empty;

    println!("   String value: {string_value}");
    println!("   Decimal value: {decimal_value}");
    println!("   Hex value: {hex_value}");
    println!("   Empty value: '{empty_value}'");

    // Try to convert values to Rust types
    println!("\n   Conversion attempts:");

    // String to String
    match String::try_from(string_value.clone()) {
        Ok(s) => println!("     String → String: '{s}'"),
        Err(e) => println!("     String → String failed: {e}"),
    }

    // Decimal to i64
    match i64::try_from(decimal_value.clone()) {
        Ok(n) => println!("     Decimal → i64: {n}"),
        Err(e) => println!("     Decimal → i64 failed: {e}"),
    }

    // Try invalid conversion
    match i64::try_from(string_value.clone()) {
        Ok(n) => println!("     String → i64: {n} (unexpected!)"),
        Err(e) => println!("     String → i64 correctly failed: {e}"),
    }

    // Example 5: Working with different field types
    println!("\n5. Demonstrating field type validation...");

    use ngdp_bpsv::{BpsvBuilder, BpsvFieldType};

    let mut builder = BpsvBuilder::new();
    builder
        .add_field("StringField", BpsvFieldType::String(5))? // Max 5 chars
        .add_field("HexField", BpsvFieldType::Hex(8))? // Exactly 8 hex chars
        .add_field("DecField", BpsvFieldType::Decimal(4))?; // Decimal number

    // Valid values
    println!("   Testing valid values:");
    match builder.add_raw_row(&[
        "hello".to_string(),
        "deadbeef".to_string(),
        "1234".to_string(),
    ]) {
        Ok(_) => println!("     ✅ Valid row accepted"),
        Err(e) => println!("     ❌ Error: {e}"),
    }

    // Test string length validation
    println!("   Testing string length validation:");
    let mut builder2 = BpsvBuilder::new();
    builder2.add_field("ShortString", BpsvFieldType::String(3))?;

    match builder2.add_raw_row(&["ok".to_string()]) {
        Ok(_) => println!("     ✅ Short string accepted"),
        Err(e) => println!("     ❌ Error: {e}"),
    }

    match builder2.add_raw_row(&["toolong".to_string()]) {
        Ok(_) => println!("     ❌ Long string should have been rejected!"),
        Err(e) => println!("     ✅ Long string correctly rejected: {e}"),
    }

    // Test hex validation
    println!("   Testing hex validation:");
    let mut builder3 = BpsvBuilder::new();
    builder3.add_field("HexField", BpsvFieldType::Hex(4))?;

    match builder3.add_raw_row(&["abcd".to_string()]) {
        Ok(_) => println!("     ✅ Valid hex accepted"),
        Err(e) => println!("     ❌ Error: {e}"),
    }

    match builder3.add_raw_row(&["xyz".to_string()]) {
        Ok(_) => println!("     ❌ Invalid hex should have been rejected!"),
        Err(e) => println!("     ✅ Invalid hex correctly rejected: {e}"),
    }

    // Example 6: Empty values handling
    println!("\n6. Handling empty values...");

    let empty_data = r#"Field1!STRING:0|Field2!DEC:4|Field3!HEX:8
row1||deadbeef
|123|
value|456|abcd1234"#;

    let mut empty_doc = BpsvDocument::parse(empty_data)?;
    println!("   Parsed document with empty values:");

    let empty_schema = empty_doc.schema().clone();

    for (i, row) in empty_doc.rows_mut().iter_mut().enumerate() {
        let typed_values = row.get_typed_values(&empty_schema)?;
        println!("     Row {}: {:?}", i + 1, typed_values);

        // Check for empty values
        for (j, value) in typed_values.iter().enumerate() {
            if value.is_empty() {
                let field_name = &empty_schema.fields()[j].name;
                println!("       Field '{field_name}' is empty");
            }
        }
    }

    println!("\n✅ All typed access examples completed successfully!");

    Ok(())
}
