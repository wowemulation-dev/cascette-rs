//! Integration tests for ngdp-bpsv
//!
//! These tests verify end-to-end functionality and edge cases

use ngdp_bpsv::{BpsvBuilder, BpsvDocument, BpsvFieldType, BpsvValue, Error};

/// Test data representing actual NGDP data formats
mod test_data {
    pub const RIBBIT_VERSIONS: &str = r#"Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16|KeyRing!HEX:16|BuildId!DEC:4|VersionsName!STRING:0|ProductConfig!HEX:16
## seqn = 3016450
us|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
eu|be2bb98dc28aee05bbee519393696cdb|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
cn|dcfc289eea032df214ebba097dc2880d|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61265|11.1.5.61265|53020d32e1a25648c8e1eafd5771935f
kr|6b8de4c2971c2cb3fce69e40c7c825d7|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
tw|6b8de4c2971c2cb3fce69e40c7c825d7|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
sg|6b8de4c2971c2cb3fce69e40c7c825d7|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f
xx|6b8de4c2971c2cb3fce69e40c7c825d7|fac77b9ca52c84ac28ad83a7dbe1c829|3ca57fe7319a297346440e4d2a03a0cd|61491|11.1.7.61491|53020d32e1a25648c8e1eafd5771935f"#;

    pub const CDN_CONFIG: &str = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
## seqn = 2241282
us|tpr/wow|us.cdn.blizzard.com level3.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://us.cdn.blizzard.com/?maxhosts=4 https://level3.ssl.blizzard.com/?maxhosts=4 https://us.cdn.blizzard.com/?maxhosts=4|tpr/configs/data
eu|tpr/wow|eu.cdn.blizzard.com level3.blizzard.com|http://eu.cdn.blizzard.com/?maxhosts=4 http://level3.blizzard.com/?maxhosts=4 https://eu.cdn.blizzard.com/?maxhosts=4 https://level3.ssl.blizzard.com/?maxhosts=4|tpr/configs/data
cn|tpr/wow|cn.cdn.blizzard.com client02.pdl.wow.battlenet.com.cn client04.pdl.wow.battlenet.com.cn client05.pdl.wow.battlenet.com.cn|http://cn.cdn.blizzard.com/?maxhosts=4 http://client02.pdl.wow.battlenet.com.cn/?maxhosts=4 http://client04.pdl.wow.battlenet.com.cn/?maxhosts=4 http://client05.pdl.wow.battlenet.com.cn/?maxhosts=4 https://cn.cdn.blizzard.com/?maxhosts=4 https://client02.pdl.wow.battlenet.com.cn/?maxhosts=4 https://client04.pdl.wow.battlenet.com.cn/?maxhosts=4 https://client05.pdl.wow.battlenet.com.cn/?maxhosts=4|tpr/configs/data
kr|tpr/wow|kr.cdn.blizzard.com level3.blizzard.com blizzard.nefficient.co.kr|http://blizzard.nefficient.co.kr/?maxhosts=4 http://kr.cdn.blizzard.com/?maxhosts=4 http://level3.blizzard.com/?maxhosts=4 https://blizzard.nefficient.co.kr/?maxhosts=4 https://kr.cdn.blizzard.com/?maxhosts=4 https://level3.ssl.blizzard.com/?maxhosts=4|tpr/configs/data
tw|tpr/wow|level3.blizzard.com us.cdn.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://us.cdn.blizzard.com/?maxhosts=4 https://level3.ssl.blizzard.com/?maxhosts=4 https://us.cdn.blizzard.com/?maxhosts=4|tpr/configs/data"#;

    pub const SUMMARY_DATA: &str = r#"Product!STRING:0|Seqn!DEC:7|Flags!STRING:0
## seqn = 3016579
agent|3011139|
agent_beta|1858435|cdn
anbs|2478338|cdn
anbsdev|2475394|cdn
auks|2953410|cdn"#;
}

