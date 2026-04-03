//! Tags table CRUD operations.
//!
//! The `tags` table stores a single blob of tag data associated with
//! the build. The format of the blob is opaque at this layer.

use turso::Connection;

use crate::error::{ContainerlessError, ContainerlessResult};

/// Get the tags blob from the database.
pub async fn get_tags(conn: &Connection) -> ContainerlessResult<Vec<u8>> {
    let mut rows = conn.query("SELECT data FROM tags LIMIT 1", ()).await?;

    match rows.next().await? {
        Some(row) => Ok(row.get::<Vec<u8>>(0)?),
        None => Err(ContainerlessError::NotFound("no tags data".to_string())),
    }
}

/// Insert or replace the tags blob.
pub async fn set_tags(conn: &Connection, data: &[u8]) -> ContainerlessResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO tags (id, data) VALUES (1, ?1)",
        turso::params![data],
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
    async fn test_tags_round_trip() {
        let conn = test_conn().await;
        let data = vec![0x01, 0x02, 0x03, 0xFF, 0xFE];

        set_tags(&conn, &data).await.unwrap();
        let loaded = get_tags(&conn).await.unwrap();

        assert_eq!(loaded, data);
    }

    #[tokio::test]
    async fn test_tags_not_found() {
        let conn = test_conn().await;
        let result = get_tags(&conn).await;
        assert!(matches!(result, Err(ContainerlessError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_tags_update() {
        let conn = test_conn().await;
        set_tags(&conn, &[0x01, 0x02]).await.unwrap();
        set_tags(&conn, &[0x03, 0x04, 0x05]).await.unwrap();

        let loaded = get_tags(&conn).await.unwrap();
        assert_eq!(loaded, vec![0x03, 0x04, 0x05]);
    }
}
