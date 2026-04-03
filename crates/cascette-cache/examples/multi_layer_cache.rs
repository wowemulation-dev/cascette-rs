#![allow(clippy::expect_used, clippy::panic)]
//! Multi-layer caching and memory pooling
//!
//! Demonstrates MultiLayerCacheConfig construction, PromotionStrategy options,
//! NgdpMemoryPool with size classes, pool allocation/deallocation, and pool statistics.
//!
//! ```bash
//! cargo run -p cascette-cache --example multi_layer_cache
//! ```

use cascette_cache::{
    config::{DiskCacheConfig, MemoryCacheConfig, MultiLayerCacheConfig, PromotionStrategy},
    pool::{NgdpMemoryPool, NgdpSizeClass, allocate_thread_local, deallocate_thread_local},
    traits::EvictionPolicy,
};
use std::time::Duration;

fn main() {
    println!("=== Multi-Layer Cache Configuration ===");

    let memory_layer = MemoryCacheConfig::new()
        .with_max_entries(10_000)
        .with_max_memory(100 * 1024 * 1024) // 100 MB
        .with_default_ttl(Duration::from_secs(300))
        .with_eviction_policy(EvictionPolicy::Lru);

    let disk_layer = DiskCacheConfig::new("/tmp/cascette-cache")
        .with_max_files(100_000)
        .with_max_disk_usage(1024 * 1024 * 1024) // 1 GB
        .with_default_ttl(Duration::from_secs(24 * 3600))
        .with_subdirectories(true, 2);

    let multi_config = MultiLayerCacheConfig::new()
        .add_memory_layer(memory_layer)
        .add_disk_layer(disk_layer)
        .with_promotion_strategy(PromotionStrategy::OnHit);

    multi_config.validate().expect("config should be valid");

    println!("Layers configured: {}", multi_config.layers.len());
    println!("  Layer 0: Memory (L1)");
    println!("  Layer 1: Disk   (L2)");
    println!("Promotion strategy: {:?}", multi_config.promotion_strategy);
    println!(
        "Cross-layer stats:  {}",
        multi_config.enable_cross_layer_stats
    );

    println!();
    println!("=== Promotion Strategies ===");

    let strategies: &[(&str, PromotionStrategy)] = &[
        ("OnHit", PromotionStrategy::OnHit),
        ("AfterNHits(3)", PromotionStrategy::AfterNHits(3)),
        (
            "FrequencyBased(0.5)",
            PromotionStrategy::FrequencyBased { threshold: 0.5 },
        ),
        (
            "AgeBased(60s)",
            PromotionStrategy::AgeBased {
                min_age: Duration::from_secs(60),
            },
        ),
        ("Manual", PromotionStrategy::Manual),
    ];

    for (label, strategy) in strategies {
        println!("  {label}: {strategy:?}");
    }

    println!();
    println!("=== NGDP Size Classes ===");

    for size_class in NgdpSizeClass::all() {
        println!(
            "  {:?}: buffer={} KB, max_pool={}",
            size_class,
            size_class.buffer_size() / 1024,
            size_class.max_pool_size()
        );
    }

    println!();
    println!("=== Size Class Classification ===");

    let test_sizes: &[(usize, &str)] = &[
        (1024, "1 KB config file"),
        (16 * 1024, "16 KB Ribbit response"),
        (128 * 1024, "128 KB archive index"),
        (2 * 1024 * 1024, "2 MB root file"),
        (16 * 1024 * 1024, "16 MB encoding file"),
    ];

    for (size, description) in test_sizes {
        let class = NgdpSizeClass::from_size(*size);
        println!("  {description} -> {class:?}");
    }

    println!();
    println!("=== Memory Pool Allocation ===");

    let pool = NgdpMemoryPool::new();

    // Allocate buffers of different sizes
    let small_buf = pool.allocate(4 * 1024);
    println!(
        "Small allocation:  requested=4 KB, capacity={} KB",
        small_buf.capacity() / 1024
    );

    let medium_buf = pool.allocate(64 * 1024);
    println!(
        "Medium allocation: requested=64 KB, capacity={} KB",
        medium_buf.capacity() / 1024
    );

    let large_buf = pool.allocate(1024 * 1024);
    println!(
        "Large allocation:  requested=1 MB, capacity={} KB",
        large_buf.capacity() / 1024
    );

    println!();
    println!("=== Pool Reuse (Deallocate then Reallocate) ===");

    // Return buffers to pool
    pool.deallocate(small_buf);
    pool.deallocate(medium_buf);
    pool.deallocate(large_buf);
    println!("Returned 3 buffers to pool.");

    // Reallocate -- should reuse pooled buffers
    let reused_small = pool.allocate(4 * 1024);
    let reused_medium = pool.allocate(64 * 1024);
    println!(
        "Reused small:  capacity={} KB",
        reused_small.capacity() / 1024
    );
    println!(
        "Reused medium: capacity={} KB",
        reused_medium.capacity() / 1024
    );

    // Drop reused buffers (not returned to pool)
    drop(reused_small);
    drop(reused_medium);

    println!();
    println!("=== Pool Warm-Up ===");

    let warm_pool = NgdpMemoryPool::new();
    warm_pool.warm_up();

    for size_class in NgdpSizeClass::all() {
        let stats = warm_pool.size_class_stats(*size_class);
        println!(
            "  {:?}: pool_size={} after warm-up",
            size_class, stats.pool_size
        );
    }

    println!();
    println!("=== Pool Statistics ===");

    let stats_pool = NgdpMemoryPool::new();

    // Generate some traffic across size classes
    for _ in 0..10 {
        let buf = stats_pool.allocate(2 * 1024);
        stats_pool.deallocate(buf);
    }
    for _ in 0..5 {
        let buf = stats_pool.allocate(128 * 1024);
        stats_pool.deallocate(buf);
    }
    for _ in 0..2 {
        let buf = stats_pool.allocate(4 * 1024 * 1024);
        stats_pool.deallocate(buf);
    }

    let total = stats_pool.total_stats();
    println!("Total allocations:  {}", total.allocations);
    println!("Total bytes:        {} KB", total.bytes_allocated / 1024);
    println!("Total reuses:       {}", total.reuses);
    println!("Total pool misses:  {}", total.pool_misses);
    println!("Reuse rate:         {:.1}%", total.reuse_rate() * 100.0);
    println!("Miss rate:          {:.1}%", total.miss_rate() * 100.0);
    println!(
        "Memory efficiency:  {:.1}%",
        total.memory_efficiency() * 100.0
    );

    println!();
    println!("Per size class:");
    for (i, size_class) in NgdpSizeClass::all().iter().enumerate() {
        let sc_stats = &total.size_class_stats[i];
        println!(
            "  {:?}: allocs={}, reuses={}, pool_size={}, reuse_rate={:.1}%",
            size_class,
            sc_stats.allocations,
            sc_stats.reuses,
            sc_stats.pool_size,
            sc_stats.reuse_rate() * 100.0,
        );
    }

    println!();
    println!("=== Thread-Local Pool ===");

    let tl_buf = allocate_thread_local(8 * 1024);
    println!(
        "Thread-local allocation: capacity={} KB",
        tl_buf.capacity() / 1024
    );
    deallocate_thread_local(tl_buf);
    println!("Returned to thread-local pool.");

    let tl_reused = allocate_thread_local(4 * 1024);
    println!(
        "Thread-local reuse:     capacity={} KB",
        tl_reused.capacity() / 1024
    );
    drop(tl_reused);
}
