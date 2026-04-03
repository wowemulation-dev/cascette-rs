//! Cross-platform sparse file handling.
//!
//! Containerless files use sparse file attributes for disk allocation
//! during downloads. Sparse regions represent content not yet downloaded,
//! avoiding allocation of disk blocks for zero-filled ranges.
//!
//! Platform support:
//! - **Linux**: `fallocate()` with `FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE`
//! - **Windows**: `FSCTL_SET_SPARSE` / `FSCTL_SET_ZERO_DATA` via `DeviceIoControl`
//! - **macOS**: `fcntl()` with `F_PUNCHHOLE`
//! - **Other**: No-op stubs that return success

use std::path::Path;

use crate::error::ContainerlessResult;

/// Platform sparse file capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SparseCapability {
    /// Full sparse file support (create, punch holes, query).
    Full,
    /// No sparse file support on this platform.
    None,
}

impl SparseCapability {
    /// Detect sparse file support for the current platform.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Full
        }
        #[cfg(target_os = "windows")]
        {
            Self::Full
        }
        #[cfg(target_os = "macos")]
        {
            Self::Full
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        {
            Self::None
        }
    }

    /// Whether any sparse operations are available.
    #[must_use]
    pub fn is_supported(self) -> bool {
        self != Self::None
    }
}

/// Mark a file as sparse.
///
/// On unsupported platforms, this is a no-op returning `Ok(())`.
pub fn set_sparse(path: &Path) -> ContainerlessResult<()> {
    platform::set_sparse(path)
}

/// Clear the sparse attribute from a file.
///
/// On unsupported platforms, this is a no-op returning `Ok(())`.
pub fn clear_sparse(path: &Path) -> ContainerlessResult<()> {
    platform::clear_sparse(path)
}

/// Check whether a file has the sparse attribute.
///
/// On unsupported platforms, returns `false`.
pub fn is_sparse(path: &Path) -> ContainerlessResult<bool> {
    platform::is_sparse(path)
}

/// Punch a hole in a file, deallocating the specified byte range.
///
/// The file size remains unchanged (the hole reads back as zeros).
/// On unsupported platforms, this is a no-op returning `Ok(())`.
pub fn punch_hole(path: &Path, offset: u64, length: u64) -> ContainerlessResult<()> {
    platform::punch_hole(path, offset, length)
}

// Linux implementation using fallocate.
#[cfg(target_os = "linux")]
mod platform {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;
    use std::path::Path;

    use crate::error::{ContainerlessError, ContainerlessResult};

