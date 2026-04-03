//! Product registry: CRUD operations for product state.

use std::collections::HashMap;

use chrono::Utc;
use tracing::debug;

use crate::error::{AgentError, AgentResult};
use crate::models::product::{Product, ProductStatus};
use crate::server::router::ProductDownloadConfig;

use super::db::Database;

/// Product registry backed by SQLite.
pub struct ProductRegistry {
    db: Database,
}

impl ProductRegistry {
    /// Create a new registry using the given database.
    #[must_use]
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Get all registered products.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn list(&self) -> AgentResult<Vec<Product>> {
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT product_code, name, status, version, install_path,
                        size_bytes, region, locale, installation_mode,
                        is_update_available, available_version,
                        patch_url, protocol, build_config, cdn_config,
                        subfolder, patch_region_hint, created_at, updated_at
                 FROM products ORDER BY product_code",
                (),
            )
            .await?;

        let mut products = Vec::new();
        while let Some(row) = rows.next().await? {
            products.push(row_to_product(&row)?);
        }
        Ok(products)
    }

    /// Get a single product by code.
    ///
    /// UID comparison is case-insensitive. The code is normalized to ASCII
    /// lowercase before querying.
    ///
    /// # Errors
    ///
    /// Returns `ProductNotFound` if the product does not exist.
    pub async fn get(&self, product_code: &str) -> AgentResult<Product> {
        let normalized = product_code.to_ascii_lowercase();
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT product_code, name, status, version, install_path,
                        size_bytes, region, locale, installation_mode,
                        is_update_available, available_version,
                        patch_url, protocol, build_config, cdn_config,
                        subfolder, patch_region_hint, created_at, updated_at
                 FROM products WHERE product_code = ?1",
                turso::params![normalized.clone()],
            )
            .await?;

        match rows.next().await? {
            Some(row) => row_to_product(&row),
            None => Err(AgentError::ProductNotFound(normalized)),
        }
    }

    /// Insert a new product.
    ///
    /// The product code is normalized to ASCII lowercase before storage,
    /// matching Agent.exe's case-insensitive UID handling.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn insert(&self, product: &Product) -> AgentResult<()> {
        let now = Utc::now().to_rfc3339();
        let normalized_code = product.product_code.to_ascii_lowercase();
        self.db
            .conn()
            .execute(
                "INSERT INTO products (product_code, name, status, version, install_path,
                                       size_bytes, region, locale, installation_mode,
                                       is_update_available, available_version,
                                       patch_url, protocol, build_config, cdn_config,
                                       subfolder, patch_region_hint,
                                       created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                turso::params![
                    normalized_code,
                    product.name.clone(),
                    product.status.to_string(),
                    product.version.clone(),
                    product.install_path.clone(),
                    product.size_bytes.map(u64::cast_signed),
                    product.region.clone(),
                    product.locale.clone(),
                    product.installation_mode.map(|m| m.to_string()),
                    i64::from(product.is_update_available),
                    product.available_version.clone(),
                    product.patch_url.clone(),
                    product.protocol.clone(),
                    product.build_config.clone(),
                    product.cdn_config.clone(),
                    product.subfolder.clone(),
                    product.patch_region_hint.clone(),
                    now.clone(),
                    now
                ],
            )
            .await?;

        debug!(product_code = %product.product_code, "product inserted");
        Ok(())
    }

    /// Update an existing product's status and fields.
    ///
    /// The product code is normalized to ASCII lowercase for the WHERE clause.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn update(&self, product: &Product) -> AgentResult<()> {
        let now = Utc::now().to_rfc3339();
        let normalized_code = product.product_code.to_ascii_lowercase();
        self.db
            .conn()
            .execute(
                "UPDATE products SET
                    name = ?1, status = ?2, version = ?3, install_path = ?4,
                    size_bytes = ?5, region = ?6, locale = ?7, installation_mode = ?8,
                    is_update_available = ?9, available_version = ?10,
                    patch_url = ?11, protocol = ?12,
                    build_config = ?13, cdn_config = ?14,
                    subfolder = ?15, patch_region_hint = ?16,
                    updated_at = ?17
                 WHERE product_code = ?18",
                turso::params![
                    product.name.clone(),
                    product.status.to_string(),
                    product.version.clone(),
                    product.install_path.clone(),
                    product.size_bytes.map(u64::cast_signed),
                    product.region.clone(),
                    product.locale.clone(),
                    product.installation_mode.map(|m| m.to_string()),
                    i64::from(product.is_update_available),
                    product.available_version.clone(),
                    product.patch_url.clone(),
                    product.protocol.clone(),
                    product.build_config.clone(),
                    product.cdn_config.clone(),
                    product.subfolder.clone(),
                    product.patch_region_hint.clone(),
                    now,
                    normalized_code
                ],
            )
            .await?;

        debug!(product_code = %product.product_code, status = %product.status, "product updated");
        Ok(())
    }

    /// Check if a product exists.
    ///
    /// Uses ASCII-lowercase normalization to match Agent.exe's
    /// case-insensitive UID comparison.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn exists(&self, product_code: &str) -> AgentResult<bool> {
        let normalized = product_code.to_ascii_lowercase();
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT 1 FROM products WHERE product_code = ?1",
                turso::params![normalized],
            )
            .await?;
        Ok(rows.next().await?.is_some())
    }

    /// Register a new product or return the existing one.
    ///
    /// Returns `true` if a new product was created, `false` if it already existed.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn register(&self, product: &Product) -> AgentResult<bool> {
        if self.exists(&product.product_code).await? {
            // Product already registered -- update fields that may have changed.
            self.update(product).await?;
            Ok(false)
        } else {
            self.insert(product).await?;
            Ok(true)
        }
    }

    /// Find products installed at the given path.
    ///
    /// Used for conflict detection during registration: two products
    /// cannot share the same install directory.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn find_by_install_path(&self, install_path: &str) -> AgentResult<Vec<Product>> {
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT product_code, name, status, version, install_path,
                        size_bytes, region, locale, installation_mode,
                        is_update_available, available_version,
                        patch_url, protocol, build_config, cdn_config,
                        subfolder, patch_region_hint, created_at, updated_at
                 FROM products WHERE install_path = ?1",
                turso::params![install_path],
            )
            .await?;

        let mut products = Vec::new();
        while let Some(row) = rows.next().await? {
            products.push(row_to_product(&row)?);
        }
        Ok(products)
    }

    /// Upsert per-product download configuration.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn set_download_config(
        &self,
        product_code: &str,
        config: &ProductDownloadConfig,
    ) -> AgentResult<()> {
        let normalized = product_code.to_ascii_lowercase();
        self.db
            .conn()
            .execute(
                "INSERT INTO product_download_config
                    (product_code, background_download, priority, download_limit, paused)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(product_code) DO UPDATE SET
                    background_download = excluded.background_download,
                    priority = excluded.priority,
                    download_limit = excluded.download_limit,
                    paused = excluded.paused",
                turso::params![
                    normalized,
                    i64::from(config.background_download),
                    i64::from(config.priority),
                    config.download_limit.cast_signed(),
                    i64::from(config.paused)
                ],
            )
            .await?;

        debug!(
            product_code,
            priority = config.priority,
            "download config updated"
        );
        Ok(())
    }

    /// Get per-product download configuration.
    ///
    /// Returns `None` if no configuration has been set for this product.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn get_download_config(
        &self,
        product_code: &str,
    ) -> AgentResult<Option<ProductDownloadConfig>> {
        let normalized = product_code.to_ascii_lowercase();
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT background_download, priority, download_limit, paused
                 FROM product_download_config WHERE product_code = ?1",
                turso::params![normalized],
            )
            .await?;

        match rows.next().await? {
            Some(row) => {
                let background_download: i64 = row.get(0)?;
                let priority: i64 = row.get(1)?;
                let download_limit: i64 = row.get(2)?;
                let paused: i64 = row.get(3)?;
                Ok(Some(ProductDownloadConfig {
                    background_download: background_download != 0,
                    priority: priority as u32,
                    download_limit: download_limit as u64,
                    paused: paused != 0,
                }))
            }
            None => Ok(None),
        }
    }

    /// Load all per-product download configurations.
    ///
    /// Used at startup to hydrate the in-memory HashMap.
    ///
    /// # Errors
    ///
    /// Returns an error on database failure.
    pub async fn list_download_configs(
        &self,
    ) -> AgentResult<HashMap<String, ProductDownloadConfig>> {
        let mut rows = self
            .db
            .conn()
            .query(
                "SELECT product_code, background_download, priority, download_limit, paused
                 FROM product_download_config",
                (),
            )
            .await?;

        let mut configs = HashMap::new();
        while let Some(row) = rows.next().await? {
            let product_code: String = row.get(0)?;
            let background_download: i64 = row.get(1)?;
            let priority: i64 = row.get(2)?;
            let download_limit: i64 = row.get(3)?;
            let paused: i64 = row.get(4)?;
            configs.insert(
                product_code,
                ProductDownloadConfig {
                    background_download: background_download != 0,
                    priority: priority as u32,
                    download_limit: download_limit as u64,
                    paused: paused != 0,
                },
            );
        }
        Ok(configs)
    }
}

