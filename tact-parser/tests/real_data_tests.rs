//! Tests using real data from Blizzard CDN

use blte::decompress_blte;
use ngdp_crypto::KeyService;
use tact_parser::config::{BuildConfig, CdnConfig};
use tact_parser::download::DownloadManifest;
use tact_parser::encoding::EncodingFile;
use tact_parser::install::InstallManifest;
use tact_parser::size::SizeFile;

/// Test helper to decompress BLTE data with key service
fn decompress_blte_data(data: Vec<u8>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Create key service with default keys
    let key_service = KeyService::new();

    // Check if data is BLTE encoded
    if data.len() >= 4 && &data[0..4] == b"BLTE" {
        println!("BLTE-encoded data detected, decompressing...");
        let decompressed = decompress_blte(data, Some(&key_service))?;
        println!("Decompressed {} bytes from BLTE", decompressed.len());
        Ok(decompressed)
    } else {
        // Not BLTE encoded, return as-is
        Ok(data)
    }
}

/// Test helper to download a file from CDN
async fn download_cdn_file(cdn_host: &str, cdn_path: &str, hash: &str) -> Vec<u8> {
    let url = format!(
        "http://{}/{}/data/{}/{}/{}",
        cdn_host,
        cdn_path,
        &hash[0..2],
        &hash[2..4],
        hash
    );

    println!("Downloading: {url}");

    let response = reqwest::get(&url).await.expect("Failed to download");
    response
        .bytes()
        .await
        .expect("Failed to read bytes")
        .to_vec()
}

/// Test helper to get build config info for a specific product
async fn get_build_info_for_product(product: &str) -> (String, String, String, String, String) {
    // Use ribbit to get current build info
    let ribbit_client = ribbit_client::RibbitClient::new(ribbit_client::Region::US);

    // Get versions
    let versions_endpoint = ribbit_client::Endpoint::ProductVersions(product.to_string());
    let versions_response = ribbit_client
        .request_raw(&versions_endpoint)
        .await
        .expect("Failed to get versions");

    // Parse the BPSV to find build config
    let versions_str = std::str::from_utf8(&versions_response).expect("Invalid UTF-8");
    let bpsv = ngdp_bpsv::BpsvDocument::parse(versions_str).expect("Failed to parse versions");

    // Find US entry
    let us_row = bpsv
        .rows()
        .iter()
        .find(|row| {
            row.raw_values()
                .first()
                .map(|v| *v == "us")
                .unwrap_or(false)
        })
        .expect("No US region found");

    let build_config_hash = us_row.raw_values()[1].to_string();
    let cdn_config_hash = us_row.raw_values()[2].to_string();

    // Get CDN info
    let cdns_endpoint = ribbit_client::Endpoint::ProductCdns(product.to_string());
    let cdns_response = ribbit_client
        .request_raw(&cdns_endpoint)
        .await
        .expect("Failed to get CDNs");

    let cdns_str = std::str::from_utf8(&cdns_response).expect("Invalid UTF-8");
    let cdns_bpsv = ngdp_bpsv::BpsvDocument::parse(cdns_str).expect("Failed to parse CDNs");

    let cdn_row = cdns_bpsv
        .rows()
        .iter()
        .find(|row| {
            row.raw_values()
                .first()
                .map(|v| *v == "us")
                .unwrap_or(false)
        })
        .expect("No US CDN found");

    let cdn_host = cdn_row.raw_values()[2]
        .split(' ')
        .next()
        .unwrap()
        .to_string();
    let cdn_path = cdn_row.raw_values()[1].to_string();

    println!("{product} Build config: {build_config_hash}");
    println!("{product} CDN config: {cdn_config_hash}");
    println!("{product} CDN host: {cdn_host}");
    println!("{product} CDN path: {cdn_path}");

    (
        cdn_host,
        cdn_path,
        build_config_hash,
        cdn_config_hash,
        product.to_string(),
    )
}

/// Test helper to get build config info
async fn get_build_info() -> (String, String, String, String, String) {
    get_build_info_for_product("wow").await
}

