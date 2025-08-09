//! Test that cached data is still valid and being used

#[cfg(target_os = "linux")]
use ngdp_cache::{generic::GenericCache, ribbit::RibbitCache};
#[cfg(target_os = "linux")]
use tempfile::TempDir;

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_cache_ttl_and_validity() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    // SAFETY: Test runs in isolation. Setting XDG_CACHE_HOME for test environment is safe.
    unsafe {
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
    }

    let cache = RibbitCache::new().await?;

    // Write test data with known TTL
    let test_data = b"test certificate data";
    cache
        .write("us", "certs", "test-cert-hash", test_data)
        .await?;

    // Small delay to ensure filesystem operations complete
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // First read to verify data is written
    let data1 = cache.read("us", "certs", "test-cert-hash").await?;
    assert_eq!(data1, test_data);

    // Check validity immediately - should be valid
    assert!(cache.is_valid("us", "certs", "test-cert-hash").await);

    // Second read should be fast (from cache)
    let start = std::time::Instant::now();
    let data2 = cache.read("us", "certs", "test-cert-hash").await?;
    let duration = start.elapsed();

    // File system reads should be very fast
    assert!(
        duration.as_millis() < 10,
        "Cache read should be very fast, but took {duration:?}"
    );
    assert_eq!(data2, test_data);

    unsafe {
        std::env::remove_var("XDG_CACHE_HOME");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_generic_cache_performance() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    // SAFETY: Test runs in isolation. Setting XDG_CACHE_HOME for test environment is safe.
    unsafe {
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
    }

    let cache = GenericCache::new().await?;

    let test_data = b"test data for performance";
    let key = "perf-test-key";

    // Write data
    cache.write(key, test_data).await?;

    // Time multiple reads
    let mut total_time = std::time::Duration::ZERO;
    for _ in 0..5 {
        let start = std::time::Instant::now();
        let data = cache.read(key).await?;
        total_time += start.elapsed();
        assert_eq!(data, test_data);
    }

    let avg_time = total_time / 5;
    assert!(
        avg_time.as_millis() < 5,
        "Average cache read should be under 5ms, but was {avg_time:?}"
    );

    unsafe {
        std::env::remove_var("XDG_CACHE_HOME");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn test_cache_file_structure() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    // SAFETY: Test runs in isolation. Setting XDG_CACHE_HOME for test environment is safe.
    unsafe {
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
    }

    // Test RibbitCache structure
    let ribbit_cache = RibbitCache::new().await?;
    ribbit_cache
        .write("us", "wow", "versions", b"version data")
        .await?;
    ribbit_cache.write("eu", "wow", "cdns", b"cdn data").await?;

    // Verify directory structure
    let cache_base = temp_dir.path().join("ngdp/ribbit");
    let us_dir = cache_base.join("us");
    let eu_dir = cache_base.join("eu");

    assert!(us_dir.exists(), "US region directory should exist");
    assert!(eu_dir.exists(), "EU region directory should exist");

    // Verify files exist (the exact file structure depends on implementation)
    let us_versions = us_dir.join("wow").join("versions");
    let eu_cdns = eu_dir.join("wow").join("cdns");

    assert!(us_versions.exists(), "US versions file should exist");
    assert!(eu_cdns.exists(), "EU cdns file should exist");

    unsafe {
        std::env::remove_var("XDG_CACHE_HOME");
    }
    Ok(())
}
