//! Benchmarks for ngdp-cdn

use criterion::{Criterion, criterion_group, criterion_main};
use ngdp_cdn::CdnClient;
use std::hint::black_box;

fn benchmark_url_building(c: &mut Criterion) {
    c.bench_function("build_url", |b| {
        b.iter(|| {
            CdnClient::build_url(
                black_box("blzddist1-a.akamaihd.net"),
                black_box("tpr/wow"),
                black_box("2e9c1e3b5f5a0c9d9e8f1234567890ab"),
            )
        })
    });
}

fn benchmark_client_creation(c: &mut Criterion) {
    c.bench_function("client_new", |b| b.iter(CdnClient::new));

    c.bench_function("client_builder", |b| {
        b.iter(|| {
            CdnClient::builder()
                .max_retries(5)
                .initial_backoff_ms(200)
                .build()
        })
    });
}

fn benchmark_backoff_calculation(c: &mut Criterion) {
    let client = CdnClient::new().unwrap();

    c.bench_function("calculate_backoff_0", |b| {
        b.iter(|| client.calculate_backoff(black_box(0)))
    });

    c.bench_function("calculate_backoff_5", |b| {
        b.iter(|| client.calculate_backoff(black_box(5)))
    });
}

criterion_group!(
    benches,
    benchmark_url_building,
    benchmark_client_creation,
    benchmark_backoff_calculation
);
criterion_main!(benches);