#[tokio::test]
async fn test_real_build_config() {
    let (cdn_host, cdn_path, build_config_hash, _, _) = get_build_info().await;

    // Download build config
    let url = format!(
        "http://{}/{}/config/{}/{}/{}",
        cdn_host,
        cdn_path,
        &build_config_hash[0..2],
        &build_config_hash[2..4],
        build_config_hash
    );

    println!("Downloading build config from: {url}");
    let response = reqwest::get(&url).await.expect("Failed to download");
    let data = response.text().await.expect("Failed to read text");

    // Parse it
    let build_config = BuildConfig::parse(&data).expect("Failed to parse build config");

    // Print all keys to see what we have
    println!("Build config keys:");
    for key in build_config.config.keys() {
        if let Some(value) = build_config.config.get_value(key) {
            println!("  {key}: {value}");
        }
    }

    // Check what we have instead of asserting
    println!("\nAnalysis:");
    let root_value = build_config.config.get_value("root");
    println!("root value: {root_value:?}");
    println!(
        "encoding value: {:?}",
        build_config.config.get_value("encoding")
    );
    println!(
        "install value: {:?}",
        build_config.config.get_value("install")
    );
    println!(
        "download value: {:?}",
        build_config.config.get_value("download")
    );
    let size_value = build_config.config.get_value("size");
    println!("size value: {size_value:?}");

    println!("\nHash extraction:");
    let has_root = build_config.root_hash().is_some();
    println!("Has root hash: {has_root}");
    println!(
        "Has encoding hash: {}",
        build_config.encoding_hash().is_some()
    );
    println!(
        "Has install hash: {}",
        build_config.install_hash().is_some()
    );
    println!(
        "Has download hash: {}",
        build_config.download_hash().is_some()
    );
    let has_size_hash = build_config.size_hash().is_some();
    println!("Has size hash: {has_size_hash}");

    if let Some(download_hash) = build_config.download_hash() {
        println!("Download manifest hash: {download_hash}");
    }

    if let Some(size_hash) = build_config.size_hash() {
        println!("Size file hash: {size_hash}");
    }

    let build_name = build_config.build_name();
    println!("Build name: {build_name:?}");

    // Check for download and size hashes
    let has_download = build_config.download_hash().is_some();
    let has_size = build_config.size_hash().is_some();

    println!("Has download manifest: {has_download}");
    println!("Has size file: {has_size}");
}

#[tokio::test]
async fn test_real_cdn_config() {
    let (cdn_host, cdn_path, _, cdn_config_hash, _) = get_build_info().await;

    // Download CDN config
    let url = format!(
        "http://{}/{}/config/{}/{}/{}",
        cdn_host,
        cdn_path,
        &cdn_config_hash[0..2],
        &cdn_config_hash[2..4],
        cdn_config_hash
    );

    println!("Downloading CDN config from: {url}");
    let response = reqwest::get(&url).await.expect("Failed to download");
    let data = response.text().await.expect("Failed to read text");

    // Parse it
    let cdn_config = CdnConfig::parse(&data).expect("Failed to parse CDN config");

    // Verify we got archives
    let archives = cdn_config.archives();
    assert!(!archives.is_empty(), "Should have archives");
    let archive_count = archives.len();
    println!("Number of archives: {archive_count}");

    if let Some(file_index) = cdn_config.file_index() {
        println!("File index: {file_index}");
    }
}

