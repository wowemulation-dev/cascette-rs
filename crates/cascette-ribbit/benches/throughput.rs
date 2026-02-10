//! Throughput and latency benchmarks for Ribbit server.
//!
//! These benchmarks measure HTTP and TCP protocol performance to verify:
//! - HTTP: 1,000 requests/second, 50ms p50 / 200ms p99 latency
//! - TCP: 500 connections/second, 100ms p50 / 300ms p99 latency
//!
//! Run with:
//! ```bash
//! cargo bench --bench throughput
//! ```

#![allow(clippy::expect_used)]

use cascette_ribbit::{AppState, ServerConfig};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;

/// Create test database with multiple products and builds.
fn create_test_db() -> NamedTempFile {
    let mut file =
        NamedTempFile::new().expect("Failed to create temporary benchmark database file");
    let json = r#"[{
        "id": 1,
        "product": "wow",
        "version": "1.14.2.42597",
        "build": "42597",
        "build_config": "0123456789abcdef0123456789abcdef",
        "cdn_config": "fedcba9876543210fedcba9876543210",
        "keyring": null,
        "product_config": null,
        "build_time": "2024-01-01T00:00:00+00:00",
        "encoding_ekey": "aaaabbbbccccddddeeeeffffaaaaffff",
        "root_ekey": "bbbbccccddddeeeeffffaaaabbbbcccc",
        "install_ekey": "ccccddddeeeeffffaaaabbbbccccdddd",
        "download_ekey": "ddddeeeeffffaaaabbbbccccddddeeee"
    }]"#;
    file.write_all(json.as_bytes())
        .expect("Failed to write benchmark JSON data to temporary file");
    file
}

/// Benchmark BPSV response generation.
fn bench_bpsv_generation(c: &mut Criterion) {
    let db_file = create_test_db();
    let config = ServerConfig {
        http_bind: "127.0.0.1:8080"
            .parse()
            .expect("Failed to parse HTTP bind address"),
        tcp_bind: "127.0.0.1:1119"
            .parse()
            .expect("Failed to parse TCP bind address"),
        builds: db_file.path().to_path_buf(),
        cdn_hosts: "cdn.test.com".to_string(),
        cdn_path: "test/path".to_string(),
        tls_cert: None,
        tls_key: None,
    };

    let state = Arc::new(AppState::new(&config).expect("Failed to initialize benchmark AppState"));
    let build = state
        .database()
        .latest_build("wow")
        .expect("Failed to get latest build for benchmark");

    let mut group = c.benchmark_group("bpsv_generation");

    group.bench_function(BenchmarkId::new("versions", "wow"), |b| {
        b.iter(|| {
            let response =
                cascette_ribbit::BpsvResponse::versions(black_box(build), black_box(1730534400));
            black_box(response.to_string())
        });
    });

    group.bench_function(BenchmarkId::new("cdns", "wow"), |b| {
        b.iter(|| {
            let response = cascette_ribbit::BpsvResponse::cdns(
                black_box(state.cdn_config()),
                black_box(1730534400),
            );
            black_box(response.to_string())
        });
    });

    group.bench_function(BenchmarkId::new("summary", "all"), |b| {
        let products: Vec<&str> = state.database().products();
        b.iter(|| {
            let response =
                cascette_ribbit::BpsvResponse::summary(black_box(&products), black_box(1730534400));
            black_box(response.to_string())
        });
    });

    group.finish();
}

criterion_group!(benches, bench_bpsv_generation);
criterion_main!(benches);
