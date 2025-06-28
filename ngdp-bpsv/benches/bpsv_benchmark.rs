//! Benchmarks for BPSV parsing and building performance

use criterion::{Criterion, criterion_group, criterion_main};
use ngdp_bpsv::{BpsvBuilder, BpsvDocument, BpsvFieldType, BpsvValue};
use std::hint::black_box;

/// Generate a simple BPSV document string for benchmarking
fn generate_simple_bpsv(rows: usize) -> String {
    let mut lines = vec!["Region!STRING:0|BuildId!DEC:4|Hash!HEX:32".to_string()];
    lines.push("## seqn = 12345".to_string());

    for i in 0..rows {
        lines.push(format!(
            "us|{}|deadbeefcafebabedeadbeefcafebabedeadbeefcafebabedeadbeefcafebabe",
            1000 + i
        ));
    }

    lines.join("\n")
}

/// Generate a complex BPSV document with many columns
fn generate_complex_bpsv(rows: usize, cols: usize) -> String {
    let mut header_parts = Vec::new();
    for i in 0..cols {
        match i % 3 {
            0 => header_parts.push(format!("Field{i}!STRING:0")),
            1 => header_parts.push(format!("Field{i}!DEC:4")),
            _ => header_parts.push(format!("Field{i}!HEX:16")),
        }
    }

    let mut lines = vec![header_parts.join("|")];
    lines.push("## seqn = 99999".to_string());

    for row in 0..rows {
        let mut row_parts = Vec::new();
        for col in 0..cols {
            match col % 3 {
                0 => row_parts.push(format!("value{row}")),
                1 => row_parts.push(format!("{}", row * 100 + col)),
                _ => row_parts.push("1234567890abcdef1234567890abcdef".to_string()),
            }
        }
        lines.push(row_parts.join("|"));
    }

    lines.join("\n")
}

fn benchmark_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("parsing");

    // Small document (10 rows)
    let small_doc = generate_simple_bpsv(10);
    group.bench_function("parse_small_10_rows", |b| {
        b.iter(|| {
            let doc = BpsvDocument::parse(black_box(&small_doc)).unwrap();
            black_box(doc);
        });
    });

    // Medium document (100 rows)
    let medium_doc = generate_simple_bpsv(100);
    group.bench_function("parse_medium_100_rows", |b| {
        b.iter(|| {
            let doc = BpsvDocument::parse(black_box(&medium_doc)).unwrap();
            black_box(doc);
        });
    });

    // Large document (1000 rows)
    let large_doc = generate_simple_bpsv(1000);
    group.bench_function("parse_large_1000_rows", |b| {
        b.iter(|| {
            let doc = BpsvDocument::parse(black_box(&large_doc)).unwrap();
            black_box(doc);
        });
    });

    // Complex document (100 rows, 20 columns)
    let complex_doc = generate_complex_bpsv(100, 20);
    group.bench_function("parse_complex_100x20", |b| {
        b.iter(|| {
            let doc = BpsvDocument::parse(black_box(&complex_doc)).unwrap();
            black_box(doc);
        });
    });

    group.finish();
}

fn benchmark_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("building");

    // Building small documents
    group.bench_function("build_small_10_rows", |b| {
        b.iter(|| {
            let mut builder = BpsvBuilder::new();
            builder
                .add_field("Region", BpsvFieldType::String(0))
                .unwrap();
            builder
                .add_field("BuildId", BpsvFieldType::Decimal(4))
                .unwrap();
            builder.add_field("Hash", BpsvFieldType::Hex(32)).unwrap();
            builder.set_sequence_number(12345);

            for i in 0..10 {
                builder
                    .add_row(vec![
                        BpsvValue::String("us".to_string()),
                        BpsvValue::Decimal(1000 + i),
                        BpsvValue::Hex(
                            "deadbeefcafebabedeadbeefcafebabedeadbeefcafebabedeadbeefcafebabe"
                                .to_string(),
                        ),
                    ])
                    .unwrap();
            }

            let output = builder.build().unwrap();
            black_box(output);
        });
    });

    // Building medium documents
    group.bench_function("build_medium_100_rows", |b| {
        b.iter(|| {
            let mut builder = BpsvBuilder::new();
            builder
                .add_field("Region", BpsvFieldType::String(0))
                .unwrap();
            builder
                .add_field("BuildId", BpsvFieldType::Decimal(4))
                .unwrap();
            builder.add_field("Hash", BpsvFieldType::Hex(32)).unwrap();
            builder.set_sequence_number(12345);

            for i in 0..100 {
                builder
                    .add_row(vec![
                        BpsvValue::String("us".to_string()),
                        BpsvValue::Decimal(1000 + i),
                        BpsvValue::Hex(
                            "deadbeefcafebabedeadbeefcafebabedeadbeefcafebabedeadbeefcafebabe"
                                .to_string(),
                        ),
                    ])
                    .unwrap();
            }

            let output = builder.build().unwrap();
            black_box(output);
        });
    });

    group.finish();
}

fn benchmark_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("access");

    // Create a medium-sized document for access benchmarks
    let doc_str = generate_simple_bpsv(100);
    let doc = BpsvDocument::parse(&doc_str).unwrap();

    group.bench_function("get_column_by_name", |b| {
        b.iter(|| {
            // Using raw values for benchmarking since typed access requires mutable access
            for row in doc.rows() {
                // Get by field name through schema
                if let Some(field) = doc.schema().get_field("Region") {
                    let region = row.get_raw(field.index).unwrap();
                    black_box(region);
                }
            }
        });
    });

    group.bench_function("get_column_by_index", |b| {
        b.iter(|| {
            for row in doc.rows() {
                let region = row.get_raw(0).unwrap();
                black_box(region);
            }
        });
    });

    group.bench_function("find_rows_by_value", |b| {
        b.iter(|| {
            let rows = doc.find_rows_by_field("Region", "us").unwrap();
            black_box(rows);
        });
    });

    group.finish();
}

fn benchmark_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("round_trip");

    let doc_str = generate_simple_bpsv(50);

    group.bench_function("parse_build_parse", |b| {
        b.iter(|| {
            // Parse
            let doc = BpsvDocument::parse(black_box(&doc_str)).unwrap();

            // Build
            let output = doc.to_bpsv_string();

            // Parse again
            let doc2 = BpsvDocument::parse(&output).unwrap();
            black_box(doc2);
        });
    });

    group.finish();
}

fn benchmark_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation");

    // Valid hex values
    let valid_hex = "deadbeefcafebabedeadbeefcafebabedeadbeefcafebabedeadbeefcafebabe";
    let hex_type = BpsvFieldType::Hex(32);

    group.bench_function("validate_hex_32", |b| {
        b.iter(|| {
            let is_valid = hex_type.is_valid_value(black_box(valid_hex));
            black_box(is_valid);
        });
    });

    // Decimal parsing
    let decimal_str = "12345";
    let decimal_type = BpsvFieldType::Decimal(4);

    group.bench_function("validate_decimal", |b| {
        b.iter(|| {
            let is_valid = decimal_type.is_valid_value(black_box(decimal_str));
            black_box(is_valid);
        });
    });

    // String length validation
    let string_value = "This is a test string";
    let string_type = BpsvFieldType::String(30);

    group.bench_function("validate_string_length", |b| {
        b.iter(|| {
            let is_valid = string_type.is_valid_value(black_box(string_value));
            black_box(is_valid);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_parsing,
    benchmark_building,
    benchmark_access,
    benchmark_round_trip,
    benchmark_validation
);
criterion_main!(benches);