#[test]
fn test_parse_real_ribbit_versions_data() {
    let doc = BpsvDocument::parse(test_data::RIBBIT_VERSIONS).unwrap();

    // Verify structure
    assert_eq!(doc.schema().fields().len(), 7);
    assert_eq!(doc.sequence_number(), Some(3016450));
    assert_eq!(doc.rows().len(), 7);

    // Verify specific fields
    let region_field = doc.schema().get_field("Region").unwrap();
    assert_eq!(region_field.field_type, BpsvFieldType::String(0));

    let build_id_field = doc.schema().get_field("BuildId").unwrap();
    assert_eq!(build_id_field.field_type, BpsvFieldType::Decimal(4));

    // Verify data content
    let first_row = &doc.rows()[0];
    assert_eq!(
        first_row.get_raw_by_name("Region", doc.schema()).unwrap(),
        "us"
    );
    assert_eq!(
        first_row.get_raw_by_name("BuildId", doc.schema()).unwrap(),
        "61491"
    );
    assert_eq!(
        first_row
            .get_raw_by_name("VersionsName", doc.schema())
            .unwrap(),
        "11.1.7.61491"
    );

    // Verify all regions
    let regions: Vec<&str> = doc
        .rows()
        .iter()
        .map(|row| row.get_raw_by_name("Region", doc.schema()).unwrap())
        .collect();
    assert_eq!(regions, vec!["us", "eu", "cn", "kr", "tw", "sg", "xx"]);
}

#[test]
fn test_parse_real_cdn_config_data() {
    let doc = BpsvDocument::parse(test_data::CDN_CONFIG).unwrap();

    // Verify structure
    assert_eq!(doc.schema().fields().len(), 5);
    assert_eq!(doc.sequence_number(), Some(2241282));
    assert_eq!(doc.rows().len(), 5);

    // Verify complex string data with spaces
    let first_row = &doc.rows()[0];
    assert_eq!(
        first_row.get_raw_by_name("Name", doc.schema()).unwrap(),
        "us"
    );
    assert_eq!(
        first_row.get_raw_by_name("Path", doc.schema()).unwrap(),
        "tpr/wow"
    );

    let hosts = first_row.get_raw_by_name("Hosts", doc.schema()).unwrap();
    assert!(hosts.contains("us.cdn.blizzard.com"));
    assert!(hosts.contains("level3.blizzard.com"));

    // Verify servers with URLs
    let servers = first_row.get_raw_by_name("Servers", doc.schema()).unwrap();
    assert!(servers.contains("http://"));
    assert!(servers.contains("https://"));
    assert!(servers.contains("?maxhosts=4"));
}

#[test]
fn test_parse_summary_with_empty_fields() {
    let doc = BpsvDocument::parse(test_data::SUMMARY_DATA).unwrap();

    // Verify structure
    assert_eq!(doc.schema().fields().len(), 3);
    assert_eq!(doc.sequence_number(), Some(3016579));
    assert_eq!(doc.rows().len(), 5);

    // Verify empty flags field
    let agent_row = &doc.rows()[0];
    assert_eq!(
        agent_row.get_raw_by_name("Product", doc.schema()).unwrap(),
        "agent"
    );
    assert_eq!(
        agent_row.get_raw_by_name("Seqn", doc.schema()).unwrap(),
        "3011139"
    );
    assert_eq!(
        agent_row.get_raw_by_name("Flags", doc.schema()).unwrap(),
        ""
    );

    // Verify non-empty flags
    let agent_beta_row = &doc.rows()[1];
    assert_eq!(
        agent_beta_row
            .get_raw_by_name("Flags", doc.schema())
            .unwrap(),
        "cdn"
    );
}

