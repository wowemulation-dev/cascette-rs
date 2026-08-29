//! Files table CRUD operations.
//!
//! The `files` table is the main table storing metadata for each content
//! file in the containerless installation. Files are keyed by their
//! encoding key (ekey) and content key (ckey), both 16-byte MD5 hashes.

use turso::Connection;

use crate::error::ContainerlessResult;

/// A file entry from the `files` table.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// File index / FDID.
    pub index: u32,
    /// Encoding key (MD5 of BLTE-encoded data).
    pub ekey: [u8; 16],
    /// Content key (MD5 of decoded data).
    pub ckey: [u8; 16],
    /// BLTE-encoded size on disk.
    pub encoded_size: u64,
    /// Decompressed content size.
    pub decoded_size: u64,
    /// File path (FDID-only entries have `None`).
    pub path: Option<String>,
    /// Flags.
    pub flags: u32,
}

/// Get a file entry by encoding key.
pub async fn get_file(
    conn: &Connection,
    ekey: &[u8; 16],
) -> ContainerlessResult<Option<FileEntry>> {
    let mut rows = conn
        .query(
            "SELECT idx, ekey, ckey, encoded_size, decoded_size, path, flags FROM files WHERE ekey = ?1",
            turso::params![&ekey[..]],
        )
        .await?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_entry(&row)?)),
        None => Ok(None),
    }
}

/// Get a file entry by content key.
pub async fn get_file_by_ckey(
    conn: &Connection,
    ckey: &[u8; 16],
) -> ContainerlessResult<Option<FileEntry>> {
    let mut rows = conn
        .query(
            "SELECT idx, ekey, ckey, encoded_size, decoded_size, path, flags FROM files WHERE ckey = ?1",
            turso::params![&ckey[..]],
        )
        .await?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_entry(&row)?)),
        None => Ok(None),
    }
}

/// Insert or update a file entry.
pub async fn upsert_file(conn: &Connection, entry: &FileEntry) -> ContainerlessResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO files (idx, ekey, ckey, encoded_size, decoded_size, path, flags) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        turso::params![
            i64::from(entry.index),
            &entry.ekey[..],
            &entry.ckey[..],
            entry.encoded_size.cast_signed(),
            entry.decoded_size.cast_signed(),
            entry.path.as_deref(),
            i64::from(entry.flags)
        ],
    )
    .await?;
    Ok(())
}

/// Remove a file entry by encoding key. Returns `true` if a row was deleted.
pub async fn remove_file(conn: &Connection, ekey: &[u8; 16]) -> ContainerlessResult<bool> {
    let changed = conn
        .execute(
            "DELETE FROM files WHERE ekey = ?1",
            turso::params![&ekey[..]],
        )
        .await?;
    Ok(changed > 0)
}

/// Get all file entries.
pub async fn all_files(conn: &Connection) -> ContainerlessResult<Vec<FileEntry>> {
    let mut rows = conn
        .query(
            "SELECT idx, ekey, ckey, encoded_size, decoded_size, path, flags FROM files ORDER BY idx",
            (),
        )
        .await?;

    let mut entries = Vec::new();
    while let Some(row) = rows.next().await? {
        entries.push(row_to_entry(&row)?);
    }
    Ok(entries)
}

/// Count the number of file entries.
pub async fn file_count(conn: &Connection) -> ContainerlessResult<usize> {
    let mut rows = conn.query("SELECT COUNT(*) FROM files", ()).await?;
    match rows.next().await? {
        Some(row) => {
            let count: i64 = row.get(0)?;
            Ok(count as usize)
        }
        None => Ok(0),
    }
}