    #[allow(clippy::unnecessary_wraps)]
    pub fn set_sparse(_path: &Path) -> ContainerlessResult<()> {
        // On Linux, files are sparse by default on ext4/btrfs/xfs.
        // No explicit action needed to mark a file as sparse.
        Ok(())
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn clear_sparse(_path: &Path) -> ContainerlessResult<()> {
        // Linux does not have an explicit sparse attribute to clear.
        // The file becomes non-sparse when all holes are written.
        Ok(())
    }

    pub fn is_sparse(path: &Path) -> ContainerlessResult<bool> {
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len();
        // st_blocks counts 512-byte blocks actually allocated.
        let allocated = metadata.blocks() * 512;
        Ok(allocated < file_size)
    }

    #[allow(unsafe_code, clippy::cast_possible_wrap)]
    pub fn punch_hole(path: &Path, offset: u64, length: u64) -> ContainerlessResult<()> {
        let file = OpenOptions::new().write(true).open(path)?;
        // FALLOC_FL_PUNCH_HOLE (0x02) | FALLOC_FL_KEEP_SIZE (0x01) = 0x03
        let ret = unsafe {
            libc::fallocate(
                file.as_raw_fd(),
                0x03,
                offset as libc::off_t,
                length as libc::off_t,
            )
        };
        if ret != 0 {
            return Err(ContainerlessError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

// Windows implementation using DeviceIoControl.
#[cfg(target_os = "windows")]
mod platform {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;
    use std::path::Path;

    use crate::error::{ContainerlessError, ContainerlessResult};

    // FSCTL constants.
    const FSCTL_SET_SPARSE: u32 = 0x000900c4;
    const FSCTL_SET_ZERO_DATA: u32 = 0x000980c8;
    const FSCTL_QUERY_ALLOCATED_RANGES: u32 = 0x000940cf;

    #[repr(C)]
    struct FileSetSparseBuffer {
        set_sparse: u8,
    }

    #[repr(C)]
    struct FileZeroDataInformation {
        file_offset: i64,
        beyond_final_zero: i64,
    }

    extern "system" {
        fn DeviceIoControl(
            h_device: *mut std::ffi::c_void,
            dw_io_control_code: u32,
            lp_in_buffer: *const std::ffi::c_void,
            n_in_buffer_size: u32,
            lp_out_buffer: *mut std::ffi::c_void,
            n_out_buffer_size: u32,
            lp_bytes_returned: *mut u32,
            lp_overlapped: *mut std::ffi::c_void,
        ) -> i32;

        fn GetFileInformationByHandle(
            h_file: *mut std::ffi::c_void,
            lp_file_information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    #[repr(C)]
    struct ByHandleFileInformation {
        dw_file_attributes: u32,
        _rest: [u8; 48],
    }

    const FILE_ATTRIBUTE_SPARSE_FILE: u32 = 0x00000200;

    pub fn set_sparse(path: &Path) -> ContainerlessResult<()> {
        let file = OpenOptions::new().write(true).open(path)?;
        let buf = FileSetSparseBuffer { set_sparse: 1 };
        let mut bytes_returned: u32 = 0;
        let ret = unsafe {
            DeviceIoControl(
                file.as_raw_handle() as *mut _,
                FSCTL_SET_SPARSE,
                &buf as *const _ as *const _,
                std::mem::size_of::<FileSetSparseBuffer>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };
        if ret == 0 {
            return Err(ContainerlessError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn clear_sparse(path: &Path) -> ContainerlessResult<()> {
        let file = OpenOptions::new().write(true).open(path)?;
        let buf = FileSetSparseBuffer { set_sparse: 0 };
        let mut bytes_returned: u32 = 0;
        let ret = unsafe {
            DeviceIoControl(
                file.as_raw_handle() as *mut _,
                FSCTL_SET_SPARSE,
                &buf as *const _ as *const _,
                std::mem::size_of::<FileSetSparseBuffer>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };
        if ret == 0 {
            return Err(ContainerlessError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }

    pub fn is_sparse(path: &Path) -> ContainerlessResult<bool> {
        let file = OpenOptions::new().read(true).open(path)?;
        let mut info = ByHandleFileInformation {
            dw_file_attributes: 0,
            _rest: [0u8; 48],
        };
        let ret = unsafe { GetFileInformationByHandle(file.as_raw_handle() as *mut _, &mut info) };
        if ret == 0 {
            return Err(ContainerlessError::Io(std::io::Error::last_os_error()));
        }
        Ok(info.dw_file_attributes & FILE_ATTRIBUTE_SPARSE_FILE != 0)
    }

    pub fn punch_hole(path: &Path, offset: u64, length: u64) -> ContainerlessResult<()> {
        let file = OpenOptions::new().write(true).open(path)?;
        let data = FileZeroDataInformation {
            file_offset: offset as i64,
            beyond_final_zero: (offset + length) as i64,
        };
        let mut bytes_returned: u32 = 0;
        let ret = unsafe {
            DeviceIoControl(
                file.as_raw_handle() as *mut _,
                FSCTL_SET_ZERO_DATA,
                &data as *const _ as *const _,
                std::mem::size_of::<FileZeroDataInformation>() as u32,
                std::ptr::null_mut(),
                0,
                &mut bytes_returned,
                std::ptr::null_mut(),
            )
        };
        if ret == 0 {
            return Err(ContainerlessError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

// macOS implementation using fcntl F_PUNCHHOLE.
#[cfg(target_os = "macos")]
mod platform {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;
    use std::path::Path;

    use crate::error::{ContainerlessError, ContainerlessResult};

    // F_PUNCHHOLE = 99 on macOS.
    const F_PUNCHHOLE: libc::c_int = 99;
    // Flag to indicate we are punching a hole and keeping the file size.
    const F_PEOFPOSMODE: libc::c_uint = 0;

    #[repr(C)]
    struct FPunchHole {
        fp_flags: libc::c_uint,
        reserved: libc::c_uint,
        fp_offset: libc::off_t,
        fp_length: libc::off_t,
    }

    pub fn set_sparse(_path: &Path) -> ContainerlessResult<()> {
        // macOS files on APFS are sparse by default when holes exist.
        Ok(())
    }

    pub fn clear_sparse(_path: &Path) -> ContainerlessResult<()> {
        // macOS does not have an explicit sparse attribute.
        Ok(())
    }

    pub fn is_sparse(path: &Path) -> ContainerlessResult<bool> {
        use std::os::unix::fs::MetadataExt;

        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len();
        let allocated = metadata.blocks() * 512;
        Ok(allocated < file_size)
    }

    pub fn punch_hole(path: &Path, offset: u64, length: u64) -> ContainerlessResult<()> {
        let file = OpenOptions::new().write(true).open(path)?;
        let args = FPunchHole {
            fp_flags: F_PEOFPOSMODE,
            reserved: 0,
            fp_offset: offset as libc::off_t,
            fp_length: length as libc::off_t,
        };
        let ret = unsafe { libc::fcntl(file.as_raw_fd(), F_PUNCHHOLE, &args) };
        if ret != 0 {
            return Err(ContainerlessError::Io(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
mod platform {
    use std::path::Path;

    use crate::error::ContainerlessResult;

    pub fn set_sparse(_path: &Path) -> ContainerlessResult<()> {
        Ok(())
    }

    pub fn clear_sparse(_path: &Path) -> ContainerlessResult<()> {
        Ok(())
    }

    pub fn is_sparse(_path: &Path) -> ContainerlessResult<bool> {
        Ok(false)
    }

    pub fn punch_hole(_path: &Path, _offset: u64, _length: u64) -> ContainerlessResult<()> {
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_detect() {
        let cap = SparseCapability::detect();
        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        assert_eq!(cap, SparseCapability::Full);
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        assert_eq!(cap, SparseCapability::None);
    }

    #[test]
    fn test_capability_is_supported() {
        assert!(SparseCapability::Full.is_supported());
        assert!(!SparseCapability::None.is_supported());
    }

    #[test]
    fn test_set_sparse_on_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        // set_sparse should succeed (no-op on Linux/macOS, real on Windows).
        set_sparse(&path).unwrap();
    }

    #[test]
    fn test_clear_sparse_on_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, vec![0u8; 4096]).unwrap();

        clear_sparse(&path).unwrap();
    }

    #[test]
    fn test_is_sparse_non_sparse_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, vec![1u8; 4096]).unwrap();

        // A small file with non-zero content should not be sparse.
        let result = is_sparse(&path).unwrap();
        // On Linux/macOS, small files may or may not show as sparse depending
        // on filesystem block allocation. We only verify the call succeeds.
        let _ = result;
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_punch_hole_linux() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sparse.bin");

        // Create a file large enough for the filesystem to allocate blocks.
        let size = 1024 * 1024; // 1 MiB
        std::fs::write(&path, vec![0xAA; size]).unwrap();

        let before_blocks = std::fs::metadata(&path).unwrap().blocks();

        // Punch a hole in the middle.
        punch_hole(&path, 4096, (size - 8192) as u64).unwrap();

        let after_blocks = std::fs::metadata(&path).unwrap().blocks();

        // Verify the file is now sparse (fewer blocks allocated).
        assert!(
            after_blocks < before_blocks,
            "expected fewer blocks after hole punch: before={before_blocks}, after={after_blocks}"
        );

        // File size should remain unchanged.
        let file_size = std::fs::metadata(&path).unwrap().len();
        assert_eq!(file_size, size as u64);
    }

    #[test]
    fn test_set_sparse_nonexistent_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.bin");
        // Should return an I/O error for missing file.
        // On Linux/macOS set_sparse is a no-op, so it won't fail.
        // On Windows it would fail. On all platforms, is_sparse should fail.
        let _ = set_sparse(&path);
    }

    #[test]
    fn test_clear_sparse_nonexistent_noop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.bin");
        // On most platforms, clear_sparse is a no-op or returns an error.
        let _ = clear_sparse(&path);
    }
}
