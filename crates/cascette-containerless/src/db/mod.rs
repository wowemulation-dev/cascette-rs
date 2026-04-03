//! SQLite database layer for containerless storage.
//!
//! Wraps turso (libsql) with schema management, encryption, and typed
//! accessors for the `meta`, `tags`, and `files` tables.

pub mod crypto;
pub mod files;
pub mod meta;
pub mod schema;
pub mod tags;

use std::path::Path;

use tracing::debug;

use crate::error::{ContainerlessError, ContainerlessResult};

pub use files::FileEntry;
pub use meta::BuildMeta;

/// SQLite database for containerless file metadata.
///
/// Holds both the turso `Database` handle (which must outlive the
/// connection) and a `Connection` used for all queries.
pub struct FileDatabase {
    _db: turso::Database,
    conn: turso::Connection,
    /// If backed by a temp file (for encrypted DBs), keep it alive.
    temp_dir: Option<tempfile::TempDir>,
}

impl FileDatabase {
    /// Open an encrypted database from raw bytes.
    ///
    /// Decrypts the bytes with Salsa20 using the given key and IV,
    /// writes to a temporary file, and opens from there. Handles both
    /// plain SQLite files and our bundled db+WAL export format.
    pub async fn open_encrypted(
        data: &[u8],
        key: &[u8; 16],
        iv: &[u8],
    ) -> ContainerlessResult<Self> {
        let decrypted = crypto::decrypt_db(data, key, iv)?;
        let (temp_dir, temp_path) = unpack_db_bytes(&decrypted).await?;

        let db = turso::Builder::new_local(&temp_path.to_string_lossy())
            .build()
            .await?;
        let conn = db.connect()?;
        schema::migrate(&conn).await?;
        debug!("opened encrypted database from {} bytes", data.len());
        Ok(Self {
            _db: db,
            conn,
            temp_dir: Some(temp_dir),
        })
    }

    /// Open a plaintext database from a file path.
    pub async fn open_plaintext(path: &Path) -> ContainerlessResult<Self> {
        let db = turso::Builder::new_local(&path.to_string_lossy())
            .build()
            .await?;
        let conn = db.connect()?;
        schema::migrate(&conn).await?;
        Ok(Self {
            _db: db,
            conn,
            temp_dir: None,
        })
    }

    /// Create a new empty database with the schema.
    ///
    /// Uses a temporary file so the database can be exported later
    /// (turso does not support `VACUUM INTO` for in-memory databases).
    /// Journal mode is set to DELETE (not WAL) so all data resides in
    /// the single database file, enabling reliable `export_raw`.
    pub async fn create_new() -> ContainerlessResult<Self> {
        let temp_dir = tempfile::tempdir()?;
        let temp_path = temp_dir.path().join("new.db");
        let db = turso::Builder::new_local(&temp_path.to_string_lossy())
            .build()
            .await?;
        let conn = db.connect()?;
        schema::create_schema(&conn).await?;
        Ok(Self {
            _db: db,
            conn,
            temp_dir: Some(temp_dir),
        })
    }

    /// Get the underlying connection.
    #[must_use]
    pub fn connection(&self) -> &turso::Connection {
        &self.conn
    }

    /// Get build metadata.
    pub async fn get_meta(&self) -> ContainerlessResult<BuildMeta> {
        meta::get_meta(&self.conn).await
    }

    /// Set build metadata.
    pub async fn set_meta(&self, m: &BuildMeta) -> ContainerlessResult<()> {
        meta::set_meta(&self.conn, m).await
    }

    /// Get the tags blob.
    pub async fn get_tags(&self) -> ContainerlessResult<Vec<u8>> {
        tags::get_tags(&self.conn).await
    }

    /// Set the tags blob.
    pub async fn set_tags(&self, data: &[u8]) -> ContainerlessResult<()> {
        tags::set_tags(&self.conn, data).await
    }

    /// Get a file entry by encoding key.
    pub async fn get_file(&self, ekey: &[u8; 16]) -> ContainerlessResult<Option<FileEntry>> {
        files::get_file(&self.conn, ekey).await
    }

    /// Get a file entry by content key.
    pub async fn get_file_by_ckey(
        &self,
        ckey: &[u8; 16],
    ) -> ContainerlessResult<Option<FileEntry>> {
        files::get_file_by_ckey(&self.conn, ckey).await
    }

    /// Insert or update a file entry.
    pub async fn upsert_file(&self, entry: &FileEntry) -> ContainerlessResult<()> {
        files::upsert_file(&self.conn, entry).await
    }

    /// Remove a file entry. Returns `true` if a row was deleted.
    pub async fn remove_file(&self, ekey: &[u8; 16]) -> ContainerlessResult<bool> {
        files::remove_file(&self.conn, ekey).await
    }

    /// Get all file entries.
    pub async fn all_files(&self) -> ContainerlessResult<Vec<FileEntry>> {
        files::all_files(&self.conn).await
    }

    /// Count file entries.
    pub async fn file_count(&self) -> ContainerlessResult<usize> {
        files::file_count(&self.conn).await
    }

    /// Export the database as encrypted bytes.
    ///
    /// For file-backed databases, reads the file directly. For in-memory
    /// databases, this exports to a temp file first via `VACUUM INTO`.
    pub async fn export_encrypted(
        &self,
        key: &[u8; 16],
        iv: &[u8],
    ) -> ContainerlessResult<Vec<u8>> {
        let raw = self.export_raw().await?;
        crypto::encrypt_db(&raw, key, iv)
    }