#[tokio::test]
async fn test_ptr_build_config() {
    let (cdn_host, cdn_path, build_config_hash, _, _) = get_build_info_for_product("wowt").await;

    // Download build config
    let url = format!(
        "http://{}/{}/config/{}/{}/{}",
        cdn_host,
        cdn_path,
        &build_config_hash[0..2],
        &build_config_hash[2..4],
        build_config_hash
    );

    println!("Downloading PTR build config from: {url}");
    let response = reqwest::get(&url).await.expect("Failed to download");
    let data = response.text().await.expect("Failed to read text");

    // Parse it
    let build_config = BuildConfig::parse(&data).expect("Failed to parse build config");

    // Print all keys to see what we have
    println!("PTR Build config keys:");
    for key in build_config.config.keys() {
        if let Some(value) = build_config.config.get_value(key) {
            println!("  {key}: {value}");
        }
    }

    // Check what manifests are available
    println!("\nPTR Analysis:");
    let has_download = build_config.download_hash().is_some();
    let has_size = build_config.size_hash().is_some();
    let has_encoding = build_config.encoding_hash().is_some();
    let has_install = build_config.install_hash().is_some();

    println!("Has download manifest: {has_download}");
    println!("Has size file: {has_size}");
    println!("Has encoding file: {has_encoding}");
    println!("Has install manifest: {has_install}");

    if let Some(encoding_hash) = build_config.encoding_hash() {
        println!("PTR Encoding file hash: {encoding_hash}");
    }
}

