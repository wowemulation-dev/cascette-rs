//! Windows shared memory implementation using `CreateFileMapping`.
//!
//! Win32 specifics:
//! - `FILE_ATTRIBUTE_TEMPORARY` with `FILE_SHARE_READ | FILE_SHARE_WRITE`
//! - 10-retry bind with `Sleep(0)` between attempts
//! - `ERROR_DISK_FULL` returns error code 7
//! - Writer lock via named global mutex (`Global\` prefix)
//! - DACL: `D:(A;;GA;;;WD)(A;;GA;;;AN)S:(ML;;NW;;;ME)`
//!
//! Path normalization: lowercase, forward slashes, resolve `.`/`..`,
//! max 248 bytes. Naming: `Global\<normalized_path>/shmem`.

use std::path::{Path, PathBuf};

use crate::{Result, StorageError};

use super::control_block::{LOCK_FILE_SUFFIX, SHMEM_FILE_SUFFIX};

/// Lock file timeout in seconds.
const LOCK_TIMEOUT_SECS: u64 = 100;

/// Maximum path length for shmem name normalization.
const MAX_PATH_LENGTH: usize = 248;

/// Maximum number of retries for CreateFileMapping.
const MAX_CREATE_RETRIES: u32 = 10;

/// Platform shared memory handle for Windows.
///
/// Uses `CreateFileMappingW` / `MapViewOfFile` for the shared memory
/// region. The DACL grants full access to Everyone and Anonymous Logon,
/// with a mandatory label of Medium integrity (matching Agent.exe).
pub struct PlatformShmem {
    /// Size of the mapped region.
    size: usize,
    /// The normalized name used for the file mapping object.
    name: String,
    // Win32 handles would go here:
    // handle: HANDLE,
    // view: LPVOID,
}

impl PlatformShmem {
    /// Create or open a shared memory region.
    ///
    /// Uses the 10-retry pattern from Agent.exe:
    /// 1. Try `CREATE_NEW` + file mapping
    /// 2. On `ERROR_ALREADY_EXISTS`, try `OPEN_EXISTING`
    /// 3. Repeat up to 10 times with `Sleep(0)` between attempts
    pub fn open_or_create(name: &str, size: usize) -> Result<Self> {
        // TODO: implement with CreateFileMappingW when compiling on Windows
        Err(StorageError::SharedMemory(format!(
            "Windows shared memory not yet implemented (name={name}, size={size})"
        )))
    }

    /// Get a shared slice of the mapped memory.
    pub fn as_slice(&self) -> &[u8] {
        &[]
    }

    /// Get a mutable slice of the mapped memory.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut []
    }

    /// Get the size of the mapped region.
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Get the shared memory name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Lock file for writer exclusion.
///
/// On Windows, uses `CreateFileW` with `FILE_FLAG_DELETE_ON_CLOSE` and
/// `FILE_SHARE_NONE` for exclusive access.
pub struct LockFile {
    /// Path to the lock file.
    path: PathBuf,
}

impl LockFile {
    /// Acquire the lock file with a timeout.
    pub fn acquire(base_path: &Path) -> Result<Self> {
        // TODO: implement with CreateFileW when compiling on Windows
        let lock_path = base_path.with_extension("lock");
        Err(StorageError::SharedMemory(format!(
            "Windows lock file not yet implemented: {}",
            lock_path.display()
        )))
    }

    /// Get the lock file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Normalize a storage path for shmem naming on Windows.
///
/// - Converts to lowercase
/// - Replaces backslashes with forward slashes
/// - Resolves `.` and `..` components
/// - Truncates to 248 bytes
pub fn normalize_path(path: &Path) -> String {
    let s = path.to_string_lossy().to_lowercase().replace('\\', "/");

    // Resolve . and .. (simplified, no actual filesystem access)
    let mut parts: Vec<&str> = Vec::new();
    for component in s.split('/') {
        match component {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }

    let mut normalized = parts.join("/");
    if normalized.len() > MAX_PATH_LENGTH {
        normalized.truncate(MAX_PATH_LENGTH);
    }
    normalized
}

/// Generate a Windows global shmem name from a storage path.
///
/// Format: `Global\<normalized_path>/shmem`
pub fn shmem_name_from_path(storage_path: &Path) -> String {
    let normalized = normalize_path(storage_path);
    format!("Global\\{normalized}/shmem")
}

/// Check if a path is on a network drive.
///
/// Uses `GetDriveTypeW` on Windows.
pub fn is_network_drive(_path: &Path) -> bool {
    // TODO: implement with GetDriveTypeW when compiling on Windows
    false
}

/// Derive the shmem file path from a storage directory.
pub fn shmem_file_path(storage_path: &Path) -> PathBuf {
    storage_path.join(SHMEM_FILE_SUFFIX.trim_start_matches('.'))
}

/// Derive the lock file path from a storage directory.
pub fn lock_file_path(storage_path: &Path) -> PathBuf {
    let shmem = shmem_file_path(storage_path);
    shmem.with_extension("lock")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_path() {
        let n = normalize_path(Path::new("C:\\Users\\Test\\..\\Data\\.\\cache"));
        assert_eq!(n, "c:/users/data/cache");
    }

    #[test]
    fn test_normalize_path_truncation() {
        let long_path = "a/".repeat(200);
        let n = normalize_path(Path::new(&long_path));
        assert!(n.len() <= MAX_PATH_LENGTH);
    }

    #[test]
    fn test_shmem_name_from_path() {
        let name = shmem_name_from_path(Path::new("C:\\ProgramData\\Blizzard\\Agent"));
        assert!(name.starts_with("Global\\"));
        assert!(name.ends_with("/shmem"));
    }
}
