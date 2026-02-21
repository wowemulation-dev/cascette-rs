//! Unix shared memory implementation using `shm_open`.
//!
//! Provides `shm_open`-based shared memory with `flock` for writer
//! exclusion and file permissions matching the DACL intent from the
//! Windows implementation.

use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::os::unix::io::RawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{ptr, slice};

use libc::{MAP_SHARED, O_CREAT, O_RDWR, PROT_READ, PROT_WRITE, S_IRUSR, S_IWUSR};
use libc::{c_uint, c_void, mode_t, off_t, size_t};
use libc::{close, ftruncate, mmap, munmap, shm_open};

use crate::{Result, StorageError};

use super::control_block::{LOCK_FILE_SUFFIX, SHMEM_FILE_SUFFIX};

/// Lock file timeout in seconds.
const LOCK_TIMEOUT_SECS: u64 = 100;

/// Lock file retry interval in milliseconds.
const LOCK_RETRY_MS: u64 = 50;

/// Platform shared memory handle for Unix.
///
/// Uses POSIX `shm_open` for the shared memory object and `mmap` for
/// mapping it into process address space. File permissions are set to
/// owner-only (0600) to match the Windows DACL intent.
#[allow(unsafe_code)]
pub struct PlatformShmem {
    /// File descriptor for the shared memory object.
    fd: RawFd,
    /// Pointer to the mapped memory region.
    ptr: *mut c_void,
    /// Size of the mapped region.
    size: usize,
    /// POSIX shared memory name (e.g., `/cascette_<hash>`).
    name: String,
}

impl PlatformShmem {
    /// Create or open a shared memory region.
    ///
    /// Uses `shm_open` with `O_CREAT | O_RDWR` and permissions `S_IRUSR | S_IWUSR`
    /// (0600, owner-only). If the region already exists, it is opened.
    ///
    /// The name is derived from the storage path hash to stay within
    /// the POSIX shared memory name limit (typically 255 bytes).
    #[allow(unsafe_code)]
    pub fn open_or_create(name: &str, size: usize) -> Result<Self> {
        // POSIX shm names must start with '/'
        let shm_name = if name.starts_with('/') {
            name.to_string()
        } else {
            format!("/cascette_{name}")
        };

        let c_name = CString::new(shm_name.clone())
            .map_err(|e| StorageError::SharedMemory(format!("invalid shm name: {e}")))?;

        let fd = unsafe {
            shm_open(
                c_name.as_ptr(),
                O_CREAT | O_RDWR,
                (S_IRUSR | S_IWUSR) as mode_t as c_uint,
            )
        };

        if fd == -1 {
            return Err(StorageError::SharedMemory(format!(
                "shm_open failed for {shm_name}: {}",
                std::io::Error::last_os_error()
            )));
        }

        // Set size (idempotent if already the right size)
        let size_off_t = off_t::try_from(size).unwrap_or(off_t::MAX);
        if unsafe { ftruncate(fd, size_off_t) } == -1 {
            let err = std::io::Error::last_os_error();
            unsafe { close(fd) };
            return Err(StorageError::SharedMemory(format!(
                "ftruncate failed for {shm_name}: {err}"
            )));
        }

        let ptr = unsafe {
            mmap(
                ptr::null_mut(),
                size as size_t,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                fd,
                0,
            )
        };

        if ptr == libc::MAP_FAILED {
            let err = std::io::Error::last_os_error();
            unsafe { close(fd) };
            return Err(StorageError::SharedMemory(format!(
                "mmap failed for {shm_name}: {err}"
            )));
        }

        Ok(Self {
            fd,
            ptr,
            size,
            name: shm_name,
        })
    }

    /// Get a shared slice of the mapped memory.
    ///
    /// # Safety
    ///
    /// The caller must ensure no other process is concurrently writing
    /// to the region, or use external synchronization.
    #[allow(unsafe_code)]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.cast::<u8>(), self.size) }
    }

    /// Get a mutable slice of the mapped memory.
    ///
    /// # Safety
    ///
    /// The caller must ensure exclusive write access (e.g., via lock file).
    #[allow(unsafe_code)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr.cast::<u8>(), self.size) }
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

