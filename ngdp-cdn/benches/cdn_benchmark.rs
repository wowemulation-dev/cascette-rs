//! Benchmarks for ngdp-cdn

use criterion::{Criterion, criterion_group, criterion_main};
use ngdp_cdn::{CdnClient, CdnClientBuilder, CdnClientBuilderTrait as _};
use std::hint::black_box;

fn benchmark_url_building(c: &mut Criterion) {
    c.bench_function("build_url", |b| {
        b.iter(|| {
            CdnClient::build_url(
                black_box("blzddist1-a.akamaihd.net"),
                black_box("tpr/wow"),
                black_box("2e9c1e3b5f5a0c9d9e8f1234567890ab"),
                black_box(""),
            )
        })
    });
}

fn benchmark_client_creation(c: &mut Criterion) {
    c.bench_function("client_new", |b| b.iter(CdnClient::new));

    c.bench_function("client_builder", |b| {
        b.iter(|| {
            CdnClientBuilder::new()
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

fn benchmark_parallel_setup(c: &mut Criterion) {
    let _rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("parallel_download_setup_10", |b| {
        b.iter(|| {
            // Just measure the setup overhead, not actual downloads
            let hashes: Vec<String> = (0..10).map(|i| format!("hash{i:032x}")).collect();
            black_box(hashes);
        })
    });

    c.bench_function("parallel_download_setup_100", |b| {
        b.iter(|| {
            let hashes: Vec<String> = (0..100).map(|i| format!("hash{i:032x}")).collect();
            black_box(hashes);
        })
    });

    c.bench_function("parallel_futures_creation", |b| {
        let _client = CdnClient::new().unwrap();
        let hashes: Vec<String> = (0..10).map(|i| format!("hash{i:032x}")).collect();

        b.iter(|| {
            use futures_util::stream::{self, StreamExt};

            let futures = hashes.iter().map(|hash| {
                let cdn_host = "example.com".to_string();
                let path = "test".to_string();
                let hash = hash.clone();

                async move {
                    // Just simulate the future creation, not execution
                    (cdn_host, path, hash)
                }
            });

            let stream = stream::iter(futures).buffer_unordered(10);
            let _ = black_box(stream);
        })
    });
}

criterion_group!(
    benches,
    benchmark_url_building,
    benchmark_client_creation,
    benchmark_backoff_calculation,
    benchmark_parallel_setup
);
criterion_main!(benches);
