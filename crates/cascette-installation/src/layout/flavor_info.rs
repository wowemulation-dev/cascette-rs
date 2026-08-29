//! `.flavor.info` writer for product subfolders.
//!
//! Written as a
//! single-column BPSV (pipe-separated values) table:
//!
//! ```text
//! Product Flavor!STRING:0
//! wow_classic
//! ```

use std::path::Path;

use crate::error::InstallationResult;

/// Write `.flavor.info` to the product subfolder.
///
/// Uses BPSV format matching Blizzard Agent output.
pub async fn write_flavor_info(
    install_path: &Path,
    subfolder: &str,
    product_code: &str,
) -> InstallationResult<()> {
    let dir = install_path.join(subfolder);
    tokio::fs::create_dir_all(&dir).await?;
    let content = format!("Product Flavor!STRING:0\n{product_code}\n");
    tokio::fs::write(dir.join(".flavor.info"), content).await?;
    Ok(())
}