fn row_to_product(row: &turso::Row) -> AgentResult<Product> {
    let product_code: String = row.get(0)?;
    let name: String = row.get(1)?;
    let status_str: String = row.get(2)?;
    let version: Option<String> = row.get(3)?;
    let install_path: Option<String> = row.get(4)?;
    let size_bytes: Option<i64> = row.get(5)?;
    let region: Option<String> = row.get(6)?;
    let locale: Option<String> = row.get(7)?;
    let installation_mode_str: Option<String> = row.get(8)?;
    let is_update_available: i64 = row.get::<i64>(9).unwrap_or(0);
    let available_version: Option<String> = row.get(10)?;
    let patch_url: Option<String> = row.get(11)?;
    let protocol: Option<String> = row.get(12)?;
    let build_config: Option<String> = row.get(13)?;
    let cdn_config: Option<String> = row.get(14)?;
    let subfolder: Option<String> = row.get(15)?;
    let patch_region_hint: Option<String> = row.get(16)?;
    let created_at_str: String = row.get(17)?;
    let updated_at_str: String = row.get(18)?;

    let status = parse_product_status(&status_str)?;
    let installation_mode = installation_mode_str
        .as_deref()
        .map(parse_installation_mode)
        .transpose()?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));
    let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
        .map_or_else(|_| Utc::now(), |dt| dt.with_timezone(&Utc));

    Ok(Product {
        product_code,
        name,
        status,
        version,
        install_path,
        size_bytes: size_bytes.map(|v| v as u64),
        region,
        locale,
        installation_mode,
        is_update_available: is_update_available != 0,
        available_version,
        patch_url,
        protocol,
        build_config,
        cdn_config,
        subfolder,
        patch_region_hint,
        created_at,
        updated_at,
    })
}

