//! Benchmarks for jenkins3 operations

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use tact_parser::jenkins3::hashlittle2;

// Test data of various sizes
const SMALL_DATA: &[u8] = b"Small test data - 16 bytes";
const MEDIUM_DATA: &[u8] = &[0xf0u8; 1024]; // 1KB
const LARGE_DATA: &[u8] = &[0x0fu8; 1024 * 1024]; // 1MB

fn bench_jenkins3(c: &mut Criterion) {
    let mut group = c.benchmark_group("jenkins3");

    for (name, data) in &[
        ("small", SMALL_DATA),
        ("medium", MEDIUM_DATA),
        ("large", LARGE_DATA),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), data, |b, &data| {
            b.iter_batched(
                || {},
                |_| {
                    let mut pc = 0;
                    let mut pb = 0;
                    hashlittle2(data, &mut pc, &mut pb);
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_jenkins3,);

criterion_main!(benches);