#[test]
fn test_build_and_parse_round_trip() {
    let mut builder = BpsvBuilder::new();
    builder
        .add_field("Region", BpsvFieldType::String(0))
        .unwrap();
    builder
        .add_field("BuildConfig", BpsvFieldType::Hex(16))
        .unwrap();
    builder
        .add_field("BuildId", BpsvFieldType::Decimal(10))
        .unwrap();
    builder.set_sequence_number(12345);

    builder
        .add_row(vec![
            BpsvValue::String("us".to_string()),
            BpsvValue::Hex("deadbeefcafebabedeadbeefcafebabe".to_string()),
            BpsvValue::Decimal(999999999),
        ])
        .unwrap();

    builder
        .add_row(vec![
            BpsvValue::String("eu".to_string()),
            BpsvValue::Empty, // Empty hex value
            BpsvValue::Decimal(0),
        ])
        .unwrap();

    builder
        .add_row(vec![
            BpsvValue::Empty, // Empty region
            BpsvValue::Hex("1234567890abcdef1234567890abcdef".to_string()),
            BpsvValue::Empty, // Empty decimal
        ])
        .unwrap();

    let document = builder.build().unwrap();
    let output = document.to_bpsv_string();

    let parsed = BpsvDocument::parse(&output).unwrap();

    // Verify everything matches
    assert_eq!(parsed.sequence_number(), Some(12345));
    assert_eq!(parsed.rows().len(), 3);

    let row1 = &parsed.rows()[0];
    assert_eq!(
        row1.get_raw_by_name("Region", parsed.schema()).unwrap(),
        "us"
    );
    assert_eq!(
        row1.get_raw_by_name("BuildId", parsed.schema()).unwrap(),
        "999999999"
    );

    let row2 = &parsed.rows()[1];
    assert_eq!(
        row2.get_raw_by_name("BuildConfig", parsed.schema())
            .unwrap(),
        ""
    );

    let row3 = &parsed.rows()[2];
    assert_eq!(row3.get_raw_by_name("Region", parsed.schema()).unwrap(), "");
    assert_eq!(
        row3.get_raw_by_name("BuildId", parsed.schema()).unwrap(),
        ""
    );
}

#[test]
fn test_case_insensitive_field_types() {
    // Test various case combinations
    let test_cases = vec![
        "Field1!string:0|Field2!HEX:16|Field3!dec:4",
        "Field1!String:0|Field2!hex:16|Field3!DEC:4",
        "Field1!STRING:0|Field2!Hex:16|Field3!Decimal:4",
        "Field1!StRiNg:0|Field2!hEx:16|Field3!DeC:4",
    ];

    for header in test_cases {
        let data = format!("{header}\n## seqn = 100\nvalue|abcd1234abcd1234abcd1234abcd1234|42");
        let doc = BpsvDocument::parse(&data).unwrap();

        assert_eq!(doc.schema().fields().len(), 3);
        assert_eq!(doc.rows().len(), 1);

        let row = &doc.rows()[0];
        assert_eq!(
            row.get_raw_by_name("Field1", doc.schema()).unwrap(),
            "value"
        );
        assert_eq!(
            row.get_raw_by_name("Field2", doc.schema()).unwrap(),
            "abcd1234abcd1234abcd1234abcd1234"
        );
        assert_eq!(row.get_raw_by_name("Field3", doc.schema()).unwrap(), "42");
    }
}

#[test]
fn test_error_handling() {
    // Test invalid header
    let result = BpsvDocument::parse("InvalidHeader");
    assert!(matches!(result, Err(Error::InvalidHeader { .. })));

    // Test mismatched columns
    let result = BpsvDocument::parse("Field1!STRING:0|Field2!DEC:4\nvalue1|value2|extra");
    assert!(matches!(result, Err(Error::SchemaMismatch { .. })));

    // Test invalid field type
    let result = BpsvDocument::parse("Field1!INVALID:0");
    assert!(matches!(result, Err(Error::InvalidFieldType { .. })));

    // Test invalid sequence number
    let result = BpsvDocument::parse("Field1!STRING:0\n## seqn = not_a_number");
    assert!(matches!(result, Err(Error::InvalidSequenceNumber { .. })));
}

