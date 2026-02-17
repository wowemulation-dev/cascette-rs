//! Shared memory control block for the v4/v5 protocol.
//!
//! The control block is at the start of the shared memory region and
//! contains the free space table, PID tracking slots, and version
//! information.
//!
/// Minimum supported shmem protocol version.
pub const MIN_SHMEM_VERSION: u8 = 4;

/// Maximum supported shmem protocol version.
pub const MAX_SHMEM_VERSION: u8 = 5;

/// Required free space table format identifier.
///
/// Stored at dword offset 0x42 in the control block.
/// CASC rejects any other value: "Detected unsupported free space
/// table format".
pub const FREE_SPACE_TABLE_FORMAT: u32 = 0x2AB8;

/// Free space table size in bytes (same as format identifier).
pub const FREE_SPACE_TABLE_SIZE: usize = 0x2AB8;

/// Offset of the initialization byte.
pub const INIT_BYTE_OFFSET: usize = 0x02;

/// Offset of the free space table format DWORD.
pub const FREE_SPACE_FORMAT_OFFSET: usize = 0x42;

/// Offset of the data size field (DWORD).
pub const DATA_SIZE_OFFSET: usize = 0x43;

/// Offset of the V5 exclusive access flag (DWORD).
pub const V5_EXCLUSIVE_FLAG_OFFSET: usize = 0x54;

/// V4 control block header size in bytes.
pub const V4_HEADER_SIZE: usize = 0x150;

/// V5 base control block header size (without PID tracking).
pub const V5_BASE_HEADER_SIZE: usize = 0x154;

/// V5 extended header size (with PID tracking enabled).
pub const V5_EXTENDED_HEADER_SIZE: usize = 0x258;

/// PID tracking slot size in bytes.
///
/// Each PID slot is 0x228 (552) bytes .
pub const PID_SLOT_SIZE: usize = 0x228;

/// V4 alignment: 16 bytes.
pub const V4_ALIGNMENT: usize = 16;

/// V5 alignment: 4096 bytes (page-aligned).
pub const V5_ALIGNMENT: usize = 4096;

/// Shmem file suffix.
pub const SHMEM_FILE_SUFFIX: &str = ".shmem";

/// Lock file suffix.
pub const LOCK_FILE_SUFFIX: &str = ".shmem.lock";

/// Align a size to the protocol version's alignment boundary.
///
/// /// - v4: `(size + 0xF) & !0xF` (16-byte alignment)
/// - v5: `(size + 0xFFF) & !0xFFF` (page alignment)
pub const fn align_size(size: usize, version: u8) -> usize {
    if version == 4 {
        (size + 0xF) & !0xF
    } else {
        (size + 0xFFF) & !0xFFF
    }
}

/// Calculate the total shmem file size for v4.
///
/// v4: `align16(align16(0x150) + 0x2AB8)` = 0x2C10 bytes.
pub const fn v4_file_size() -> usize {
    let header = align_size(V4_HEADER_SIZE, 4);
    align_size(header + FREE_SPACE_TABLE_SIZE, 4)
}

/// Calculate the total shmem file size for v5.
///
/// Base: `page_align(0x154 + 0x2AB8)`.
/// With PID tracking: `page_align(base + PID_SLOT_SIZE)`.
pub const fn v5_file_size(pid_tracking: bool) -> usize {
    let header = if pid_tracking {
        V5_EXTENDED_HEADER_SIZE
    } else {
        V5_BASE_HEADER_SIZE
    };
    let base = align_size(header + FREE_SPACE_TABLE_SIZE, 5);
    if pid_tracking {
        align_size(base + PID_SLOT_SIZE, 5)
    } else {
        base
    }
}

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
    /// V5: PID tracking enabled.
    pid_tracking: Option<PidTracking>,
}

/// PID tracking state for v5 shmem.
///
/// - State 1 = idle/valid, State 2 = modifying
/// - Writer count = non-readonly processes
/// - Total count = all bound processes
/// - Generation = 64-bit counter incremented on add
#[derive(Debug, Clone)]
pub struct PidTracking {
    /// State: 1 = idle, 2 = modifying.
    pub state: u32,
    /// Number of non-readonly processes.
    pub writer_count: u32,
    /// Total bound processes.
    pub total_count: u32,
    /// Last modified slot index.
    pub last_modified_slot: u32,
    /// 64-bit generation counter.
    pub generation: u64,
    /// Maximum number of PID slots.
    pub max_slots: u32,
    /// PID values per slot (0 = empty).
    pub pids: Vec<u32>,
    /// Access mode per slot (2 = read-only).
    pub modes: Vec<u32>,
}