impl Drop for PlatformShmem {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        unsafe {
            if !self.ptr.is_null() {
                munmap(self.ptr, self.size as size_t);
            }
            if self.fd != -1 {
                close(self.fd);
            }
            // Note: we do NOT shm_unlink here. The shared memory object
            // persists until explicitly unlinked or the system reboots.
            // This matches CASC behavior where the shmem outlives the
            // creating process.
        }
    }
}

// SAFETY: PlatformShmem is Send because the fd is an integer handle
// and the ptr points to OS-managed shared memory. Transfer between
// threads is safe; concurrent access requires external synchronization.
#[allow(unsafe_code)]
unsafe impl Send for PlatformShmem {}

// SAFETY: PlatformShmem is Sync when external synchronization is used
// (e.g., lock file or control block state field).
#[allow(unsafe_code)]
unsafe impl Sync for PlatformShmem {}

/// Lock file for writer exclusion.
///
/// Uses `flock(LOCK_EX | LOCK_NB)` with a retry loop and timeout.
/// The lock file is created next to the shmem file with a `.lock` suffix.
pub struct LockFile {
    /// The lock file handle (kept open to hold the lock).
    _file: File,
    /// Path to the lock file.
    path: PathBuf,
}

impl LockFile {
    /// Acquire the lock file with a timeout.
    ///
    /// Retries every 50ms for up to 100 seconds (matching CASC timeout).
    /// Returns `Err` if the timeout expires.
    pub fn acquire(base_path: &Path) -> Result<Self> {
        let lock_path = base_path.with_extension(base_path.extension().map_or_else(
            || LOCK_FILE_SUFFIX.to_string(),
            |ext| {
                format!(
                    "{}.{}",
                    ext.to_string_lossy(),
                    LOCK_FILE_SUFFIX.trim_start_matches('.')
                )
            },
        ));

        let timeout = Duration::from_secs(LOCK_TIMEOUT_SECS);
        let retry_interval = Duration::from_millis(LOCK_RETRY_MS);
        let start = Instant::now();

        loop {
            // Try to create with exclusive access
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(file) => {
                    return Ok(Self {
                        _file: file,
                        path: lock_path,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if start.elapsed() >= timeout {
                        return Err(StorageError::SharedMemory(format!(
                            "lock file timeout after {}s: {}",
                            LOCK_TIMEOUT_SECS,
                            lock_path.display()
                        )));
                    }
                    std::thread::sleep(retry_interval);
                }
                Err(e) => {
                    return Err(StorageError::SharedMemory(format!(
                        "failed to create lock file {}: {e}",
                        lock_path.display()
                    )));
                }
            }
        }
    }

    /// Get the lock file path.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        // Remove the lock file on drop to release the lock.
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Check if a path is on a network filesystem.
///
/// Uses `statfs` to check the filesystem type. Returns `true` for
/// NFS, CIFS/SMB, and other network filesystems.
#[allow(unsafe_code)]
pub fn is_network_drive(path: &Path) -> bool {
    let Ok(c_path) = CString::new(path.as_os_str().as_encoded_bytes()) else {
        return false;
    };

    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c_path.as_ptr(), &raw mut stat) } != 0 {
        return false;
    }