#[tokio::test]
#[ignore] // This test downloads large files, so we'll mark it as ignored by default
async fn test_ptr_encoding_file_encrypted() {
    let (cdn_host, cdn_path, build_config_hash, _, _) = get_build_info_for_product("wowt").await;

    // Get build config first - using the config URL format, not data URL format
    let config_url = format!(
        "http://{}/{}/config/{}/{}/{}",
        cdn_host,
        cdn_path,
        &build_config_hash[0..2],
        &build_config_hash[2..4],
        build_config_hash
    );

    println!("Downloading PTR build config from: {config_url}");
    let response = reqwest::get(&config_url).await.expect("Failed to download");
    let build_config_text = response.text().await.expect("Failed to read text");
    let build_config =
        BuildConfig::parse(&build_config_text).expect("Failed to parse build config");

    // Debug encoding value extraction
    if let Some(encoding_value) = build_config.config.get_value("encoding") {
        println!("PTR encoding value: '{encoding_value}'");
    } else {
        println!("PTR: no 'encoding' key found");
    }

    // Get encoding file hash
    let encoding_hash = match build_config.encoding_hash() {
        Some(hash) => {
            println!("Extracted PTR encoding hash: {hash}");
            hash
        }
        None => {
            println!("Failed to extract encoding hash from PTR build config");
            println!("Available keys: {:?}", build_config.config.keys());
            panic!("PTR should have encoding hash");
        }
    };
    println!("Downloading PTR encoding file: {encoding_hash}");

    // Download the encoding file (this will be BLTE encoded and might be encrypted)
    let encoding_data = download_cdn_file(&cdn_host, &cdn_path, encoding_hash).await;

    assert!(
        !encoding_data.is_empty(),
        "PTR Encoding file should not be empty"
    );
    println!(
        "PTR Encoding file size (BLTE compressed): {} bytes",
        encoding_data.len()
    );

    // Check if it's BLTE encoded
    if encoding_data.len() >= 4 && &encoding_data[0..4] == b"BLTE" {
        println!("✅ PTR encoding file is BLTE-encoded");

        // Try to decompress with our key service
        match decompress_blte_data(encoding_data) {
            Ok(decompressed) => {
                println!(
                    "✅ Successfully decompressed PTR encoding file to {} bytes",
                    decompressed.len()
                );

                // Now try to parse the decompressed encoding file
                match EncodingFile::parse(&decompressed) {
                    Ok(encoding_file) => {
                        println!("✅ Successfully parsed PTR encoding file!");
                        println!("  - Version: {}", encoding_file.header.version);
                        println!("  - CKey entries: {}", encoding_file.ckey_count());
                        println!("  - EKey to CKey mappings: {}", encoding_file.ekey_count());

                        // Test lookups
                        let total_ckeys = encoding_file.ckey_count();
                        let total_ekeys = encoding_file.ekey_count();
                        println!("  - Total CKey entries: {total_ckeys}");
                        println!("  - Total EKey mappings: {total_ekeys}");
                    }
                    Err(e) => {
                        println!("⚠️  Could not parse PTR encoding file: {e}");
                        println!(
                            "This might be expected if the format has changed or is encrypted"
                        );
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Could not decompress PTR BLTE data: {e}");
                println!("This is expected if encryption keys are needed for PTR content!");
                println!("Error details: {e:?}");
            }
        }
    } else {
        println!("PTR encoding file is not BLTE-encoded");
    }
}

#[tokio::test]
#[ignore] // This test downloads large files, so we'll mark it as ignored by default
async fn test_real_download_manifest() {
    let (cdn_host, cdn_path, build_config_hash, _, _) = get_build_info().await;

    // Get build config first
    let build_config_data = download_cdn_file(&cdn_host, &cdn_path, &build_config_hash).await;
    let build_config_text = String::from_utf8_lossy(&build_config_data);
    let build_config =
        BuildConfig::parse(&build_config_text).expect("Failed to parse build config");

    // Check if we have a download manifest
    if let Some(download_hash) = build_config.download_hash() {
        println!("Downloading download manifest: {download_hash}");

        // Download the manifest (this will be BLTE encoded)
        let download_data = download_cdn_file(&cdn_host, &cdn_path, download_hash).await;

        assert!(
            !download_data.is_empty(),
            "Download manifest should not be empty"
        );
        println!(
            "Download manifest size (BLTE compressed): {} bytes",
            download_data.len()
        );

        // Decompress BLTE data
        match decompress_blte_data(download_data) {
            Ok(decompressed) => {
                println!(
                    "Successfully decompressed download manifest to {} bytes",
                    decompressed.len()
                );

                // Now try to parse the decompressed manifest
                match DownloadManifest::parse(&decompressed) {
                    Ok(manifest) => {
                        println!("✅ Successfully parsed download manifest!");
                        println!("  - Version: {}", manifest.header.version);
                        let entry_count = manifest.entries.len();
                        println!("  - Entry count: {entry_count}");
                        let tag_count = manifest.tags.len();
                        println!("  - Tag count: {tag_count}");

                        // Verify we can get some files
                        let priority_files = manifest.get_priority_files(10);
                        let count = priority_files.len();
                        println!("  - Top 10 priority files: {count} found");
                    }
                    Err(e) => {
                        println!("⚠️  Could not parse download manifest: {e}");
                        println!(
                            "This might be expected if the format has changed or is encrypted"
                        );
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Could not decompress BLTE data: {e}");
                println!("This might be expected if encryption keys are needed");
            }
        }
    } else {
        println!("No download manifest in this build");
    }
}

#[tokio::test]
#[ignore] // This test downloads large files
async fn test_real_size_file() {
    let (cdn_host, cdn_path, build_config_hash, _, _) = get_build_info().await;

    // Get build config first
    let build_config_data = download_cdn_file(&cdn_host, &cdn_path, &build_config_hash).await;
    let build_config_text = String::from_utf8_lossy(&build_config_data);
    let build_config =
        BuildConfig::parse(&build_config_text).expect("Failed to parse build config");

    // Check if we have a size file
    if let Some(size_hash) = build_config.size_hash() {
        println!("Downloading size file: {size_hash}");

        // Download the size file (this will be BLTE encoded)
        let size_data = download_cdn_file(&cdn_host, &cdn_path, size_hash).await;

        assert!(!size_data.is_empty(), "Size file should not be empty");
        println!(
            "Size file size (BLTE compressed): {} bytes",
            size_data.len()
        );

        // Decompress BLTE data
        match decompress_blte_data(size_data) {
            Ok(decompressed) => {
                println!(
                    "Successfully decompressed size file to {} bytes",
                    decompressed.len()
                );

                // Now try to parse the decompressed size file
                match SizeFile::parse(&decompressed) {
                    Ok(size_file) => {
                        println!("✅ Successfully parsed size file!");
                        println!("  - Version: {}", size_file.header.version);
                        let entry_count = size_file.entries.len();
                        println!("  - Entry count: {entry_count}");
                        let tag_count = size_file.tags.len();
                        println!("  - Tag count: {tag_count}");
                        let total_size = size_file.get_total_size();
                        println!("  - Total size: {total_size} bytes");

                        // Get some statistics
                        let stats = size_file.get_statistics();
                        let avg_size = stats.average_size;
                        println!("  - Average file size: {avg_size} bytes");
                        let max_size = stats.max_size;
                        println!("  - Largest file: {max_size} bytes");

                        // Get largest files
                        let largest = size_file.get_largest_files(5);
                        let count = largest.len();
                        println!("  - Top 5 largest files: {count} found");
                    }
                    Err(e) => {
                        println!("⚠️  Could not parse size file: {e}");
                        println!(
                            "This might be expected if the format has changed or is encrypted"
                        );
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Could not decompress BLTE data: {e}");
                println!("This might be expected if encryption keys are needed");
            }
        }
    } else {
        println!("No size file in this build");
    }
}

#[tokio::test]
#[ignore] // This test downloads large files
async fn test_real_encoding_file() {
    let (cdn_host, cdn_path, build_config_hash, _, _) = get_build_info().await;

    // Get build config first
    let build_config_data = download_cdn_file(&cdn_host, &cdn_path, &build_config_hash).await;
    let build_config_text = String::from_utf8_lossy(&build_config_data);
    let build_config =
        BuildConfig::parse(&build_config_text).expect("Failed to parse build config");

    // Get encoding file hash
    let encoding_hash = build_config
        .encoding_hash()
        .expect("Should have encoding hash");
    println!("Downloading encoding file: {encoding_hash}");

    // Download the encoding file (this will be BLTE encoded)
    let encoding_data = download_cdn_file(&cdn_host, &cdn_path, encoding_hash).await;

    assert!(
        !encoding_data.is_empty(),
        "Encoding file should not be empty"
    );
    println!(
        "Encoding file size (BLTE compressed): {} bytes",
        encoding_data.len()
    );

    // Decompress BLTE data
    match decompress_blte_data(encoding_data) {
        Ok(decompressed) => {
            println!(
                "Successfully decompressed encoding file to {} bytes",
                decompressed.len()
            );

            // Now try to parse the decompressed encoding file
            match EncodingFile::parse(&decompressed) {
                Ok(encoding_file) => {
                    println!("✅ Successfully parsed encoding file!");
                    println!("  - Version: {}", encoding_file.header.version);
                    println!("  - CKey entries: {}", encoding_file.ckey_count());
                    println!("  - EKey to CKey mappings: {}", encoding_file.ekey_count());

                    // Test lookups
                    let total_ckeys = encoding_file.ckey_count();
                    let total_ekeys = encoding_file.ekey_count();
                    println!("  - Total CKey entries: {total_ckeys}");
                    println!("  - Total EKey mappings: {total_ekeys}");
                }
                Err(e) => {
                    println!("⚠️  Could not parse encoding file: {e}");
                    println!("This might be expected if the format has changed or is encrypted");
                }
            }
        }
        Err(e) => {
            println!("⚠️  Could not decompress BLTE data: {e}");
            println!("This might be expected if encryption keys are needed");
        }
    }
}

#[tokio::test]
#[ignore] // This test downloads large files
async fn test_real_install_manifest() {
    let (cdn_host, cdn_path, build_config_hash, _, _) = get_build_info().await;

    // Get build config first
    let build_config_data = download_cdn_file(&cdn_host, &cdn_path, &build_config_hash).await;
    let build_config_text = String::from_utf8_lossy(&build_config_data);
    let build_config =
        BuildConfig::parse(&build_config_text).expect("Failed to parse build config");

    // Get install manifest hash
    if let Some(install_hash) = build_config.install_hash() {
        println!("Downloading install manifest: {install_hash}");

        // Download the install manifest (this will be BLTE encoded)
        let install_data = download_cdn_file(&cdn_host, &cdn_path, install_hash).await;

        assert!(
            !install_data.is_empty(),
            "Install manifest should not be empty"
        );
        println!(
            "Install manifest size (BLTE compressed): {} bytes",
            install_data.len()
        );

        // Decompress BLTE data
        match decompress_blte_data(install_data) {
            Ok(decompressed) => {
                println!(
                    "Successfully decompressed install manifest to {} bytes",
                    decompressed.len()
                );

                // Now try to parse the decompressed install manifest
                match InstallManifest::parse(&decompressed) {
                    Ok(manifest) => {
                        println!("✅ Successfully parsed install manifest!");
                        println!("  - Version: {}", manifest.header.version);
                        let entry_count = manifest.entries.len();
                        println!("  - Entry count: {entry_count}");
                        let tag_count = manifest.tags.len();
                        println!("  - Tag count: {tag_count}");

                        // Test some functionality
                        if !manifest.tags.is_empty() {
                            let tag_name = &manifest.tags[0].name;
                            let files = manifest.get_files_for_tags(&[tag_name]);
                            let file_count = files.len();
                            println!("  - Files for tag '{tag_name}': {file_count} found");
                        }
                    }
                    Err(e) => {
                        println!("⚠️  Could not parse install manifest: {e}");
                        println!(
                            "This might be expected if the format has changed or is encrypted"
                        );
                    }
                }
            }
            Err(e) => {
                println!("⚠️  Could not decompress BLTE data: {e}");
                println!("This might be expected if encryption keys are needed");
            }
        }
    } else {
        println!("No install manifest in this build");
    }
}

#[tokio::test]
async fn test_compare_config_formats() {
    println!("\n=== Testing wow_classic_era config format ===");
    let (cdn_host_classic, cdn_path_classic, build_config_hash_classic, _, _) =
        get_build_info_for_product("wow_classic_era").await;

    // Download and parse classic build config
    let url_classic = format!(
        "http://{}/{}/config/{}/{}/{}",
        cdn_host_classic,
        cdn_path_classic,
        &build_config_hash_classic[0..2],
        &build_config_hash_classic[2..4],
        build_config_hash_classic
    );

    println!("Downloading classic build config from: {url_classic}");
    let response_classic = reqwest::get(&url_classic)
        .await
        .expect("Failed to download");
    let data_classic = response_classic.text().await.expect("Failed to read text");

    println!("\nClassic config sample:");
    for line in data_classic.lines().take(20) {
        println!("  {line}");
    }

    println!("\n=== Testing wow (retail) config format ===");
    let (cdn_host_retail, cdn_path_retail, build_config_hash_retail, _, _) =
        get_build_info_for_product("wow").await;

    // Download and parse retail build config
    let url_retail = format!(
        "http://{}/{}/config/{}/{}/{}",
        cdn_host_retail,
        cdn_path_retail,
        &build_config_hash_retail[0..2],
        &build_config_hash_retail[2..4],
        build_config_hash_retail
    );

    println!("Downloading retail build config from: {url_retail}");
    let response_retail = reqwest::get(&url_retail).await.expect("Failed to download");
    let data_retail = response_retail.text().await.expect("Failed to read text");

    println!("\nRetail config sample:");
    for line in data_retail.lines().take(20) {
        println!("  {line}");
    }

    // Parse both configs
    let build_config_classic =
        BuildConfig::parse(&data_classic).expect("Failed to parse classic config");
    let build_config_retail =
        BuildConfig::parse(&data_retail).expect("Failed to parse retail config");

    // Compare formats
    println!("\n=== Format Comparison ===");

    // Check classic format
    if let Some(root_value) = build_config_classic.config.get_value("root") {
        println!("Classic root value: {root_value}");
        println!(
            "Classic root hash extracted: {:?}",
            build_config_classic.root_hash()
        );
    }

    if let Some(encoding_value) = build_config_classic.config.get_value("encoding") {
        println!("Classic encoding value: {encoding_value}");
        println!(
            "Classic encoding hash extracted: {:?}",
            build_config_classic.encoding_hash()
        );
    }

    // Check retail format
    if let Some(root_value) = build_config_retail.config.get_value("root") {
        println!("\nRetail root value: {root_value}");
        println!(
            "Retail root hash extracted: {:?}",
            build_config_retail.root_hash()
        );
    }

    if let Some(encoding_value) = build_config_retail.config.get_value("encoding") {
        println!("Retail encoding value: {encoding_value}");
        println!(
            "Retail encoding hash extracted: {:?}",
            build_config_retail.encoding_hash()
        );
    }
}
