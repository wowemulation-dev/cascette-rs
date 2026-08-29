//! Database initialization, schema creation, and migrations.

use turso::Connection;

use crate::error::AgentResult;

/// Current schema version.
pub const SCHEMA_VERSION: i64 = 8;

/// Database handle wrapping a turso connection.
#[derive(Debug, Clone)]
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create a database at the given path.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or schema migration fails.
    pub async fn open(path: &std::path::Path) -> AgentResult<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let path_str = path.to_string_lossy().to_string();
        let db = turso::Builder::new_local(&path_str)
            .build()
            .await
            .map_err(|e| crate::error::AgentError::InvalidConfig(format!("database open: {e}")))?;
        let conn = db.connect().map_err(|e| {
            crate::error::AgentError::InvalidConfig(format!("database connect: {e}"))
        })?;

        let database = Self { conn };
        database.migrate().await?;
        Ok(database)
    }

    /// Open an in-memory database (for testing).
    ///
    /// # Errors
    ///
    /// Returns an error if schema creation fails.
    pub async fn open_memory() -> AgentResult<Self> {
        let db = turso::Builder::new_local(":memory:")
            .build()
            .await
            .map_err(|e| crate::error::AgentError::InvalidConfig(format!("database open: {e}")))?;
        let conn = db.connect().map_err(|e| {
            crate::error::AgentError::InvalidConfig(format!("database connect: {e}"))
        })?;

        let database = Self { conn };
        database.migrate().await?;
        Ok(database)
    }

    /// Get a reference to the underlying connection.
    #[must_use]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    async fn migrate(&self) -> AgentResult<()> {
        let _ = self.conn.query("PRAGMA journal_mode = WAL", ()).await?;
        let _ = self.conn.query("PRAGMA synchronous = NORMAL", ()).await?;
        let _ = self.conn.query("PRAGMA foreign_keys = ON", ()).await?;

        self.create_schema().await?;
        self.migrate_v1_to_v2().await?;
        self.migrate_v2_to_v3().await?;
        self.migrate_v3_to_v4().await?;
        self.migrate_v4_to_v5().await?;
        self.migrate_v5_to_v6().await?;
        self.migrate_v6_to_v7().await?;
        self.migrate_v7_to_v8().await?;
        self.reset_inflight_operations().await?;
        Ok(())
    }

    /// Reset operations that were in-flight when the agent last stopped.
    ///
    /// Operations in `initializing`, `downloading`, or `verifying` state were
    /// interrupted by an agent restart. Reset them to `queued` so the runner
    /// re-dispatches them. The install pipeline resumes from checkpoint.
    ///
    /// Also resets corresponding product statuses so the install executor's
    /// `transition_to(Installing)` call succeeds on resume.
    async fn reset_inflight_operations(&self) -> AgentResult<()> {
        // Reset in-flight operations back to queued.
        self.conn
            .execute(
                "UPDATE operations
                 SET state = 'queued', updated_at = CURRENT_TIMESTAMP
                 WHERE state IN ('initializing', 'downloading', 'verifying')",
                (),
            )
            .await?;

        // Reset products whose status reflects an interrupted operation.
        // Installing -> Available so the install executor can re-enter the
        // Installing state on resume. Updating -> Installed for the same reason.
        // Unconditional: any product left in a transient status after the
        // operation reset above is safe to roll back.
        self.conn
            .execute(
                "UPDATE products
                 SET status = 'available', updated_at = CURRENT_TIMESTAMP
                 WHERE status = 'installing'",
                (),
            )
            .await?;
        self.conn
            .execute(
                "UPDATE products
                 SET status = 'installed', updated_at = CURRENT_TIMESTAMP
                 WHERE status = 'updating'",
                (),
            )
            .await?;
        Ok(())
    }

    async fn create_schema(&self) -> AgentResult<()> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS schema_version (
                    version INTEGER PRIMARY KEY,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP
                )",
                (),
            )
            .await?;

        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS products (
                    product_code TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'available',
                    version TEXT,
                    install_path TEXT,
                    size_bytes INTEGER,
                    region TEXT,
                    locale TEXT,
                    installation_mode TEXT,
                    is_update_available INTEGER DEFAULT 0,
                    available_version TEXT,
                    patch_url TEXT,
                    protocol TEXT,
                    build_config TEXT,
                    cdn_config TEXT,
                    subfolder TEXT,
                    patch_region_hint TEXT,
                    account_country TEXT,
                    geo_ip_country TEXT,
                    product_config_hash TEXT,
                    platform TEXT,
                    architecture TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                (),
            )
            .await?;

        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS operations (
                    operation_id TEXT PRIMARY KEY,
                    product_code TEXT NOT NULL REFERENCES products(product_code),
                    operation_type TEXT NOT NULL,
                    state TEXT NOT NULL DEFAULT 'queued',
                    priority TEXT NOT NULL DEFAULT 'normal',
                    parameters TEXT,
                    metadata TEXT,
                    progress TEXT,
                    error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    started_at TEXT,
                    completed_at TEXT
                )",
                (),
            )
            .await?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_operations_product ON operations(product_code)",
                (),
            )
            .await?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_operations_state ON operations(state)",
                (),
            )
            .await?;

        self.conn
            .execute(
                "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
                turso::params![SCHEMA_VERSION],
            )
            .await?;

        Ok(())
    }

    /// Migrate from schema v1 to v2: add patch_url and protocol columns.
    ///
    /// Uses `ALTER TABLE ... ADD COLUMN` which is a no-op if the column
    /// already exists (covered by the IF NOT EXISTS in fresh schemas).
    async fn migrate_v1_to_v2(&self) -> AgentResult<()> {
        // SQLite ignores ADD COLUMN if the column already exists when using
        // a check first. We just attempt the alter and ignore "duplicate column" errors.
        for col in &["patch_url TEXT", "protocol TEXT"] {
            let sql = format!("ALTER TABLE products ADD COLUMN {col}");
            if let Err(e) = self.conn.execute(&sql, ()).await {
                let msg = e.to_string();
                if !msg.contains("duplicate column") {
                    return Err(crate::error::AgentError::Database(e));
                }
            }
        }
        Ok(())
    }

    /// Migrate from schema v2 to v3: add build_config and cdn_config columns.
    async fn migrate_v2_to_v3(&self) -> AgentResult<()> {
        for col in &["build_config TEXT", "cdn_config TEXT"] {
            let sql = format!("ALTER TABLE products ADD COLUMN {col}");
            if let Err(e) = self.conn.execute(&sql, ()).await {
                let msg = e.to_string();
                if !msg.contains("duplicate column") {
                    return Err(crate::error::AgentError::Database(e));
                }
            }
        }
        Ok(())
    }

    /// Migrate from schema v3 to v4: add subfolder and patch_region_hint columns.
    async fn migrate_v3_to_v4(&self) -> AgentResult<()> {
        for col in &["subfolder TEXT", "patch_region_hint TEXT"] {
            let sql = format!("ALTER TABLE products ADD COLUMN {col}");
            if let Err(e) = self.conn.execute(&sql, ()).await {
                let msg = e.to_string();
                if !msg.contains("duplicate column") {
                    return Err(crate::error::AgentError::Database(e));
                }
            }
        }
        Ok(())
    }

    /// Migrate from schema v4 to v5: add product_download_config table.
    ///
    /// Per-product download configuration (background_download, priority,
    /// download_limit, paused) is stored separately from the product registry.
    /// We model this as a separate table rather than widening the products table.
    async fn migrate_v4_to_v5(&self) -> AgentResult<()> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS product_download_config (
                    product_code TEXT PRIMARY KEY REFERENCES products(product_code),
                    background_download INTEGER NOT NULL DEFAULT 0,
                    priority INTEGER NOT NULL DEFAULT 700,
                    download_limit INTEGER NOT NULL DEFAULT 0,
                    paused INTEGER NOT NULL DEFAULT 0
                )",
                (),
            )
            .await?;
        Ok(())
    }

    /// Migrate from schema v5 to v6: add account_country and geo_ip_country columns.
    ///
    /// These store the ISO country codes received via the launcher's
    /// `POST /agent/{product}` override endpoint and propagate to
    /// `.product.db` and `.build.info` tag strings.
    async fn migrate_v5_to_v6(&self) -> AgentResult<()> {
        for col in &["account_country TEXT", "geo_ip_country TEXT"] {
            let sql = format!("ALTER TABLE products ADD COLUMN {col}");
            if let Err(e) = self.conn.execute(&sql, ()).await {
                let msg = e.to_string();
                if !msg.contains("duplicate column") {
                    return Err(crate::error::AgentError::Database(e));
                }
            }
        }
        Ok(())
    }

    /// Migrate from schema v6 to v7: add product_config_hash column.
    ///
    /// Stores the hex hash pointing to the product config JSON on CDN.
    /// Used to resolve `launcher_install_info` for concurrent launcher
    /// binary installation.
    async fn migrate_v6_to_v7(&self) -> AgentResult<()> {
        let sql = "ALTER TABLE products ADD COLUMN product_config_hash TEXT";
        if let Err(e) = self.conn.execute(sql, ()).await {
            let msg = e.to_string();
            if !msg.contains("duplicate column") {
                return Err(crate::error::AgentError::Database(e));
            }
        }
        Ok(())
    }

    /// Migrate from schema v7 to v8: add platform and architecture columns.
    ///
    /// Allows registering products for non-Windows platforms (e.g., "OSX")
    /// and non-x86_64 architectures (e.g., "arm64"). Existing products
    /// with NULL values fall back to "Windows" / "x86_64" at the config
    /// builder layer.
    async fn migrate_v7_to_v8(&self) -> AgentResult<()> {
        for col in &["platform TEXT", "architecture TEXT"] {
            let sql = format!("ALTER TABLE products ADD COLUMN {col}");
            if let Err(e) = self.conn.execute(&sql, ()).await {
                let msg = e.to_string();
                if !msg.contains("duplicate column") {
                    return Err(crate::error::AgentError::Database(e));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_open_memory() {
        let db = Database::open_memory().await.unwrap();
        // Verify schema version
        let mut rows = db
            .conn()
            .query("SELECT MAX(version) FROM schema_version", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let version: i64 = row.get(0).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[tokio::test]
    async fn test_schema_idempotent() {
        let db = Database::open_memory().await.unwrap();
        // Tables should exist
        let mut rows = db
            .conn()
            .query("SELECT COUNT(*) FROM products", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_foreign_keys_enabled() {
        let db = Database::open_memory().await.unwrap();
        let mut rows = db.conn().query("PRAGMA foreign_keys", ()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let fk: i64 = row.get(0).unwrap();
        assert_eq!(fk, 1);
    }
}