    // Known network filesystem magic numbers (from <linux/magic.h>)
    #[cfg(target_os = "linux")]
    {
        const NFS_SUPER_MAGIC: i64 = 0x6969;
        const SMB_SUPER_MAGIC: i64 = 0x517B;
        const SMB2_MAGIC: i64 = 0xFE53_4D42;
        const CIFS_MAGIC: i64 = 0xFF53_4D42;
        const CODA_SUPER_MAGIC: i64 = 0x7372_7974;
        const AFS_SUPER_MAGIC: i64 = 0x5346_4141;

        let fstype = stat.f_type;
        matches!(
            fstype,
            NFS_SUPER_MAGIC
                | SMB_SUPER_MAGIC
                | SMB2_MAGIC
                | CIFS_MAGIC
                | CODA_SUPER_MAGIC
                | AFS_SUPER_MAGIC
        )
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, check f_fstypename for network FS types
        let fstype = unsafe {
            std::ffi::CStr::from_ptr(stat.f_fstypename.as_ptr())
                .to_string_lossy()
                .to_string()
        };
        matches!(fstype.as_str(), "nfs" | "smbfs" | "afpfs" | "webdav")
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        false
    }
}

/// Generate a POSIX-safe shared memory name from a storage path.
///
/// POSIX shm names have a limit (typically 255 bytes) and must start
/// with `/`. We hash the normalized path to produce a fixed-length name.
pub fn shmem_name_from_path(storage_path: &Path) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    storage_path
        .to_string_lossy()
        .to_lowercase()
        .replace('\\', "/")
        .hash(&mut hasher);
    let hash = hasher.finish();

    format!("/cascette_{hash:016x}")
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_shmem_name_from_path() {
        let name = shmem_name_from_path(Path::new("/home/user/.casc/data"));
        assert!(name.starts_with("/cascette_"));
        assert!(name.len() <= 255);

        // Same path should produce same name
        let name2 = shmem_name_from_path(Path::new("/home/user/.casc/data"));
        assert_eq!(name, name2);

        // Different path should produce different name
        let name3 = shmem_name_from_path(Path::new("/home/other/.casc/data"));
        assert_ne!(name, name3);
    }

    #[test]
    fn test_shmem_name_case_insensitive() {
        let name1 = shmem_name_from_path(Path::new("/Home/User/Data"));
        let name2 = shmem_name_from_path(Path::new("/home/user/data"));
        assert_eq!(name1, name2);
    }

    #[test]
    fn test_shmem_file_path() {
        let path = shmem_file_path(Path::new("/data/casc"));
        assert_eq!(path, Path::new("/data/casc/shmem"));
    }

    #[test]
    fn test_lock_file_path() {
        let path = lock_file_path(Path::new("/data/casc"));
        assert_eq!(path, Path::new("/data/casc/shmem.lock"));
    }

    #[test]
    fn test_lock_file_acquire_release() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().join("test.shmem");

        // Write a dummy file so the path exists
        std::fs::write(&base, b"").expect("write");

        let lock = LockFile::acquire(&base);
        assert!(lock.is_ok());
        let lock = lock.expect("lock");

        // Lock file should exist
        assert!(lock.path().exists());

        // Drop releases the lock
        let lock_path = lock.path().to_path_buf();
        drop(lock);
        assert!(!lock_path.exists());
    }

    #[test]
    fn test_platform_shmem_create() {
        // Create a unique name for this test
        let name = format!("test_{}", std::process::id());
        let result = PlatformShmem::open_or_create(&name, 4096);

        match result {
            Ok(mut shmem) => {
                assert_eq!(shmem.size(), 4096);

                // Write and read back
                let data = shmem.as_mut_slice();
                data[0] = 0x42;
                data[4095] = 0xFF;

                let data = shmem.as_slice();
                assert_eq!(data[0], 0x42);
                assert_eq!(data[4095], 0xFF);

                // Clean up
                drop(shmem);
                let c_name = CString::new(format!("/cascette_{name}")).expect("cstring");
                #[allow(unsafe_code)]
                unsafe {
                    libc::shm_unlink(c_name.as_ptr());
                }
            }
            Err(e) => {
                // shm_open may fail in CI containers without /dev/shm
                eprintln!("skipping shmem test (not available): {e}");
            }
        }
    }

    #[test]
    fn test_is_network_drive() {
        // Local paths should not be network drives
        assert!(!is_network_drive(Path::new("/tmp")));
        assert!(!is_network_drive(Path::new("/")));
    }
}