fn parse_product_status(s: &str) -> AgentResult<ProductStatus> {
    match s {
        "available" => Ok(ProductStatus::Available),
        "installing" => Ok(ProductStatus::Installing),
        "installed" => Ok(ProductStatus::Installed),
        "updating" => Ok(ProductStatus::Updating),
        "repairing" => Ok(ProductStatus::Repairing),
        "verifying" => Ok(ProductStatus::Verifying),
        "uninstalling" => Ok(ProductStatus::Uninstalling),
        "corrupted" => Ok(ProductStatus::Corrupted),
        _ => Err(AgentError::Schema(format!("unknown product status: {s}"))),
    }
}

fn parse_installation_mode(s: &str) -> AgentResult<crate::models::product::InstallationMode> {
    use crate::models::product::InstallationMode;
    match s {
        "casc" => Ok(InstallationMode::Casc),
        "containerless" => Ok(InstallationMode::Containerless),
        _ => Err(AgentError::Schema(format!(
            "unknown installation mode: {s}"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::product::Product;

    async fn test_registry() -> ProductRegistry {
        let db = Database::open_memory().await.unwrap();
        ProductRegistry::new(db)
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let registry = test_registry().await;
        let product = Product::new("wow".to_string(), "World of Warcraft".to_string());
        registry.insert(&product).await.unwrap();

        let fetched = registry.get("wow").await.unwrap();
        assert_eq!(fetched.product_code, "wow");
        assert_eq!(fetched.name, "World of Warcraft");
        assert_eq!(fetched.status, ProductStatus::Available);
    }

    #[tokio::test]
    async fn test_list_empty() {
        let registry = test_registry().await;
        let products = registry.list().await.unwrap();
        assert!(products.is_empty());
    }

    #[tokio::test]
    async fn test_list_multiple() {
        let registry = test_registry().await;
        registry
            .insert(&Product::new("wow".to_string(), "WoW".to_string()))
            .await
            .unwrap();
        registry
            .insert(&Product::new(
                "wow_classic".to_string(),
                "WoW Classic".to_string(),
            ))
            .await
            .unwrap();

        let products = registry.list().await.unwrap();
        assert_eq!(products.len(), 2);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let registry = test_registry().await;
        let result = registry.get("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_exists() {
        let registry = test_registry().await;
        assert!(!registry.exists("wow").await.unwrap());
        registry
            .insert(&Product::new("wow".to_string(), "WoW".to_string()))
            .await
            .unwrap();
        assert!(registry.exists("wow").await.unwrap());
    }

    #[tokio::test]
    async fn test_update() {
        let registry = test_registry().await;
        let mut product = Product::new("wow".to_string(), "WoW".to_string());
        registry.insert(&product).await.unwrap();

        product.status = ProductStatus::Installed;
        product.version = Some("1.0.0".to_string());
        product.install_path = Some("/opt/wow".to_string());
        registry.update(&product).await.unwrap();

        let fetched = registry.get("wow").await.unwrap();
        assert_eq!(fetched.status, ProductStatus::Installed);
        assert_eq!(fetched.version.as_deref(), Some("1.0.0"));
    }

    #[tokio::test]
    async fn test_case_insensitive_get() {
        let registry = test_registry().await;
        registry
            .insert(&Product::new(
                "wow_classic".to_string(),
                "WoW Classic".to_string(),
            ))
            .await
            .unwrap();

        // Lookup with mixed case should find the product.
        let fetched = registry.get("WoW_Classic").await.unwrap();
        assert_eq!(fetched.product_code, "wow_classic");

        let fetched = registry.get("WOW_CLASSIC").await.unwrap();
        assert_eq!(fetched.product_code, "wow_classic");
    }

    #[tokio::test]
    async fn test_case_insensitive_exists() {
        let registry = test_registry().await;
        registry
            .insert(&Product::new("wow".to_string(), "WoW".to_string()))
            .await
            .unwrap();

        assert!(registry.exists("WoW").await.unwrap());
        assert!(registry.exists("WOW").await.unwrap());
        assert!(registry.exists("wow").await.unwrap());
    }

    #[tokio::test]
    async fn test_insert_normalizes_product_code() {
        let registry = test_registry().await;
        let product = Product::new("WoW_Classic".to_string(), "WoW Classic".to_string());
        registry.insert(&product).await.unwrap();

        // Stored as lowercase.
        let fetched = registry.get("wow_classic").await.unwrap();
        assert_eq!(fetched.product_code, "wow_classic");
    }
}
