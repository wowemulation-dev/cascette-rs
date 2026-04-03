//! Battle.net directory layout generation.
//!
//! Creates the directory structure and metadata files that Battle.net agent
//! expects: `.build.info`, `.product.db`, `Data/config/` files, and
//! optionally `.flavor.info` in the product subfolder.

pub mod build_info;
pub mod config_store;
pub mod flavor_info;
pub mod product_db;

use tracing::info;

use crate::config::InstallConfig;
use crate::error::InstallationResult;
use crate::pipeline::manifests::BuildManifests;

/// Write the full Battle.net directory layout.
///
/// Creates:
/// - `.build.info` (BPSV format)
/// - `.product.db` (protobuf-like binary)
/// - `Data/config/{hash_path}` for build and CDN config files
/// - `.flavor.info` in product subfolder (if `game_subfolder` is set)
pub async fn write_layout(
    config: &InstallConfig,
    manifests: &BuildManifests,
) -> InstallationResult<()> {
    info!("writing Battle.net directory layout");

    // Write .build.info
    build_info::write_build_info(config, manifests).await?;

    // Write .product.db
    product_db::write_product_db(config, manifests).await?;

    // Write config files to Data/config/
    config_store::write_config_files(config, manifests).await?;

    // Write .flavor.info to product subfolder
    if let Some(ref subfolder) = config.game_subfolder {
        flavor_info::write_flavor_info(&config.install_path, subfolder, &config.product).await?;
    }

    Ok(())
}
