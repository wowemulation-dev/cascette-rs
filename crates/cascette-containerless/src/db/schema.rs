//! SQLite schema DDL and migrations for the containerless database.

use turso::Connection;

use crate::error::ContainerlessResult;

/// Current schema version.
pub const SCHEMA_VERSION: i64 = 1;

/// Create the initial database schema.
pub async fn create_schema(conn: &Connection) -> ContainerlessResult<()> {
    let _ = conn.query("PRAGMA journal_mode = WAL", ()).await?;
    let _ = conn.query("PRAGMA synchronous = NORMAL", ()).await?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS meta (
            id          INTEGER PRIMARY KEY,
            build_key   TEXT NOT NULL,
            build_uid   TEXT,
            product     TEXT,
            version     TEXT
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS tags (
            id   INTEGER PRIMARY KEY,
            data BLOB NOT NULL
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS files (
            idx          INTEGER PRIMARY KEY,
            ekey         BLOB NOT NULL,
            ckey         BLOB NOT NULL,
            encoded_size INTEGER NOT NULL,
            decoded_size INTEGER NOT NULL,
            path         TEXT,
            flags        INTEGER NOT NULL DEFAULT 0
        )",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_ekey ON files(ekey)",
        (),
    )
    .await?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_files_ckey ON files(ckey)",
        (),
    )
    .await?;

    conn.execute(
        "INSERT OR REPLACE INTO schema_version (version) VALUES (?1)",
        turso::params![SCHEMA_VERSION],
    )
    .await?;

    Ok(())
}

/// Check the schema version stored in the database.
pub async fn schema_version(conn: &Connection) -> ContainerlessResult<Option<i64>> {
    // Table might not exist yet.
    let result = conn
        .query("SELECT MAX(version) FROM schema_version", ())
        .await;
    match result {
        Ok(mut rows) => match rows.next().await? {
            Some(row) => Ok(row.get::<Option<i64>>(0).ok().flatten()),
            None => Ok(None),
        },
        Err(_) => Ok(None),
    }
}

/// Migrate the schema to the current version.
pub async fn migrate(conn: &Connection) -> ContainerlessResult<()> {
    let version = schema_version(conn).await?;
    match version {
        None | Some(0) => {
            create_schema(conn).await?;
        }
        Some(v) if v < SCHEMA_VERSION => {
            // Future migrations go here.
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    async fn test_conn() -> Connection {
        let db = turso::Builder::new_local(":memory:").build().await.unwrap();
        db.connect().unwrap()
    }

    #[tokio::test]
    async fn test_create_schema() {
        let conn = test_conn().await;
        create_schema(&conn).await.unwrap();

        let version = schema_version(&conn).await.unwrap();
        assert_eq!(version, Some(SCHEMA_VERSION));
    }

    #[tokio::test]
    async fn test_create_schema_idempotent() {
        let conn = test_conn().await;
        create_schema(&conn).await.unwrap();
        create_schema(&conn).await.unwrap();

        let version = schema_version(&conn).await.unwrap();
        assert_eq!(version, Some(SCHEMA_VERSION));
    }

    #[tokio::test]
    async fn test_migrate_fresh() {
        let conn = test_conn().await;
        migrate(&conn).await.unwrap();

        let version = schema_version(&conn).await.unwrap();
        assert_eq!(version, Some(SCHEMA_VERSION));
    }

    #[tokio::test]
    async fn test_tables_exist_after_schema() {
        let conn = test_conn().await;
        create_schema(&conn).await.unwrap();

        let mut rows = conn.query("SELECT COUNT(*) FROM meta", ()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        assert_eq!(count, 0);

        let mut rows = conn.query("SELECT COUNT(*) FROM tags", ()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        assert_eq!(count, 0);

        let mut rows = conn.query("SELECT COUNT(*) FROM files", ()).await.unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let count: i64 = row.get(0).unwrap();
        assert_eq!(count, 0);
    }
}