#[test]
fn test_builder_validation() {
    let mut builder = BpsvBuilder::new();
    builder
        .add_field("Field1", BpsvFieldType::String(5))
        .unwrap();
    builder.add_field("Field2", BpsvFieldType::Hex(8)).unwrap();
    builder
        .add_field("Field3", BpsvFieldType::Decimal(4))
        .unwrap();

    // Test string length validation
    let result = builder.add_row(vec![
        BpsvValue::String("toolong".to_string()), // Exceeds length 5
        BpsvValue::Hex("deadbeef".to_string()),
        BpsvValue::Decimal(1234),
    ]);
    assert!(matches!(result, Err(Error::InvalidValue { .. })));

    // Reset and test hex validation
    builder.clear_rows();
    let result = builder.add_row(vec![
        BpsvValue::String("ok".to_string()),
        BpsvValue::Hex("invalid!".to_string()), // Invalid hex
        BpsvValue::Decimal(1234),
    ]);
    assert!(matches!(result, Err(Error::InvalidValue { .. })));

    // Reset and test hex length validation
    builder.clear_rows();
    let result = builder.add_row(vec![
        BpsvValue::String("ok".to_string()),
        BpsvValue::Hex("deadbeefcafe".to_string()), // Too long (12 chars > 8)
        BpsvValue::Decimal(1234),
    ]);
    assert!(matches!(result, Err(Error::InvalidValue { .. })));
}

#[test]
fn test_large_documents() {
    let mut builder = BpsvBuilder::new();

    for i in 0..20 {
        match i % 3 {
            0 => builder
                .add_field(&format!("StringField{i}"), BpsvFieldType::String(0))
                .unwrap(),
            1 => builder
                .add_field(&format!("DecField{i}"), BpsvFieldType::Decimal(10))
                .unwrap(),
            _ => builder
                .add_field(&format!("HexField{i}"), BpsvFieldType::Hex(16))
                .unwrap(),
        };
    }

    builder.set_sequence_number(999999);

    for row_idx in 0..1000 {
        let mut values = Vec::new();
        for col_idx in 0..20 {
            match col_idx % 3 {
                0 => values.push(BpsvValue::String(format!("row{row_idx}col{col_idx}"))),
                1 => values.push(BpsvValue::Decimal(row_idx as i64 * 100 + col_idx as i64)),
                _ => values.push(BpsvValue::Hex(format!(
                    "{:016x}{:016x}",
                    row_idx * 1000 + col_idx,
                    row_idx * 1000 + col_idx
                ))),
            }
        }
        builder.add_row(values).unwrap();
    }

    let document = builder.build().unwrap();
    let output = document.to_bpsv_string();

    let doc = BpsvDocument::parse(&output).unwrap();
    assert_eq!(doc.schema().fields().len(), 20);
    assert_eq!(doc.rows().len(), 1000);
    assert_eq!(doc.sequence_number(), Some(999999));

    // Spot check some values
    let row_500 = &doc.rows()[500];
    assert_eq!(
        row_500
            .get_raw_by_name("StringField0", doc.schema())
            .unwrap(),
        "row500col0"
    );
    assert_eq!(
        row_500.get_raw_by_name("DecField1", doc.schema()).unwrap(),
        "50001"
    );
}

