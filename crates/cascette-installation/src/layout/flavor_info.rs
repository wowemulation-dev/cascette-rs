//! `.flavor.info` writer for product subfolders.
//!
//! Contains the product code string (e.g., "wow_classic"). The Blizzard Agent
//! writes this via `GameSubfolder::WriteFlavorAndMerge`.

use std::path::Path;

use crate::error::InstallationResult;

/// Write `.flavor.info` to the product subfolder.
pub async fn write_flavor_info(
    install_path: &Path,
    subfolder: &str,
    product_code: &str,
) -> InstallationResult<()> {
    let dir = install_path.join(subfolder);
    tokio::fs::create_dir_all(&dir).await?;
    tokio::fs::write(dir.join(".flavor.info"), product_code).await?;
    Ok(())
}