    /// Export the database as raw (unencrypted) bytes.
    ///
    /// Only works for databases backed by a temp directory (encrypted
    /// and newly created databases). The export bundles the main
    /// database file and WAL file together because turso does not
    /// support WAL checkpointing. Format:
    /// `[4-byte LE db_len][db_bytes][wal_bytes]`.
    pub async fn export_raw(&self) -> ContainerlessResult<Vec<u8>> {
        let temp_dir = self.temp_dir.as_ref().ok_or_else(|| {
            ContainerlessError::InvalidConfig(
                "export_raw requires a temp-dir-backed database".to_string(),
            )
        })?;

        let db_path = find_db_file(temp_dir)?;

        let db_data = tokio::fs::read(&db_path).await?;
        let wal_path = db_path.with_extension("db-wal");
        let wal_data = if wal_path.exists() {
            tokio::fs::read(&wal_path).await.unwrap_or_default()
        } else {
            Vec::new()
        };

        // Pack: [4-byte LE length of db_data][db_data][wal_data]
        let db_len = u32::try_from(db_data.len()).map_err(|_| {
            ContainerlessError::Integrity("database file too large for export".to_string())
        })?;
        let mut out = Vec::with_capacity(4 + db_data.len() + wal_data.len());
        out.extend_from_slice(&db_len.to_le_bytes());
        out.extend_from_slice(&db_data);
        out.extend_from_slice(&wal_data);
        Ok(out)
    }
}

/// Find the database file inside a temp directory.
fn find_db_file(temp_dir: &tempfile::TempDir) -> ContainerlessResult<std::path::PathBuf> {
    for name in &["decrypted.db", "new.db"] {
        let candidate = temp_dir.path().join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(ContainerlessError::NotFound(
        "database file not found in temp dir".to_string(),
    ))
}

/// SQLite file magic header bytes.
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

/// Unpack database bytes into a temp directory.
///
/// Handles two formats:
/// - Plain SQLite file (starts with `"SQLite format 3\0"`)
/// - Bundled export: `[4-byte LE db_len][db_bytes][wal_bytes]`
///
/// Returns the temp directory (must be kept alive) and the database
/// file path within it.
async fn unpack_db_bytes(
    data: &[u8],
) -> ContainerlessResult<(tempfile::TempDir, std::path::PathBuf)> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("decrypted.db");

    if data.len() >= SQLITE_MAGIC.len() && data[..SQLITE_MAGIC.len()] == *SQLITE_MAGIC {
        // Plain SQLite file — write directly.
        tokio::fs::write(&db_path, data).await?;
    } else if data.len() >= 4 {
        // Bundled format with length-prefixed db + wal.
        let db_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if data.len() < 4 + db_len {
            return Err(ContainerlessError::Integrity(
                "export data truncated".to_string(),
            ));
        }

        let db_bytes = &data[4..4 + db_len];
        let wal_bytes = &data[4 + db_len..];

        tokio::fs::write(&db_path, db_bytes).await?;
        if !wal_bytes.is_empty() {
            tokio::fs::write(db_path.with_extension("db-wal"), wal_bytes).await?;
        }
    } else {
        return Err(ContainerlessError::Integrity(
            "export data too short".to_string(),
        ));
    }

    Ok((temp_dir, db_path))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_new_and_use() {
        let db = FileDatabase::create_new().await.unwrap();

        let entry = FileEntry {
            index: 1,
            ekey: [0xAA; 16],
            ckey: [0xBB; 16],
            encoded_size: 100,
            decoded_size: 200,
            path: Some("test.bin".to_string()),
            flags: 0,
        };

        db.upsert_file(&entry).await.unwrap();
        let loaded = db.get_file(&[0xAA; 16]).await.unwrap().unwrap();
        assert_eq!(loaded.index, 1);
        assert_eq!(loaded.encoded_size, 100);
    }

    #[tokio::test]
    async fn test_export_import_encrypted() {
        let db = FileDatabase::create_new().await.unwrap();
        let m = BuildMeta {
            id: 1,
            build_key: "test-build".to_string(),
            build_uid: None,
            product: Some("wow".to_string()),
            version: None,
        };
        db.set_meta(&m).await.unwrap();

        let entry = FileEntry {
            index: 42,
            ekey: [0x42; 16],
            ckey: [0x43; 16],
            encoded_size: 512,
            decoded_size: 1024,
            path: None,
            flags: 0,
        };
        db.upsert_file(&entry).await.unwrap();

        let key = [0x01u8; 16];
        let iv = crypto::iv_from_key(&key);
        let encrypted = db.export_encrypted(&key, &iv).await.unwrap();

        let db2 = FileDatabase::open_encrypted(&encrypted, &key, &iv)
            .await
            .unwrap();
        let loaded_meta = db2.get_meta().await.unwrap();
        assert_eq!(loaded_meta.build_key, "test-build");

        let loaded_file = db2.get_file(&[0x42; 16]).await.unwrap().unwrap();
        assert_eq!(loaded_file.index, 42);
    }

    #[tokio::test]
    async fn test_plaintext_file_db() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        {
            let db = FileDatabase::open_plaintext(&db_path).await.unwrap();
            let entry = FileEntry {
                index: 1,
                ekey: [0x11; 16],
                ckey: [0x22; 16],
                encoded_size: 256,
                decoded_size: 512,
                path: None,
                flags: 0,
            };
            db.upsert_file(&entry).await.unwrap();
        }

        {
            let db = FileDatabase::open_plaintext(&db_path).await.unwrap();
            let loaded = db.get_file(&[0x11; 16]).await.unwrap().unwrap();
            assert_eq!(loaded.decoded_size, 512);
        }
    }
}
