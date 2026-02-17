//! Archive segment header and tracking.
//!
//! Each archive `.data` file begins with a 480-byte segment header.
//! CASC supports up to 1023 segments per container.

/// Segment header size: 0x1E0 (480) bytes.
///
/// This is reserved at the start of each `.data` archive file.
pub const SEGMENT_HEADER_SIZE: usize = 0x1E0;

/// Maximum number of archive segments.
pub const MAX_SEGMENTS: u16 = 0x3FF;

/// Segment tracking entry size: 0x40 (64) bytes per segment.
pub const SEGMENT_TRACKING_SIZE: usize = 0x40;

/// Segment header at the start of each `.data` archive file.
///
/// The full layout is 480 bytes. Fields are populated during
/// segment initialization and updated during compaction.
#[derive(Debug, Clone)]
pub struct SegmentHeader {
    /// Raw header data (480 bytes).
    data: [u8; SEGMENT_HEADER_SIZE],
}

impl SegmentHeader {
    /// Create a new zeroed segment header.
    pub const fn new() -> Self {
        Self {
            data: [0u8; SEGMENT_HEADER_SIZE],
        }
    }

    /// Create from raw bytes.
    ///
    /// Returns `None` if the slice is shorter than 480 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < SEGMENT_HEADER_SIZE {
            return None;
        }
        let mut data = [0u8; SEGMENT_HEADER_SIZE];
        data.copy_from_slice(&bytes[..SEGMENT_HEADER_SIZE]);
        Some(Self { data })
    }

    /// Get the raw header data.
    pub const fn as_bytes(&self) -> &[u8; SEGMENT_HEADER_SIZE] {
        &self.data
    }
}

impl Default for SegmentHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Segment state: frozen (read-only) or thawed (writable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentState {
    /// Segment is frozen (read-only). No new data can be written.
    Frozen,
    /// Segment is thawed (writable). New data can be appended.
    Thawed,
}
