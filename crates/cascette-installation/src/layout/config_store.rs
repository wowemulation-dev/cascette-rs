//! Config file placement into `Data/config/`.
//!
//! Build config and CDN config files are stored in the CASC directory
//! structure using their hash as the path:
//! `Data/config/{hash[0..2]}/{hash[2..4]}/{hash}`

use crate::config::InstallConfig;
use crate::error::InstallationResult;
use crate::pipeline::manifests::BuildManifests;

/// Write build config and CDN config files to `Data/config/`.
///
/// Each config is stored at `Data/config/{hash[0..2]}/{hash[2..4]}/{hash}`
/// matching the CDN URL structure.
pub async fn write_config_files(
    config: &InstallConfig,
    manifests: &BuildManifests,
) -> InstallationResult<()> {
    let config_dir = config.install_path.join("Data").join("config");

    // Write build config
    if let Some(ref hash) = config.build_config {
        let data = manifests.build_config.build();
        write_config_file(&config_dir, hash, &data).await?;
    }

    // Write CDN config
    if let Some(ref hash) = config.cdn_config {
        let data = manifests.cdn_config.build();
        write_config_file(&config_dir, hash, &data).await?;
    }

    // Write patch config (from the build config's `patch-config` field).
    // The client reads the patch config from the local cache to determine
    // if an update is available. Without it, the client contacts the CDN
    // to fetch it and discovers newer builds, showing an update dialog.
    if let Some(hash) = manifests.build_config.patch_config() {
        let hash = hash.to_string();
        // Check if already cached
        let patch_path = config_dir.join(&hash[..2]).join(&hash[2..4]).join(&hash);
        if !patch_path.exists() {
            // Download from CDN and cache
            if let Some(ref raw_data) = manifests.patch_config_data {
                write_config_file(&config_dir, &hash, raw_data).await?;
            }
        }
    }

    Ok(())
}

/// Write a single config file using CDN hash path structure.
async fn write_config_file(
    config_dir: &std::path::Path,
    hash: &str,
    data: &[u8],
) -> InstallationResult<()> {
    if hash.len() < 4 {
        return Err(crate::error::InstallationError::InvalidConfig(format!(
            "config hash too short: {hash}"
        )));
    }

    let dir = config_dir.join(&hash[..2]).join(&hash[2..4]);
    tokio::fs::create_dir_all(&dir).await?;

    let file_path = dir.join(hash);
    tokio::fs::write(&file_path, data).await?;

    Ok(())
}
