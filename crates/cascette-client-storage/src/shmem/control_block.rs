//! Shared memory control block for the v4/v5 protocol.
//!
//! The control block is at the start of the shared memory region and
//! contains the free space table, PID tracking slots, and version
//! information.

/// Minimum supported shmem protocol version.
pub const MIN_SHMEM_VERSION: u8 = 4;

/// Maximum supported shmem protocol version.
pub const MAX_SHMEM_VERSION: u8 = 5;

/// Required free space table format identifier.
pub const FREE_SPACE_TABLE_FORMAT: u32 = 0x2AB8;

/// Offset of the initialization byte.
pub const INIT_BYTE_OFFSET: usize = 0x02;

/// Offset of the free space table format DWORD.
pub const FREE_SPACE_FORMAT_OFFSET: usize = 0x42;

/// Offset of the data size field.
pub const DATA_SIZE_OFFSET: usize = 0x43;

/// Offset of the V5 exclusive access flag.
pub const V5_EXCLUSIVE_FLAG_OFFSET: usize = 0x54;

/// Shared memory control block.
///
/// Manages the shared memory region header including version checks,
/// free space table, and PID tracking.
#[derive(Debug)]
pub struct ShmemControlBlock {
    /// Protocol version (4 or 5).
    version: u8,
    /// Whether the control block has been initialized.
    initialized: bool,
    /// Free space table format (must be 0x2AB8).
    free_space_format: u32,
    /// Data size in the shared memory region.
    data_size: u32,
    /// V5: exclusive access flag (bit 0 at offset 0x54).
    exclusive_access: bool,
}

impl ShmemControlBlock {
    /// Create a new control block with the given protocol version.
    ///
    /// Returns `None` if the version is not in [4, 5].
    pub fn new(version: u8) -> Option<Self> {
        if !(MIN_SHMEM_VERSION..=MAX_SHMEM_VERSION).contains(&version) {
            return None;
        }

        Some(Self {
            version,
            initialized: false,
            free_space_format: FREE_SPACE_TABLE_FORMAT,
            data_size: 0,
            exclusive_access: false,
        })
    }

    /// Get the protocol version.
    pub const fn version(&self) -> u8 {
        self.version
    }

    /// Check if exclusive access is set (V5 only).
    pub const fn is_exclusive(&self) -> bool {
        self.version >= 5 && self.exclusive_access
    }

    /// Set exclusive access flag (V5 only).
    pub fn set_exclusive(&mut self, exclusive: bool) {
        if self.version >= 5 {
            self.exclusive_access = exclusive;
        }
    }

    /// Validate the control block state.
    ///
    /// Checks:
    /// - Initialization byte is non-zero
    /// - Free space table format is 0x2AB8
    /// - Data size is non-zero
    pub const fn validate(&self) -> bool {
        self.initialized && self.free_space_format == FREE_SPACE_TABLE_FORMAT && self.data_size > 0
    }

    /// Initialize the control block.
    pub fn initialize(&mut self, data_size: u32) {
        self.initialized = true;
        self.data_size = data_size;
        self.free_space_format = FREE_SPACE_TABLE_FORMAT;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_validation() {
        assert!(ShmemControlBlock::new(3).is_none());
        assert!(ShmemControlBlock::new(4).is_some());
        assert!(ShmemControlBlock::new(5).is_some());
        assert!(ShmemControlBlock::new(6).is_none());
    }

    #[test]
    fn test_exclusive_access_v5_only() {
        let mut v4 = ShmemControlBlock::new(4).unwrap();
        v4.set_exclusive(true);
        assert!(!v4.is_exclusive()); // V4 ignores exclusive flag

        let mut v5 = ShmemControlBlock::new(5).unwrap();
        v5.set_exclusive(true);
        assert!(v5.is_exclusive());
    }

    #[test]
    fn test_validation() {
        let mut cb = ShmemControlBlock::new(4).unwrap();
        assert!(!cb.validate()); // Not initialized

        cb.initialize(1024);
        assert!(cb.validate());
    }
}