#[test]
fn test_special_characters_in_strings() {
    let mut builder = BpsvBuilder::new();
    builder.add_field("Text", BpsvFieldType::String(0)).unwrap();
    builder
        .add_field("Code", BpsvFieldType::Decimal(4))
        .unwrap();

    // Test strings with special characters (but not pipes)
    let special_strings = [
        "Hello, World!",
        "Line with spaces and tabs\t\there",
        "Special chars: @#$%^&*()_+-={}[]",
        "Unicode: ñáéíóú",
        "Quotes: \"double\" and 'single'",
        "", // Empty string
    ];

    for (i, text) in special_strings.iter().enumerate() {
        builder
            .add_row(vec![
                BpsvValue::String(text.to_string()),
                BpsvValue::Decimal(i as i64),
            ])
            .unwrap();
    }

    let document = builder.build().unwrap();
    let output = document.to_bpsv_string();
    let doc = BpsvDocument::parse(&output).unwrap();

    // Verify all strings parsed correctly
    for (i, expected) in special_strings.iter().enumerate() {
        let actual = doc.rows()[i].get_raw_by_name("Text", doc.schema()).unwrap();
        assert_eq!(actual, *expected);
    }
}

#[test]
fn test_edge_case_numbers() {
    let mut builder = BpsvBuilder::new();
    builder
        .add_field("SmallDec", BpsvFieldType::Decimal(3))
        .unwrap();
    builder
        .add_field("LargeDec", BpsvFieldType::Decimal(20))
        .unwrap();

    // Test various decimal values
    let test_values = vec![
        (0i64, 0i64),
        (999, 9223372036854775807),   // Max i64
        (-999, -9223372036854775808), // Min i64
        (1, 1),
        (-1, -1),
    ];

    for (small, large) in test_values {
        builder
            .add_row(vec![BpsvValue::Decimal(small), BpsvValue::Decimal(large)])
            .unwrap();
    }

    let document = builder.build().unwrap();
    let output = document.to_bpsv_string();
    let doc = BpsvDocument::parse(&output).unwrap();

    // Verify values
    assert_eq!(
        doc.rows()[1]
            .get_raw_by_name("LargeDec", doc.schema())
            .unwrap(),
        "9223372036854775807"
    );
    assert_eq!(
        doc.rows()[2]
            .get_raw_by_name("LargeDec", doc.schema())
            .unwrap(),
        "-9223372036854775808"
    );
}

#[test]
fn test_from_existing_document() {
    let original = BpsvDocument::parse(test_data::SUMMARY_DATA).unwrap();

    let mut builder = BpsvBuilder::from_bpsv(&original.to_bpsv_string()).unwrap();

    builder
        .add_row(vec![
            BpsvValue::String("newproduct".to_string()),
            BpsvValue::Decimal(9999999),
            BpsvValue::String("newcdn".to_string()),
        ])
        .unwrap();

    let document = builder.build().unwrap();
    let output = document.to_bpsv_string();
    let modified = BpsvDocument::parse(&output).unwrap();

    // Verify original data is preserved
    assert_eq!(modified.sequence_number(), Some(3016579));
    assert_eq!(modified.rows().len(), 6); // Original 5 + 1 new

    let new_row = modified.rows().last().unwrap();
    assert_eq!(
        new_row
            .get_raw_by_name("Product", modified.schema())
            .unwrap(),
        "newproduct"
    );
    assert_eq!(
        new_row.get_raw_by_name("Seqn", modified.schema()).unwrap(),
        "9999999"
    );
}

#[test]
fn test_malformed_data_handling() {
    // Test various malformed inputs
    let test_cases = vec![
        ("", "Empty document"),
        ("   \n\n   ", "Only whitespace"),
        ("Field1!STRING:0|", "Trailing pipe in header"),
        ("|Field1!STRING:0", "Leading pipe in header"),
        ("Field1!STRING:0\n|value", "Leading pipe in data"),
        ("Field1!STRING:0\nvalue|", "Trailing pipe in data"),
        ("Field1!STRING:0||Field2!DEC:4", "Double pipe in header"),
        ("Field1!STRING:0\nvalue1||value2", "Double pipe in data"),
    ];

    for (input, description) in test_cases {
        let result = BpsvDocument::parse(input);
        assert!(result.is_err(), "Should fail for: {description}");
    }
}
