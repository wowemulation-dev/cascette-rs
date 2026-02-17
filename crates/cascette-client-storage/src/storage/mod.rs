//! Archive segment management and file access.
//!
//! This module handles the low-level storage layer:
//! - Archive segments with 480-byte headers
//! - Memory-mapped archive file access
//! - 30-byte local BLTE entry headers
//!
//! CASC organizes data into segments (up to 1023) that can be
//! individually frozen (read-only) or thawed (writable).

pub mod archive_file;
pub mod local_header;
pub mod segment;

pub use archive_file::ArchiveManager;
pub use local_header::LocalHeader;
pub use segment::{
    BUCKET_COUNT, DEFAULT_FILE_OFFSET_BITS, MAX_SEGMENTS, SEGMENT_HEADER_SIZE, SEGMENT_SIZE,
    SegmentHeader, SegmentInfo, SegmentState, bucket_hash, parse_data_filename, segment_data_path,
};
