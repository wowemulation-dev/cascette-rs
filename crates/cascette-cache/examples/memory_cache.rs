#![allow(clippy::expect_used, clippy::panic)]
//! In-memory caching for CASC content
//!
//! Demonstrates the MemoryCache with typed cache keys, eviction policies,
//! TTL-based expiration, and cache statistics.
//!
//! ```bash
//! cargo run -p cascette-cache --example memory_cache
//! ```

use bytes::Bytes;
use cascette_cache::{
    config::MemoryCacheConfig,
    key::{BlteKey, ConfigKey, ContentCacheKey, ManifestKey, RibbitKey},
    memory_cache::MemoryCache,
    traits::{AsyncCache, EvictionPolicy},
};
use cascette_crypto::{ContentKey as CryptoContentKey, EncodingKey};
use std::time::Duration;

#[tokio::main]
async fn main() {
    println!("=== Memory Cache Configuration ===");

    let config = MemoryCacheConfig::new()
        .with_max_entries(1_000)
        .with_max_memory(50 * 1024 * 1024) // 50 MB
        .with_default_ttl(Duration::from_secs(300)) // 5 minutes
        .with_eviction_policy(EvictionPolicy::Lru);

    println!("Max entries:     {}", config.max_entries);
    println!(
        "Max memory:      {} MB",
        config.max_memory_bytes.unwrap_or(0) / (1024 * 1024)
    );
    println!("Default TTL:     {:?}", config.default_ttl);
    println!("Eviction policy: {:?}", config.eviction_policy);
    config.validate().expect("config should be valid");
    println!("Configuration validated.");

    println!();
    println!("=== Cache Key Types ===");

    // RibbitKey -- service discovery responses
    let ribbit_key = RibbitKey::new("summary", "us");
    println!("RibbitKey:          {ribbit_key}");

    let ribbit_product = RibbitKey::with_product("builds", "eu", "wow");
    println!("RibbitKey (product): {ribbit_product}");

    // ConfigKey -- build/CDN configuration files
    let config_key = ConfigKey::new("buildconfig", "abc123def456");
    println!("ConfigKey:          {config_key}");

    // BlteKey -- BLTE-encoded content
    let encoding_key = EncodingKey::from_data(b"example encoding data");
    let blte_key = BlteKey::new(encoding_key);
    println!("BlteKey:            {blte_key}");

    let blte_block = BlteKey::with_block(encoding_key, 3);
    println!("BlteKey (block 3):  {blte_block}");

    // ContentCacheKey -- decompressed content
    let content_key = CryptoContentKey::from_data(b"example content data");
    let content_cache_key = ContentCacheKey::new(content_key);
    println!("ContentCacheKey:    {content_cache_key}");

    // ManifestKey -- root/encoding/install manifests
    let manifest_key = ManifestKey::new("root", content_key);
    println!("ManifestKey:        {manifest_key}");

    let manifest_versioned = ManifestKey::with_version("encoding", content_key, "v2");
    println!("ManifestKey (v2):   {manifest_versioned}");

    println!();
    println!("=== Fast Hashing ===");
    let hash = ribbit_key.fast_hash();
    println!(
        "RibbitKey hash32={:#010x}, hash64={:#018x}",
        hash.hash32, hash.hash64
    );
    let hash2 = ribbit_key.fast_hash();
    println!("Same key, same hash: {}", hash.fast_eq(&hash2));
    let other_hash = ribbit_product.fast_hash();
    println!("Different key, same hash: {}", hash.fast_eq(&other_hash));

    println!();
    println!("=== Put / Get / Contains / Remove ===");

    let cache: MemoryCache<RibbitKey> =
        MemoryCache::new(config.clone()).expect("cache creation should succeed");

    let key = RibbitKey::new("versions", "us");
    let value = Bytes::from(b"v1.0.0\nv1.0.1\n".as_slice());

    cache
        .put(key.clone(), value.clone())
        .await
        .expect("put should succeed");
    println!("Put key: {key}");

    let retrieved = cache
        .get(&key)
        .await
        .expect("get should succeed")
        .expect("key should exist");
    println!(
        "Get key: {} bytes, matches: {}",
        retrieved.len(),
        retrieved == value
    );

    let exists = cache.contains(&key).await.expect("contains should succeed");
    println!("Contains: {exists}");

    let removed = cache.remove(&key).await.expect("remove should succeed");
    println!("Removed: {removed}");

    let after_remove = cache.get(&key).await.expect("get should succeed");
    println!("Get after remove: {after_remove:?}");

    println!();
    println!("=== TTL-Based Expiration ===");

    let short_ttl_config = MemoryCacheConfig::new()
        .with_max_entries(100)
        .with_eviction_policy(EvictionPolicy::Lru);
    let ttl_cache: MemoryCache<ConfigKey> =
        MemoryCache::new(short_ttl_config).expect("cache creation should succeed");

    let cfg_key = ConfigKey::new("cdnconfig", "deadbeef01234567");
    let cfg_value = Bytes::from("cdn-host = http://cdn.example.com");

    // Store with a 100ms TTL
    ttl_cache
        .put_with_ttl(
            cfg_key.clone(),
            cfg_value.clone(),
            Duration::from_millis(100),
        )
        .await
        .expect("put_with_ttl should succeed");
    println!("Stored with 100ms TTL: {cfg_key}");

    let before = ttl_cache.get(&cfg_key).await.expect("get should succeed");
    println!("Before expiry: present={}", before.is_some());

    tokio::time::sleep(Duration::from_millis(150)).await;

    let after = ttl_cache.get(&cfg_key).await.expect("get should succeed");
    println!("After expiry:  present={}", after.is_some());

    println!();
    println!("=== Cache Statistics ===");

    let stats_config = MemoryCacheConfig::new()
        .with_max_entries(500)
        .with_max_memory(10 * 1024 * 1024)
        .with_eviction_policy(EvictionPolicy::Lfu);
    let stats_cache: MemoryCache<RibbitKey> =
        MemoryCache::new(stats_config).expect("cache creation should succeed");

    // Generate some traffic
    for i in 0..20 {
        let k = RibbitKey::new(format!("endpoint-{i}"), "us");
        stats_cache
            .put(k, Bytes::from(vec![0u8; 256]))
            .await
            .expect("put should succeed");
    }

    // Hit some keys, miss others
    for i in 0..30 {
        let k = RibbitKey::new(format!("endpoint-{i}"), "us");
        let _ = stats_cache.get(&k).await.expect("get should succeed");
    }

    let stats = stats_cache.stats().await.expect("stats should succeed");
    println!("Entries:      {}", stats.entry_count);
    println!("Memory usage: {} bytes", stats.memory_usage_bytes);
    println!("Get count:    {}", stats.get_count);
    println!("Hit count:    {}", stats.hit_count);
    println!("Miss count:   {}", stats.miss_count);
    println!("Hit rate:     {:.1}%", stats.hit_rate() * 100.0);

    println!();
    println!("=== Eviction Policies ===");
    let policies = [
        EvictionPolicy::Lru,
        EvictionPolicy::Lfu,
        EvictionPolicy::Fifo,
        EvictionPolicy::Random,
        EvictionPolicy::Ttl,
    ];
    for policy in &policies {
        println!("  {policy:?}");
    }
    println!("Each policy determines which entries are removed when the cache is full.");
}
