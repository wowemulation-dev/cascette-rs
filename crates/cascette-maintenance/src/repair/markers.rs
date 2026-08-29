//! Repair marker file handling.
//!
//! Two marker formats:
//! - `RepairMarker.psv` — pipe-separated list of encoding keys that need repair
//! - `CASCRepair.mrk` — binary marker recording repair state for crash recovery

use std::path::Path;

use crate::error::{MaintenanceError, MaintenanceResult};
use crate::report::RepairReport;

/// PSV (pipe-separated value) repair marker.
///
/// Lists encoding keys (9-byte truncated) that were identified as corrupted
/// and need re-download. Written to the storage directory as `RepairMarker.psv`.
#[derive(Debug, Clone, Default)]
pub struct RepairMarker {
    /// Corrupted encoding keys.
    pub keys: Vec<[u8; 9]>,
}

impl RepairMarker {
    /// Write a PSV marker file with one hex-encoded key per line.
    pub async fn write_psv(path: &Path, keys: &[[u8; 9]]) -> MaintenanceResult<()> {
        let mut content = String::new();
        for key in keys {
            content.push_str(&hex::encode(key));
            content.push('\n');
        }
        tokio::fs::write(path, content.as_bytes()).await?;
        Ok(())
    }

    /// Read a PSV marker file.
    pub async fn read_psv(path: &Path) -> MaintenanceResult<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let mut keys = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let bytes = hex::decode(line).map_err(|e| {
                MaintenanceError::Repair(format!("invalid hex in repair marker: {e}"))
            })?;
            if bytes.len() != 9 {
                return Err(MaintenanceError::Repair(format!(
                    "expected 9-byte key, got {} bytes",
                    bytes.len()
                )));
            }
            let mut key = [0u8; 9];
            key.copy_from_slice(&bytes);
            keys.push(key);
        }

        Ok(Self { keys })
    }

    /// Delete a PSV marker file. No error if the file does not exist.
    pub async fn delete(path: &Path) -> MaintenanceResult<()> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Write a human-readable summary of repair results.
    pub async fn write_summary(path: &Path, report: &RepairReport) -> MaintenanceResult<()> {
        let content = format!(
            "entries_verified|{}\n\
             entries_valid|{}\n\
             entries_corrupted|{}\n\
             entries_redownloaded|{}\n\
             redownload_failed|{}\n\
             loose_files_checked|{}\n\
             loose_files_repaired|{}\n\
             indices_rebuilt|{}\n",
            report.entries_verified,
            report.entries_valid,
            report.entries_corrupted,
            report.entries_redownloaded,
            report.redownload_failed,
            report.loose_files_checked,
            report.loose_files_repaired,
            report.indices_rebuilt,
        );
        tokio::fs::write(path, content.as_bytes()).await?;
        Ok(())
    }
}

/// Binary repair state marker (`CASCRepair.mrk`).
///
/// Records the repair state machine's position so that an interrupted repair
/// can resume from where it left off.
#[derive(Debug, Clone)]
pub struct CascRepairMarker {
    /// Marker version (1 or 2).
    pub version: u32,
    /// Repair state when the marker was written.
    pub state: u32,
}

impl CascRepairMarker {
    /// Write a binary repair marker.
    pub async fn write(path: &Path, version: u32, state: u32) -> MaintenanceResult<()> {
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&version.to_le_bytes());
        data.extend_from_slice(&state.to_le_bytes());
        tokio::fs::write(path, &data).await?;
        Ok(())
    }

    /// Read a binary repair marker. Returns `None` if the file does not exist.
    pub async fn read(path: &Path) -> MaintenanceResult<Option<Self>> {
        let data = match tokio::fs::read(path).await {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        if data.len() < 8 {
            return Err(MaintenanceError::Repair(format!(
                "CASCRepair.mrk too short: {} bytes",
                data.len()
            )));
        }

        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let state = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);

        Ok(Some(Self { version, state }))
    }

    /// Delete a binary repair marker. No error if the file does not exist.
    pub async fn delete(path: &Path) -> MaintenanceResult<()> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn psv_marker_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("RepairMarker.psv");

        let keys = vec![[1, 2, 3, 4, 5, 6, 7, 8, 9], [9, 8, 7, 6, 5, 4, 3, 2, 1]];

        RepairMarker::write_psv(&path, &keys).await.unwrap();
        let marker = RepairMarker::read_psv(&path).await.unwrap();

        assert_eq!(marker.keys.len(), 2);
        assert_eq!(marker.keys[0], keys[0]);
        assert_eq!(marker.keys[1], keys[1]);
    }

    #[tokio::test]
    async fn empty_psv_marker() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("RepairMarker.psv");

        RepairMarker::write_psv(&path, &[]).await.unwrap();
        let marker = RepairMarker::read_psv(&path).await.unwrap();
        assert!(marker.keys.is_empty());
    }

    #[tokio::test]
    async fn delete_nonexistent_marker() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.psv");
        // Should not error
        RepairMarker::delete(&path).await.unwrap();
    }

    #[tokio::test]
    async fn casc_repair_marker_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("CASCRepair.mrk");

        CascRepairMarker::write(&path, 2, 7).await.unwrap();
        let marker = CascRepairMarker::read(&path).await.unwrap().unwrap();

        assert_eq!(marker.version, 2);
        assert_eq!(marker.state, 7);
    }

    #[tokio::test]
    async fn casc_repair_marker_nonexistent() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("CASCRepair.mrk");
        let result = CascRepairMarker::read(&path).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_casc_marker() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.mrk");
        CascRepairMarker::delete(&path).await.unwrap();
    }

    #[tokio::test]
    async fn write_summary() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("summary.psv");
        let report = RepairReport {
            entries_verified: 100,
            entries_valid: 97,
            entries_corrupted: 3,
            entries_redownloaded: 2,
            redownload_failed: 1,
            ..Default::default()
        };
        RepairMarker::write_summary(&path, &report).await.unwrap();
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("entries_corrupted|3"));
        assert!(content.contains("entries_redownloaded|2"));
    }
}
