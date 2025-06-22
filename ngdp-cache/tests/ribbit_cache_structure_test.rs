//! Test to verify correct cache directory structure
//!
//! This test demonstrates that CachedRibbitClient uses the correct
//! cache directory structure: ~/.cache/ngdp/ribbit/{region}/

use ngdp_cache::ribbit::RibbitCache;
use tempfile::TempDir;

#[tokio::test]
async fn test_ribbit_cache_directory_structure() -> Result<(), Box<dyn std::error::Error>> {
    // Use a temporary directory for testing
    let temp_dir = TempDir::new()?;
    unsafe {
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
    }

    // Create RibbitCache and write test data for different regions
    let cache = RibbitCache::new().await?;

    // Write test data for different regions
    let regions = vec!["us", "eu", "kr", "cn"];
    for region in &regions {
        cache.write(region, "summary", "test", b"test data").await?;
    }

    // Verify the expected cache directory structure
    let cache_base = temp_dir.path().join("ngdp").join("ribbit");
    assert!(cache_base.exists(), "Cache base directory should exist");

    // Check that region directories exist
    for region in &regions {
        let region_dir = cache_base.join(region);
        assert!(
            region_dir.exists(),
            "{} region directory should exist",
            region.to_uppercase()
        );
    }

    // Verify no incorrect "cached" subdirectory exists
    let incorrect_path = cache_base.join("cached");
    assert!(
        !incorrect_path.exists(),
        "Incorrect 'cached' subdirectory should not exist"
    );

    unsafe {
        std::env::remove_var("XDG_CACHE_HOME");
    }

    Ok(())
}

#[tokio::test]
async fn test_ribbit_cache_file_naming() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = TempDir::new()?;
    unsafe {
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());
    }

    let cache = RibbitCache::new().await?;

    // Test different endpoint types generate correct filenames
    let test_cases = vec![
        ("summary", "test", "summary data"),
        ("versions", "wow", "version data"),
        ("cdns", "wow", "cdn data"),
        (
            "certs",
            "5168ff90af0207753cccd9656462a212b859723b",
            "cert data",
        ),
    ];

    for (endpoint, product, data) in &test_cases {
        cache
            .write("us", endpoint, product, data.as_bytes())
            .await?;
    }

    // Verify files exist with expected names
    let cache_dir = temp_dir.path().join("ngdp").join("ribbit").join("us");

    // Check that the expected directories/files exist
    assert!(
        cache_dir.join("summary").join("test").exists(),
        "Summary cache file should exist"
    );
    assert!(
        cache_dir.join("versions").join("wow").exists(),
        "Versions cache file should exist"
    );
    assert!(
        cache_dir.join("cdns").join("wow").exists(),
        "CDNs cache file should exist"
    );
    assert!(
        cache_dir
            .join("certs")
            .join("5168ff90af0207753cccd9656462a212b859723b")
            .exists(),
        "Certificate cache file should exist"
    );

    unsafe {
        std::env::remove_var("XDG_CACHE_HOME");
    }
    Ok(())
}