/// Convert a turso Row to a `FileEntry`.
fn row_to_entry(row: &turso::Row) -> ContainerlessResult<FileEntry> {
    let idx: i64 = row.get(0)?;
    let ekey_blob: Vec<u8> = row.get(1)?;
    let ckey_blob: Vec<u8> = row.get(2)?;
    let encoded_size: i64 = row.get(3)?;
    let decoded_size: i64 = row.get(4)?;
    let path: Option<String> = row.get::<Option<String>>(5).ok().flatten();
    let flags: i64 = row.get(6)?;

    let mut ekey = [0u8; 16];
    let mut ckey = [0u8; 16];

    let ekey_len = ekey_blob.len().min(16);
    ekey[..ekey_len].copy_from_slice(&ekey_blob[..ekey_len]);

    let ckey_len = ckey_blob.len().min(16);
    ckey[..ckey_len].copy_from_slice(&ckey_blob[..ckey_len]);

    Ok(FileEntry {
        index: idx as u32,
        ekey,
        ckey,
        encoded_size: encoded_size as u64,
        decoded_size: decoded_size as u64,
        path,
        flags: flags as u32,
    })
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

    fn sample_entry(index: u32, ekey_byte: u8, ckey_byte: u8) -> FileEntry {
        FileEntry {
            index,
            ekey: [ekey_byte; 16],
            ckey: [ckey_byte; 16],
            encoded_size: 1024,
            decoded_size: 2048,
            path: Some(format!("data/file_{index}.bin")),
            flags: 0,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get_by_ekey() {
        let conn = test_conn().await;
        let entry = sample_entry(1, 0xAA, 0xBB);

        upsert_file(&conn, &entry).await.unwrap();
        let loaded = get_file(&conn, &entry.ekey).await.unwrap().unwrap();

        assert_eq!(loaded.index, 1);
        assert_eq!(loaded.ekey, [0xAA; 16]);
        assert_eq!(loaded.ckey, [0xBB; 16]);
        assert_eq!(loaded.encoded_size, 1024);
        assert_eq!(loaded.decoded_size, 2048);
        assert_eq!(loaded.path.as_deref(), Some("data/file_1.bin"));
    }

    #[tokio::test]
    async fn test_get_by_ckey() {
        let conn = test_conn().await;
        let entry = sample_entry(2, 0xCC, 0xDD);

        upsert_file(&conn, &entry).await.unwrap();
        let loaded = get_file_by_ckey(&conn, &entry.ckey).await.unwrap().unwrap();

        assert_eq!(loaded.index, 2);
        assert_eq!(loaded.ckey, [0xDD; 16]);
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let conn = test_conn().await;
        let ekey = [0xFF; 16];
        let result = get_file(&conn, &ekey).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_remove_file() {
        let conn = test_conn().await;
        let entry = sample_entry(3, 0x11, 0x22);

        upsert_file(&conn, &entry).await.unwrap();
        let removed = remove_file(&conn, &entry.ekey).await.unwrap();
        assert!(removed);

        let result = get_file(&conn, &entry.ekey).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_remove_nonexistent() {
        let conn = test_conn().await;
        let ekey = [0xFF; 16];
        let removed = remove_file(&conn, &ekey).await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_all_files() {
        let conn = test_conn().await;
        upsert_file(&conn, &sample_entry(1, 0x01, 0x10))
            .await
            .unwrap();
        upsert_file(&conn, &sample_entry(2, 0x02, 0x20))
            .await
            .unwrap();
        upsert_file(&conn, &sample_entry(3, 0x03, 0x30))
            .await
            .unwrap();

        let files = all_files(&conn).await.unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].index, 1);
        assert_eq!(files[2].index, 3);
    }

    #[tokio::test]
    async fn test_file_count() {
        let conn = test_conn().await;
        assert_eq!(file_count(&conn).await.unwrap(), 0);

        upsert_file(&conn, &sample_entry(1, 0x01, 0x10))
            .await
            .unwrap();
        upsert_file(&conn, &sample_entry(2, 0x02, 0x20))
            .await
            .unwrap();
        assert_eq!(file_count(&conn).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_upsert_replaces() {
        let conn = test_conn().await;
        let mut entry = sample_entry(1, 0xAA, 0xBB);
        upsert_file(&conn, &entry).await.unwrap();

        entry.encoded_size = 4096;
        entry.flags = 7;
        upsert_file(&conn, &entry).await.unwrap();

        let loaded = get_file(&conn, &entry.ekey).await.unwrap().unwrap();
        assert_eq!(loaded.encoded_size, 4096);
        assert_eq!(loaded.flags, 7);
        assert_eq!(file_count(&conn).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_file_without_path() {
        let conn = test_conn().await;
        let entry = FileEntry {
            index: 42,
            ekey: [0x42; 16],
            ckey: [0x43; 16],
            encoded_size: 512,
            decoded_size: 1024,
            path: None,
            flags: 0,
        };

        upsert_file(&conn, &entry).await.unwrap();
        let loaded = get_file(&conn, &entry.ekey).await.unwrap().unwrap();
        assert!(loaded.path.is_none());
    }
}
