//! Benchmarks for the Ribbit client

use criterion::{Criterion, criterion_group, criterion_main};
use ribbit_client::{Endpoint, ProtocolVersion, Region};
use std::hint::black_box;

fn bench_region_parsing(c: &mut Criterion) {
    c.bench_function("parse region from string", |b| {
        b.iter(|| {
            let _region: Region = black_box("us").parse().unwrap();
        })
    });
}

fn bench_endpoint_path_generation(c: &mut Criterion) {
    let endpoints = vec![
        Endpoint::Summary,
        Endpoint::ProductVersions("wow".to_string()),
        Endpoint::ProductCdns("wow_classic".to_string()),
        Endpoint::Cert("5168ff90af0207753cccd9656462a212b859723b".to_string()),
    ];

    c.bench_function("generate endpoint paths", |b| {
        b.iter(|| {
            for endpoint in &endpoints {
                let _path = black_box(endpoint.as_path());
            }
        })
    });
}

fn bench_command_formatting(c: &mut Criterion) {
    let endpoint = Endpoint::ProductVersions("wow".to_string());
    let version = ProtocolVersion::V1;

    c.bench_function("format ribbit command", |b| {
        b.iter(|| {
            let _command = format!(
                "{}/{}\n",
                black_box(version.prefix()),
                black_box(endpoint.as_path())
            );
        })
    });
}

criterion_group!(
    benches,
    bench_region_parsing,
    bench_endpoint_path_generation,
    bench_command_formatting
);
criterion_main!(benches);