impl PidTracking {
    /// Create a new PID tracking state with the given slot count.
    pub fn new(max_slots: u32) -> Self {
        Self {
            state: 1,
            writer_count: 0,
            total_count: 0,
            last_modified_slot: 0,
            generation: 0,
            max_slots,
            pids: vec![0; max_slots as usize],
            modes: vec![0; max_slots as usize],
        }
    }

    /// Check if the PID tracking state is valid.
    ///
    /// Valid if state is 1 (idle) or 2 (modifying).
    pub const fn is_valid(&self) -> bool {
        self.state == 1 || self.state == 2
    }

    /// Add a process to the tracking table.
    ///
    /// Returns the slot index, or `None` if no slots are available.
    ///
    /// 1. If state == 2, recount (recovery from crash)
    /// 2. Find first empty slot (PID == 0)
    /// 3. Set state to 2 (modifying)
    /// 4. Write PID and mode
    /// 5. Increment counters
    /// 6. Set state to 1
    pub fn add_process(&mut self, pid: u32, mode: u32) -> Option<usize> {
        if self.state == 2 {
            self.recount();
        }

        if self.total_count >= self.max_slots {
            return None;
        }

        // Find first empty slot
        let slot = self.pids.iter().position(|&p| p == 0)?;

        self.state = 2;
        self.pids[slot] = pid;
        self.modes[slot] = mode;
        self.last_modified_slot = slot as u32;

        self.total_count += 1;
        if mode != 2 {
            // Not read-only
            self.writer_count += 1;
        }
        self.generation += 1;
        self.state = 1;

        Some(slot)
    }

    /// Remove a process from the tracking table.
    ///
    pub fn remove_process(&mut self, pid: u32) -> bool {
        if self.state == 2 {
            self.recount();
        }

        let Some(slot) = self.pids.iter().position(|&p| p == pid) else {
            return false;
        };

        self.state = 2;
        let mode = self.modes[slot];
        self.pids[slot] = 0;
        self.modes[slot] = 0;

        self.total_count = self.total_count.saturating_sub(1);
        if mode != 2 {
            self.writer_count = self.writer_count.saturating_sub(1);
        }
        self.state = 1;

        true
    }

