//! Build metadata CRUD operations for the `meta` table.

use turso::Connection;

use crate::error::{ContainerlessError, ContainerlessResult};

/// Build metadata stored in the `meta` table.
#[derive(Debug, Clone)]
pub struct BuildMeta {
    /// Row ID.
    pub id: i64,
    /// Build config hash.
    pub build_key: String,
    /// Build UID (optional).
    pub build_uid: Option<String>,
    /// Product code (e.g. "wow", "d4").
    pub product: Option<String>,
    /// Build version string.
    pub version: Option<String>,
}

/// Get the build metadata from the database.
pub async fn get_meta(conn: &Connection) -> ContainerlessResult<BuildMeta> {
    let mut rows = conn
        .query(
            "SELECT id, build_key, build_uid, product, version FROM meta LIMIT 1",
            (),
        )
        .await?;

    match rows.next().await? {
        Some(row) => Ok(BuildMeta {
            id: row.get(0)?,
            build_key: row.get(1)?,
            build_uid: row.get::<Option<String>>(2).ok().flatten(),
            product: row.get::<Option<String>>(3).ok().flatten(),
            version: row.get::<Option<String>>(4).ok().flatten(),
        }),
        None => Err(ContainerlessError::NotFound(
            "no build metadata".to_string(),
        )),
    }
}

/// Insert or replace the build metadata.
pub async fn set_meta(conn: &Connection, meta: &BuildMeta) -> ContainerlessResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (id, build_key, build_uid, product, version) VALUES (?1, ?2, ?3, ?4, ?5)",
        turso::params![
            meta.id,
            meta.build_key.as_str(),
            meta.build_uid.as_deref(),
            meta.product.as_deref(),
            meta.version.as_deref()
        ],
    )
    .await?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::db::schema::create_schema;

    async fn test_conn() -> Connection {
        let db = turso::Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        create_schema(&conn).await.unwrap();
        conn
    }

    #[tokio::test]
    async fn test_meta_round_trip() {
        let conn = test_conn().await;
        let meta = BuildMeta {
            id: 1,
            build_key: "abc123def456".to_string(),
            build_uid: Some("uid-001".to_string()),
            product: Some("wow".to_string()),
            version: Some("1.15.5.55261".to_string()),
        };

        set_meta(&conn, &meta).await.unwrap();
        let loaded = get_meta(&conn).await.unwrap();

        assert_eq!(loaded.id, 1);
        assert_eq!(loaded.build_key, "abc123def456");
        assert_eq!(loaded.build_uid.as_deref(), Some("uid-001"));
        assert_eq!(loaded.product.as_deref(), Some("wow"));
        assert_eq!(loaded.version.as_deref(), Some("1.15.5.55261"));
    }

    #[tokio::test]
    async fn test_meta_not_found() {
        let conn = test_conn().await;
        let result = get_meta(&conn).await;
        assert!(matches!(result, Err(ContainerlessError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_meta_update() {
        let conn = test_conn().await;
        let meta = BuildMeta {
            id: 1,
            build_key: "first".to_string(),
            build_uid: None,
            product: None,
            version: None,
        };
        set_meta(&conn, &meta).await.unwrap();

        let updated = BuildMeta {
            id: 1,
            build_key: "second".to_string(),
            build_uid: Some("uid".to_string()),
            product: Some("d4".to_string()),
            version: Some("2.0".to_string()),
        };
        set_meta(&conn, &updated).await.unwrap();

        let loaded = get_meta(&conn).await.unwrap();
        assert_eq!(loaded.build_key, "second");
        assert_eq!(loaded.product.as_deref(), Some("d4"));
    }
}
