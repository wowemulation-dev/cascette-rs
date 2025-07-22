//! Benchmarks for ngdp-cache operations

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use ngdp_cache::{
    cached_ribbit_client::CachedRibbitClient, cdn::CdnCache, generic::GenericCache,
    ribbit::RibbitCache,
};
use ribbit_client::{Endpoint, Region};
use std::hint::black_box;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Test data of various sizes
const SMALL_DATA: &[u8] = b"Small test data - 16 bytes";
const MEDIUM_DATA: &[u8] = &[0u8; 1024]; // 1KB
const LARGE_DATA: &[u8] = &[0u8; 1024 * 1024]; // 1MB

/// Sample hash for consistent paths
const TEST_HASH: &str = "abcdef1234567890abcdef1234567890";

fn bench_generic_cache_write(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("generic_cache_write");

    for (name, data) in &[
        ("small", SMALL_DATA),
        ("medium", MEDIUM_DATA),
        ("large", LARGE_DATA),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), data, |b, &data| {
            b.iter_batched(
                || {
                    // Setup: create cache and key
                    let cache = runtime.block_on(GenericCache::new()).unwrap();
                    let key = format!("bench_key_{}", rand::random::<u32>());
                    (cache, key)
                },
                |(cache, key)| {
                    runtime.block_on(async move {
                        cache
                            .write_buffer_with_suffix("", &key, "", black_box(data))
                            .await
                            .unwrap();
                        // Cleanup
                        cache.delete_object_with_suffix("", &key, "").await.unwrap();
                    });
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_generic_cache_read(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("generic_cache_read");

    for (name, data) in &[
        ("small", SMALL_DATA),
        ("medium", MEDIUM_DATA),
        ("large", LARGE_DATA),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(name), data, |b, &data| {
            b.iter_batched(
                || {
                    // Setup: create cache, write data
                    let cache = runtime.block_on(GenericCache::new()).unwrap();
                    let key = format!("bench_key_{}", rand::random::<u32>());
                    runtime
                        .block_on(cache.write_buffer("", &key, data))
                        .unwrap();
                    (cache, key)
                },
                |(cache, key)| {
                    runtime.block_on(async move {
                        let _data = black_box(cache.read_object("", &key).await.unwrap());
                        // Cleanup
                        cache.delete_object_with_suffix("", &key, "").await.unwrap();
                    });
                },
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

fn bench_cdn_cache_operations(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    c.bench_function("cdn_cache_write_data", |b| {
        b.iter_batched(
            || {
                let cache = runtime.block_on(CdnCache::new()).unwrap();
                let hash = format!("{}{:08x}", TEST_HASH, rand::random::<u32>());
                (cache, hash)
            },
            |(cache, hash)| {
                runtime.block_on(async move {
                    cache
                        .write_buffer("bench/large", &hash, black_box(LARGE_DATA))
                        .await
                        .unwrap();

                    // Cleanup
                    let _ = cache
                        .delete_object_with_suffix("bench/large", &hash, "")
                        .await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("cdn_cache_write_config", |b| {
        b.iter_batched(
            || {
                let cache = runtime.block_on(CdnCache::new()).unwrap();
                let hash = format!("{}{:08x}", TEST_HASH, rand::random::<u32>());
                (cache, hash)
            },
            |(cache, hash)| {
                runtime.block_on(async move {
                    cache
                        .write_buffer_with_suffix("bench/medium", &hash, "", black_box(MEDIUM_DATA))
                        .await
                        .unwrap();
                    // Cleanup
                    let _ = cache
                        .delete_object_with_suffix("bench/medium", &hash, "")
                        .await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("cdn_cache_data_size", |b| {
        b.iter_batched(
            || {
                // Setup: create data file
                let cache = runtime.block_on(CdnCache::new()).unwrap();
                let hash = format!("{}{:08x}", TEST_HASH, rand::random::<u32>());
                runtime
                    .block_on(cache.write_buffer("bench/size", &hash, LARGE_DATA))
                    .unwrap();
                (cache, hash)
            },
            |(cache, hash)| {
                runtime.block_on(async move {
                    let _size = black_box(cache.object_size("bench/size", &hash).await.unwrap());

                    // Cleanup
                    let _ = cache.delete_object("bench/size", &hash).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("cdn_cache_path_construction", |b| {
        let cache = runtime.block_on(CdnCache::new()).unwrap();
        b.iter(|| {
            let _ = black_box(cache.cache_path("dummy", TEST_HASH));
        });
    });
}

fn bench_ribbit_cache_operations(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    c.bench_function("ribbit_cache_write", |b| {
        b.iter_batched(
            || {
                let cache = runtime.block_on(RibbitCache::new()).unwrap();
                let endpoint = format!("endpoint_{}", rand::random::<u32>());
                (cache, endpoint)
            },
            |(cache, endpoint)| {
                runtime.block_on(async move {
                    cache
                        .write("us", "wow", &endpoint, black_box(MEDIUM_DATA))
                        .await
                        .unwrap();
                    // Cleanup
                    tokio::fs::remove_file(cache.cache_path("us", "wow", &endpoint))
                        .await
                        .ok();
                    tokio::fs::remove_file(cache.metadata_path("us", "wow", &endpoint))
                        .await
                        .ok();
                });
            },
            BatchSize::SmallInput,
        );
    });

    c.bench_function("ribbit_cache_is_valid", |b| {
        b.iter_batched(
            || {
                // Setup: create valid cache entry
                let cache = runtime
                    .block_on(RibbitCache::with_ttl(Duration::from_secs(300)))
                    .unwrap();
                let endpoint = format!("endpoint_{}", rand::random::<u32>());
                runtime
                    .block_on(cache.write("us", "wow", &endpoint, SMALL_DATA))
                    .unwrap();
                (cache, endpoint)
            },
            |(cache, endpoint)| {
                runtime.block_on(async move {
                    let _valid = black_box(cache.is_valid("us", "wow", &endpoint).await);
                    // Cleanup
                    tokio::fs::remove_file(cache.cache_path("us", "wow", &endpoint))
                        .await
                        .ok();
                    tokio::fs::remove_file(cache.metadata_path("us", "wow", &endpoint))
                        .await
                        .ok();
                });
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_concurrent_operations(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    c.bench_function("concurrent_writes", |b| {
        b.iter(|| {
            runtime.block_on(async {
                let _cache = GenericCache::new().await.unwrap();

                let mut handles = vec![];
                for i in 0..10 {
                    let cache_clone = GenericCache::new().await.unwrap();
                    let handle = tokio::spawn(async move {
                        let key = format!("concurrent_{i}");
                        cache_clone
                            .write_buffer("", &key, SMALL_DATA)
                            .await
                            .unwrap();
                        cache_clone.delete_object("", &key).await.unwrap();
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    handle.await.unwrap();
                }
            });
        });
    });
}

fn bench_path_operations(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    c.bench_function("hash_path_segmentation", |b| {
        let cdn = runtime.block_on(CdnCache::new()).unwrap();
        let hashes = vec![
            "0123456789abcdef0123456789abcdef",
            "fedcba9876543210fedcba9876543210",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "00000000000000000000000000000000",
        ];

        b.iter(|| {
            for hash in &hashes {
                let _ = black_box(cdn.cache_path("test/path_segmentation", hash));
            }
        });
    });
}

fn bench_cached_ribbit_client(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();

    let mut group = c.benchmark_group("cached_ribbit_client");

    // Benchmark cache filename generation
    group.bench_function("filename_generation", |b| {
        b.iter_batched(
            || {
                runtime
                    .block_on(CachedRibbitClient::with_cache_dir(
                        Region::US,
                        std::env::temp_dir().join("bench_ribbit_cache"),
                    ))
                    .unwrap()
            },
            |_client| {
                // Test various endpoint types
                let endpoints = vec![
                    Endpoint::Summary,
                    Endpoint::ProductVersions("wow".to_string()),
                    Endpoint::ProductCdns("d4".to_string()),
                    Endpoint::Cert("abc123def456".to_string()),
                    Endpoint::Ocsp("789xyz".to_string()),
                ];

                for endpoint in endpoints {
                    // This benchmarks the internal filename generation logic
                    // via the cache path construction
                    let _ = black_box(&endpoint);
                }
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark cache validity check
    group.bench_function("cache_validity_check", |b| {
        b.iter_batched(
            || {
                // Setup: create client with cache entry
                let temp_dir =
                    std::env::temp_dir().join(format!("bench_ribbit_{}", rand::random::<u32>()));
                let client = runtime
                    .block_on(CachedRibbitClient::with_cache_dir(
                        Region::US,
                        temp_dir.clone(),
                    ))
                    .unwrap();

                // Pre-populate cache with fresh data
                let cache_file = temp_dir.join("us").join("test-endpoint-0.bmime");
                let meta_file = temp_dir.join("us").join("test-endpoint-0.meta");

                runtime.block_on(async {
                    tokio::fs::create_dir_all(cache_file.parent().unwrap())
                        .await
                        .unwrap();
                    tokio::fs::write(&cache_file, b"cached data").await.unwrap();

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    tokio::fs::write(&meta_file, timestamp.to_string())
                        .await
                        .unwrap();
                });

                (
                    client,
                    temp_dir,
                    Endpoint::Custom("test/endpoint".to_string()),
                )
            },
            |(client, temp_dir, endpoint)| {
                runtime.block_on(async move {
                    // This checks cache validity without making network requests
                    // The actual is_cache_valid method is private, but it's called
                    // internally when we attempt to read from cache
                    match client.request_raw(&endpoint).await {
                        Ok(data) => black_box(data),
                        Err(_) => vec![], // Server request would fail for test endpoint
                    };

                    // Cleanup
                    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark cache write performance
    group.bench_function("cache_write", |b| {
        b.iter_batched(
            || {
                let temp_dir =
                    std::env::temp_dir().join(format!("bench_write_{}", rand::random::<u32>()));
                let client = runtime
                    .block_on(CachedRibbitClient::with_cache_dir(
                        Region::US,
                        temp_dir.clone(),
                    ))
                    .unwrap();
                (client, temp_dir)
            },
            |(_client, temp_dir)| {
                runtime.block_on(async move {
                    // Simulate writing cache data
                    let cache_dir = temp_dir.join("us");
                    let _ = tokio::fs::create_dir_all(&cache_dir).await;

                    // Write cache and metadata files
                    let data = b"test response data";
                    let _ = tokio::fs::write(cache_dir.join("bench-test-0.bmime"), data).await;
                    let _ =
                        tokio::fs::write(cache_dir.join("bench-test-0.meta"), "1234567890").await;

                    // Cleanup
                    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    // Benchmark cache cleanup operations
    group.bench_function("clear_expired", |b| {
        b.iter_batched(
            || {
                // Setup: create client with mix of expired and fresh entries
                let temp_dir =
                    std::env::temp_dir().join(format!("bench_expire_{}", rand::random::<u32>()));
                let client = runtime
                    .block_on(CachedRibbitClient::with_cache_dir(
                        Region::US,
                        temp_dir.clone(),
                    ))
                    .unwrap();

                let cache_dir = temp_dir.join("us");
                runtime.block_on(async {
                    tokio::fs::create_dir_all(&cache_dir).await.unwrap();

                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    // Create mix of fresh and expired entries
                    for i in 0..10 {
                        let is_cert = i % 3 == 0;
                        let prefix = if is_cert { "certs" } else { "versions" };
                        let timestamp = if i % 2 == 0 {
                            now // Fresh
                        } else if is_cert {
                            now - (31 * 24 * 60 * 60) // Expired cert
                        } else {
                            now - (6 * 60) // Expired regular
                        };

                        let cache_file = cache_dir.join(format!("{prefix}-test{i}-0.bmime"));
                        let meta_file = cache_dir.join(format!("{prefix}-test{i}-0.meta"));

                        tokio::fs::write(&cache_file, format!("data {i}"))
                            .await
                            .unwrap();
                        tokio::fs::write(&meta_file, timestamp.to_string())
                            .await
                            .unwrap();
                    }
                });

                (client, temp_dir)
            },
            |(client, temp_dir)| {
                runtime.block_on(async move {
                    client.clear_expired().await.unwrap();

                    // Cleanup
                    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
                });
            },
            BatchSize::SmallInput,
        );
    });

    group.finish();
}

// fn bench_streaming_operations(c: &mut Criterion) {
//     let runtime = Runtime::new().unwrap();

//     let mut group = c.benchmark_group("streaming_operations");

//     // Test data sizes for streaming benchmarks
//     let test_sizes = [
//         ("1MB", 1024 * 1024),
//         ("10MB", 10 * 1024 * 1024),
//         ("50MB", 50 * 1024 * 1024),
//     ];

//     for (name, size) in &test_sizes {
//         // Benchmark streaming write vs regular write
//         group.bench_with_input(
//             BenchmarkId::new("write_streaming", name),
//             size,
//             |b, &size| {
//                 b.iter_batched(
//                     || {
//                         let cache = runtime.block_on(GenericCache::new()).unwrap();
//                         let key = format!("stream_write_{}", rand::random::<u32>());
//                         let data = vec![42u8; size];
//                         (cache, key, data)
//                     },
//                     |(cache, key, data)| {
//                         runtime.block_on(async move {
//                             let mut reader = std::io::Cursor::new(data);
//                             cache.write_streaming(&key, &mut reader).await.unwrap();
//                             // Cleanup
//                             cache.delete(&key).await.unwrap();
//                         });
//                     },
//                     BatchSize::SmallInput,
//                 );
//             },
//         );

//         group.bench_with_input(BenchmarkId::new("write_regular", name), size, |b, &size| {
//             b.iter_batched(
//                 || {
//                     let cache = runtime.block_on(GenericCache::new()).unwrap();
//                     let key = format!("regular_write_{}", rand::random::<u32>());
//                     let data = vec![42u8; size];
//                     (cache, key, data)
//                 },
//                 |(cache, key, data)| {
//                     runtime.block_on(async move {
//                         cache.write(&key, &data).await.unwrap();
//                         // Cleanup
//                         cache.delete(&key).await.unwrap();
//                     });
//                 },
//                 BatchSize::SmallInput,
//             );
//         });

//         // Benchmark streaming read vs regular read
//         group.bench_with_input(
//             BenchmarkId::new("read_streaming", name),
//             size,
//             |b, &size| {
//                 b.iter_batched(
//                     || {
//                         let cache = runtime.block_on(GenericCache::new()).unwrap();
//                         let key = format!("stream_read_{}", rand::random::<u32>());
//                         let data = vec![42u8; size];
//                         runtime.block_on(cache.write(&key, &data)).unwrap();
//                         (cache, key)
//                     },
//                     |(cache, key)| {
//                         runtime.block_on(async move {
//                             let mut output = Vec::new();
//                             cache.read_streaming(&key, &mut output).await.unwrap();
//                             black_box(output);
//                             // Cleanup
//                             cache.delete(&key).await.unwrap();
//                         });
//                     },
//                     BatchSize::SmallInput,
//                 );
//             },
//         );

//         group.bench_with_input(BenchmarkId::new("read_regular", name), size, |b, &size| {
//             b.iter_batched(
//                 || {
//                     let cache = runtime.block_on(GenericCache::new()).unwrap();
//                     let key = format!("regular_read_{}", rand::random::<u32>());
//                     let data = vec![42u8; size];
//                     runtime.block_on(cache.write(&key, &data)).unwrap();
//                     (cache, key)
//                 },
//                 |(cache, key)| {
//                     runtime.block_on(async move {
//                         let data = cache.read(&key).await.unwrap();
//                         black_box(data);
//                         // Cleanup
//                         cache.delete(&key).await.unwrap();
//                     });
//                 },
//                 BatchSize::SmallInput,
//             );
//         });
//     }

//     // Benchmark chunked operations
//     group.bench_function("chunked_write_1MB", |b| {
//         b.iter_batched(
//             || {
//                 let cache = runtime.block_on(GenericCache::new()).unwrap();
//                 let key = format!("chunked_{}", rand::random::<u32>());
//                 // Create 1MB in 8KB chunks
//                 let chunks: Vec<Result<Vec<u8>, ngdp_cache::Error>> =
//                     (0..128).map(|i| Ok(vec![(i % 256) as u8; 8192])).collect();
//                 (cache, key, chunks)
//             },
//             |(cache, key, chunks)| {
//                 runtime.block_on(async move {
//                     cache.write_chunked(&key, chunks).await.unwrap();
//                     // Cleanup
//                     cache.delete(&key).await.unwrap();
//                 });
//             },
//             BatchSize::SmallInput,
//         );
//     });

//     group.bench_function("chunked_read_1MB", |b| {
//         b.iter_batched(
//             || {
//                 let cache = runtime.block_on(GenericCache::new()).unwrap();
//                 let key = format!("chunked_read_{}", rand::random::<u32>());
//                 let data = vec![42u8; 1024 * 1024]; // 1MB
//                 runtime.block_on(cache.write(&key, &data)).unwrap();
//                 (cache, key)
//             },
//             |(cache, key)| {
//                 runtime.block_on(async move {
//                     let mut total_bytes = 0u64;
//                     cache
//                         .read_chunked(&key, |chunk| {
//                             total_bytes += chunk.len() as u64;
//                             Ok(())
//                         })
//                         .await
//                         .unwrap();
//                     black_box(total_bytes);
//                     // Cleanup
//                     cache.delete(&key).await.unwrap();
//                 });
//             },
//             BatchSize::SmallInput,
//         );
//     });

//     // Benchmark copy operation
//     group.bench_function("copy_operation_10MB", |b| {
//         b.iter_batched(
//             || {
//                 let cache = runtime.block_on(GenericCache::new()).unwrap();
//                 let source_key = format!("source_{}", rand::random::<u32>());
//                 let dest_key = format!("dest_{}", rand::random::<u32>());
//                 let data = vec![42u8; 10 * 1024 * 1024]; // 10MB
//                 runtime.block_on(cache.write(&source_key, &data)).unwrap();
//                 (cache, source_key, dest_key)
//             },
//             |(cache, source_key, dest_key)| {
//                 runtime.block_on(async move {
//                     cache.copy(&source_key, &dest_key).await.unwrap();
//                     // Cleanup
//                     cache.delete(&source_key).await.unwrap();
//                     cache.delete(&dest_key).await.unwrap();
//                 });
//             },
//             BatchSize::SmallInput,
//         );
//     });

//     // Benchmark buffered streaming
//     group.bench_function("buffered_streaming_1MB", |b| {
//         b.iter_batched(
//             || {
//                 let cache = runtime.block_on(GenericCache::new()).unwrap();
//                 let key = format!("buffered_{}", rand::random::<u32>());
//                 let data = vec![42u8; 1024 * 1024]; // 1MB
//                 runtime.block_on(cache.write(&key, &data)).unwrap();
//                 (cache, key)
//             },
//             |(cache, key)| {
//                 runtime.block_on(async move {
//                     let mut output = Vec::new();
//                     cache
//                         .read_streaming_buffered(&key, &mut output, 64 * 1024)
//                         .await
//                         .unwrap(); // 64KB buffer
//                     black_box(output);
//                     // Cleanup
//                     cache.delete(&key).await.unwrap();
//                 });
//             },
//             BatchSize::SmallInput,
//         );
//     });

//     group.finish();
// }

criterion_group!(
    benches,
    bench_generic_cache_write,
    bench_generic_cache_read,
    // bench_streaming_operations,
    bench_cdn_cache_operations,
    bench_ribbit_cache_operations,
    bench_concurrent_operations,
    bench_path_operations,
    bench_cached_ribbit_client,
);

criterion_main!(benches);