    /// Recount live processes, clearing dead slots.
    ///
    pub fn recount(&mut self) {
        // Clear the last modified slot (it may be corrupt)
        if (self.last_modified_slot as usize) < self.pids.len() {
            self.pids[self.last_modified_slot as usize] = 0;
            self.modes[self.last_modified_slot as usize] = 0;
        }

        let mut total = 0u32;
        let mut writers = 0u32;
        for i in 0..self.max_slots as usize {
            if self.pids[i] != 0 {
                total += 1;
                if self.modes[i] != 2 {
                    writers += 1;
                }
            }
        }

        self.total_count = total;
        self.writer_count = writers;
        self.state = 1;
    }
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
            pid_tracking: None,
        })
    }

    /// Create a v5 control block with PID tracking enabled.
    pub fn new_v5_with_pid_tracking(max_slots: u32) -> Self {
        Self {
            version: 5,
            initialized: false,
            free_space_format: FREE_SPACE_TABLE_FORMAT,
            data_size: 0,
            exclusive_access: false,
            pid_tracking: Some(PidTracking::new(max_slots)),
        }
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

    /// Get PID tracking state (V5 only).
    pub fn pid_tracking(&self) -> Option<&PidTracking> {
        self.pid_tracking.as_ref()
    }

    /// Get mutable PID tracking state (V5 only).
    pub fn pid_tracking_mut(&mut self) -> Option<&mut PidTracking> {
        self.pid_tracking.as_mut()
    }

    /// Validate the control block state.
    ///
    /// - Initialization byte must be non-zero
    /// - Free space table format must be 0x2AB8
    /// - Data size must be non-zero
    /// - V5: exclusive access flag must not be set (for non-exclusive bind)
    pub const fn validate(&self) -> bool {
        self.initialized && self.free_space_format == FREE_SPACE_TABLE_FORMAT && self.data_size > 0
    }

    /// Validate for binding (includes exclusive access check).
    ///
    /// Returns an error message if validation fails, matching CASC
    /// error strings.
    pub fn validate_for_bind(&self) -> Result<(), &'static str> {
        if !(MIN_SHMEM_VERSION..=MAX_SHMEM_VERSION).contains(&self.version) {
            return Err("Unsupported shmem protocol version");
        }

        if self.free_space_format != FREE_SPACE_TABLE_FORMAT {
            return Err("Detected unsupported free space table format");
        }

        if self.version >= 5 && self.exclusive_access {
            return Err("Unable to bind container while another process has exclusive access");
        }

        if !self.initialized || self.data_size == 0 {
            return Err("Detected invalid shared memory initialization");
        }

        Ok(())
    }

    /// Initialize the control block.
    pub fn initialize(&mut self, data_size: u32) {
        self.initialized = true;
        self.data_size = data_size;
        self.free_space_format = FREE_SPACE_TABLE_FORMAT;
    }

    /// Get the total file size for this control block.
    pub const fn file_size(&self) -> usize {
        if self.version == 4 {
            v4_file_size()
        } else {
            v5_file_size(self.pid_tracking.is_some())
        }
    }

    /// Get the alignment for this protocol version.
    pub const fn alignment(&self) -> usize {
        if self.version == 4 {
            V4_ALIGNMENT
        } else {
            V5_ALIGNMENT
        }
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

    #[test]
    fn test_validate_for_bind() {
        let mut cb = ShmemControlBlock::new(4).unwrap();
        cb.initialize(1024);
        assert!(cb.validate_for_bind().is_ok());

        // V5 exclusive access blocks binding
        let mut v5 = ShmemControlBlock::new(5).unwrap();
        v5.initialize(1024);
        v5.set_exclusive(true);
        assert!(v5.validate_for_bind().is_err());
    }

    #[test]
    fn test_alignment() {
        assert_eq!(align_size(100, 4), 112); // ceil to 16
        assert_eq!(align_size(100, 5), 4096); // ceil to page

        assert_eq!(align_size(16, 4), 16); // exact 16
        assert_eq!(align_size(4096, 5), 4096); // exact page
    }

    #[test]
    fn test_v4_file_size() {
        // V4: align16(align16(0x150) + 0x2AB8) = align16(0x150 + 0x2AB8) = align16(0x2C08) = 0x2C10
        assert_eq!(v4_file_size(), 0x2C10);
    }

    #[test]
    fn test_v5_file_size() {
        // V5 base: page_align(0x154 + 0x2AB8)
        let base = v5_file_size(false);
        assert_eq!(base % V5_ALIGNMENT, 0); // Must be page-aligned

        // V5 with PID tracking should be larger
        let with_pid = v5_file_size(true);
        assert!(with_pid > base);
        assert_eq!(with_pid % V5_ALIGNMENT, 0);
    }

    #[test]
    fn test_pid_tracking_add_remove() {
        let mut pt = PidTracking::new(4);

        // Add a process
        let slot = pt.add_process(1234, 5).unwrap(); // RW mode
        assert_eq!(slot, 0);
        assert_eq!(pt.total_count, 1);
        assert_eq!(pt.writer_count, 1);
        assert_eq!(pt.generation, 1);

        // Add another (read-only)
        let slot2 = pt.add_process(5678, 2).unwrap(); // RO mode
        assert_eq!(slot2, 1);
        assert_eq!(pt.total_count, 2);
        assert_eq!(pt.writer_count, 1); // Only first is a writer

        // Remove first
        assert!(pt.remove_process(1234));
        assert_eq!(pt.total_count, 1);
        assert_eq!(pt.writer_count, 0);

        // Remove non-existent
        assert!(!pt.remove_process(9999));
    }

    #[test]
    fn test_pid_tracking_full() {
        let mut pt = PidTracking::new(2);

        pt.add_process(1, 5).unwrap();
        pt.add_process(2, 5).unwrap();

        // Table full
        assert!(pt.add_process(3, 5).is_none());
    }

    #[test]
    fn test_pid_tracking_recount() {
        let mut pt = PidTracking::new(4);
        pt.pids[0] = 100;
        pt.modes[0] = 5;
        pt.pids[2] = 200;
        pt.modes[2] = 2;
        pt.state = 2; // Simulate crash during modify
        pt.last_modified_slot = 2; // This slot gets cleared

        pt.recount();

        assert_eq!(pt.state, 1);
        assert_eq!(pt.total_count, 1); // Only slot 0 survives
        assert_eq!(pt.writer_count, 1);
        assert_eq!(pt.pids[2], 0); // Cleared by recount
    }

    #[test]
    fn test_pid_tracking_is_valid() {
        let pt = PidTracking::new(4);
        assert!(pt.is_valid()); // State 1

        let mut pt2 = PidTracking::new(4);
        pt2.state = 2;
        assert!(pt2.is_valid()); // State 2

        let mut pt3 = PidTracking::new(4);
        pt3.state = 0;
        assert!(!pt3.is_valid()); // Invalid
    }

    #[test]
    fn test_new_v5_with_pid_tracking() {
        let cb = ShmemControlBlock::new_v5_with_pid_tracking(8);
        assert_eq!(cb.version(), 5);
        assert!(cb.pid_tracking().is_some());
        assert_eq!(cb.pid_tracking().unwrap().max_slots, 8);
    }
}
