#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

//! Cache verification: store, retrieve, and eviction behavior.
//!
//! Exercises `cascette-cache` `MemoryCache` with round-trip storage and
//! LRU eviction.
//!
//! Usage:
//!   cargo run -p cascette-cache --example cache_verification

use cascette_cache::MemoryCache;
use cascette_cache::config::MemoryCacheConfig;
use cascette_cache::key::ConfigKey;
use cascette_cache::traits::AsyncCache;

#[tokio::main]
async fn main() {
    // D3: Store and retrieve
    println!("=== D3: Cache store and retrieve ===");
    let cache = MemoryCache::<ConfigKey>::new(MemoryCacheConfig::new().with_max_entries(100))
        .expect("cache creation");

    let key = ConfigKey::new("buildconfig", "abc123def456");
    let value = bytes::Bytes::from("build config content here");

    cache
        .put(key.clone(), value.clone())
        .await
        .expect("cache put should succeed");

    let retrieved = cache
        .get(&key)
        .await
        .expect("cache get should succeed")
        .expect("key should exist");

    assert_eq!(
        retrieved, value,
        "retrieved value should match stored value"
    );

    let missing_key = ConfigKey::new("buildconfig", "000000000000");
    let missing = cache
        .get(&missing_key)
        .await
        .expect("cache get should not error");
    assert!(missing.is_none(), "nonexistent key should return None");

    println!("  round-trip: OK");

    // D4: Eviction
    println!("\n=== D4: Cache eviction ===");
    let cache = MemoryCache::<ConfigKey>::new(MemoryCacheConfig::new().with_max_entries(5))
        .expect("cache creation");

    for i in 0..10u32 {
        let key = ConfigKey::new("buildconfig", format!("{i:032x}"));
        let value = bytes::Bytes::from(format!("value-{i}"));
        cache.put(key, value).await.expect("cache put");
    }

    let latest_key = ConfigKey::new("buildconfig", format!("{:032x}", 9));
    let latest = cache.get(&latest_key).await.expect("get latest");
    assert!(latest.is_some(), "most recent entry should still exist");

    let mut evicted_count = 0;
    for i in 0..5u32 {
        let key = ConfigKey::new("buildconfig", format!("{i:032x}"));
        if cache.get(&key).await.expect("get").is_none() {
            evicted_count += 1;
        }
    }
    assert!(
        evicted_count > 0,
        "some earlier entries should have been evicted (max_entries=5, stored 10)"
    );

    println!("  {evicted_count}/5 earlier entries evicted");

    println!("\nAll cache checks passed.");
}
