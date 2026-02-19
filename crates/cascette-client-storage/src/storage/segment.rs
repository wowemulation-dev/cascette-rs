//! Archive segment header and tracking.
//!
//! Each archive `.data` file begins with a 480-byte segment header block
//! consisting of 16 reconstruction headers (one per KMT bucket). These
//! headers allow the key mapping to be rebuilt from data files alone.
//!

use super::local_header::{LOCAL_HEADER_SIZE, LocalHeader};

/// Segment header block size: 0x1E0 (480) bytes = 16 × 30-byte headers.
pub const SEGMENT_HEADER_SIZE: usize = 0x1E0;

/// Maximum number of archive segments (0x3FF = 1023).
pub const MAX_SEGMENTS: u16 = 0x3FF;

/// Number of KMT buckets (one reconstruction header per bucket).
pub const BUCKET_COUNT: usize = 16;

/// Segment size: 1 GiB (0x40000000 bytes).
///
/// Each segment occupies this much space in the virtual address range.
/// The base offset of segment N is `N * SEGMENT_SIZE`.
pub const SEGMENT_SIZE: u64 = 0x4000_0000;

/// Default file offset bits (30), matching `FileOffsetBits` in IDX v7 headers.
pub const DEFAULT_FILE_OFFSET_BITS: u8 = 30;

/// Segment header block at the start of each `.data` archive file.
///
/// Contains 16 reconstruction headers, one per KMT bucket. Each is a
/// 30-byte `LocalHeader` with a generated key that hashes to the
/// corresponding bucket index.
#[derive(Debug, Clone)]
pub struct SegmentHeader {
    /// The 16 reconstruction headers (one per bucket).
    headers: [LocalHeader; BUCKET_COUNT],
}

impl SegmentHeader {
    /// Create a new segment header with generated keys for a segment.
    ///
    /// `segment_index` is the segment number (0-1022).
    /// `path_hash` is the 16-byte hash of the storage path.
    pub fn generate(segment_index: u16, path_hash: &[u8; 16]) -> Self {
        let mut headers = std::array::from_fn(|_| LocalHeader::new([0u8; 16], 0));

        for (bucket, header) in headers.iter_mut().enumerate() {
            let key = generate_segment_key(path_hash, segment_index, bucket as u8);
            *header = LocalHeader::new(key, 0);
        }

        Self { headers }
    }

    /// Create a zeroed segment header (for new/empty segments).
    pub fn zeroed() -> Self {
        Self {
            headers: std::array::from_fn(|_| LocalHeader::new([0u8; 16], 0)),
        }
    }

    /// Parse a segment header from 480 bytes.
    ///
    /// Returns `None` if the data is too short.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < SEGMENT_HEADER_SIZE {
            return None;
        }

        let mut headers = std::array::from_fn(|_| LocalHeader::new([0u8; 16], 0));

        for (i, header) in headers.iter_mut().enumerate() {
            let offset = i * LOCAL_HEADER_SIZE;
            if let Some(parsed) = LocalHeader::from_bytes(&data[offset..offset + LOCAL_HEADER_SIZE])
            {
                *header = parsed;
            }
        }

        Some(Self { headers })
    }

    /// Serialize the segment header to 480 bytes.
    pub fn to_bytes(&self) -> [u8; SEGMENT_HEADER_SIZE] {
        let mut buf = [0u8; SEGMENT_HEADER_SIZE];

        for (i, header) in self.headers.iter().enumerate() {
            let offset = i * LOCAL_HEADER_SIZE;
            buf[offset..offset + LOCAL_HEADER_SIZE].copy_from_slice(&header.to_bytes());
        }

        buf
    }

    /// Get the reconstruction header for a specific bucket.
    pub fn bucket_header(&self, bucket: u8) -> &LocalHeader {
        &self.headers[bucket as usize & 0x0F]
    }

    /// Get the encoding key for a specific bucket's reconstruction header.
    ///
    /// Returns the original (non-reversed) key.
    pub fn bucket_key(&self, bucket: u8) -> [u8; 16] {
        self.headers[bucket as usize & 0x0F].original_encoding_key()
    }
}

impl Default for SegmentHeader {
    fn default() -> Self {
        Self::zeroed()
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

/// Information about a single archive segment.
#[derive(Debug, Clone)]
pub struct SegmentInfo {
    /// Segment index (0-1022).
    pub index: u16,
    /// Current state.
    pub state: SegmentState,
    /// Current write position within the segment.
    pub write_position: u64,
    /// Segment header (480 bytes at start of data file).
    pub header: SegmentHeader,
}

impl SegmentInfo {
    /// Create a new segment info entry.
    pub fn new(index: u16, header: SegmentHeader) -> Self {
        Self {
            index,
            state: SegmentState::Thawed,
            write_position: SEGMENT_HEADER_SIZE as u64,
            header,
        }
    }

    /// Get the base offset for this segment in the StorageOffset encoding.
    pub fn base_offset(&self) -> u64 {
        u64::from(self.index) * SEGMENT_SIZE
    }

    /// Check if new data of the given size fits in this segment.
    pub fn has_space_for(&self, size: u64) -> bool {
        self.state == SegmentState::Thawed && self.write_position + size <= SEGMENT_SIZE
    }
}

/// Compute the bucket index for a 9-byte EKey.
///
/// XOR all 9 bytes, then `(((xor >> 4) ^ xor) + seed) & 0x0F`.
///
/// For standard lookups, `seed` is 0. For segment header key generation,
/// `seed` is 1.
pub fn bucket_hash(ekey: &[u8], seed: u8) -> u8 {
    let mut xor: u8 = 0;
    for &b in ekey.iter().take(9) {
        xor ^= b;
    }
    ((xor >> 4) ^ xor).wrapping_add(seed) & 0x0F
}

/// Generate a 16-byte key for a segment reconstruction header.
///
/// - Start with the 16-byte path hash as base
/// - Encode segment count in bytes \[1\] and \[2\] (big-endian u16)
/// - Adjust byte \[0\] (0x00-0xFF) until the first 9 bytes hash to
///   the target bucket via `bucket_hash` with seed 1
///
/// Called 16 times per segment (once per bucket) by
/// `casc::ContainerIndex::GenerateSegmentHeaders`.
fn generate_segment_key(path_hash: &[u8; 16], segment_count: u16, target_bucket: u8) -> [u8; 16] {
    let mut key = *path_hash;

    // Encode segment count in bytes 1-2 (big-endian)
    key[1] = (segment_count & 0xFF) as u8;
    key[2] = ((segment_count >> 8) & 0xFF) as u8;

    // Brute-force byte[0] until bucket_hash matches target_bucket
    for probe in 0..=0xFFu8 {
        key[0] = probe;
        if bucket_hash(&key[..9], 1) == target_bucket {
            return key;
        }
    }

    // Fallback (should not happen for 4-bit bucket space)
    key[0] = 0;
    key
}

/// Generate a data file path for a segment.
///
/// CASC uses `data.XXXX` naming (3-4 digits).
/// From `casc::DynamicStorage::EnumerateArchiveSegments`, filenames are
/// validated as `data.` followed by 3 or 4 ASCII digits.
pub fn segment_data_path(base_dir: &std::path::Path, segment_index: u16) -> std::path::PathBuf {
    base_dir.join(format!("data.{segment_index:03}"))
}

/// Parse a segment index from a data filename.
///
/// Accepts `data.NNN` or `data.NNNN` where N are ASCII digits.
/// Returns `None` if the filename doesn't match or the index is >= MAX_SEGMENTS.
pub fn parse_data_filename(filename: &str) -> Option<u16> {
    let suffix = filename.strip_prefix("data.")?;

    // Must be 3 or 4 digits
    if (suffix.len() != 3 && suffix.len() != 4) || !suffix.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let index: u16 = suffix.parse().ok()?;
    if index >= MAX_SEGMENTS {
        return None;
    }

    Some(index)
}

/// Encode a segment index and file offset into a 5-byte StorageOffset.
///
/// The StorageOffset packs two values using FileOffsetBits (default 30):
/// - Upper bits: segment index
/// - Lower FileOffsetBits bits: byte offset within the segment
pub fn encode_storage_offset(segment_index: u16, file_offset: u32) -> (u16, u32) {
    // archive_id = segment_index, archive_offset = file_offset
    // This maps directly to ArchiveLocation fields
    (segment_index, file_offset)
}

/// Decode a StorageOffset into segment index and file offset.
pub fn decode_storage_offset(archive_id: u16, archive_offset: u32) -> (u16, u32) {
    (archive_id, archive_offset)
}

/// Allocation result from `SegmentAllocator::allocate`.
#[derive(Debug, Clone, Copy)]
pub struct Allocation {
    /// Index of the segment that was allocated from.
    pub segment_index: u16,
    /// Byte offset within the segment's data file.
    pub file_offset: u32,
}

/// Manages archive segments for the write path.
///
/// Tracks thawed (writable) and frozen (read-only) segments.
/// Allocation tries thawed segments first, creates new ones when
/// all are full, and enforces `MAX_SEGMENTS`.
pub struct SegmentAllocator {
    /// All known segments, indexed by segment index.
    segments: Vec<SegmentInfo>,
    /// Per-bucket RwLock for concurrent KMT access.
    ///
    /// Each bucket's index file can be flushed independently.
    bucket_locks: [parking_lot::RwLock<()>; BUCKET_COUNT],
    /// Maximum number of segments allowed.
    max_segments: u16,
    /// Path hash for generating segment header keys.
    path_hash: [u8; 16],
    /// Base directory for data files.
    base_path: std::path::PathBuf,
}

impl SegmentAllocator {
    /// Create a new segment allocator.
    pub fn new(base_path: std::path::PathBuf, path_hash: [u8; 16], max_segments: u16) -> Self {
        Self {
            segments: Vec::new(),
            bucket_locks: std::array::from_fn(|_| parking_lot::RwLock::new(())),
            max_segments: max_segments.min(MAX_SEGMENTS),
            path_hash,
            base_path,
        }
    }

    /// Load existing segments from the data directory.
    ///
    /// Scans for `data.NNN` files, parses their segment headers,
    /// and marks all loaded segments as frozen.
    pub fn load_existing(&mut self) -> crate::Result<()> {
        let entries = match std::fs::read_dir(&self.base_path) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(crate::StorageError::Archive(format!(
                    "failed to read data directory {}: {e}",
                    self.base_path.display()
                )));
            }
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else {
                continue;
            };
            let Some(index) = parse_data_filename(name_str) else {
                continue;
            };

            let path = entry.path();
            let metadata = std::fs::metadata(&path).map_err(|e| {
                crate::StorageError::Archive(format!("failed to stat {}: {e}", path.display()))
            })?;

            // Read segment header
            let file_data = std::fs::read(&path).map_err(|e| {
                crate::StorageError::Archive(format!("failed to read {}: {e}", path.display()))
            })?;

            let header = if file_data.len() >= SEGMENT_HEADER_SIZE {
                SegmentHeader::from_bytes(&file_data).unwrap_or_default()
            } else {
                SegmentHeader::default()
            };

            let mut info = SegmentInfo::new(index, header);
            info.write_position = metadata.len();
            info.state = SegmentState::Frozen; // All loaded segments start frozen

            // Insert at the right position, expanding if needed
            while self.segments.len() <= index as usize {
                self.segments.push(SegmentInfo::new(
                    self.segments.len() as u16,
                    SegmentHeader::default(),
                ));
            }
            self.segments[index as usize] = info;
        }

        Ok(())
    }

    /// Allocate space in a segment for `size` bytes.
    ///
    /// Strategy:
    /// 1. Try thawed segments in order
    /// 2. If none have space, create a new segment
    /// 3. Returns error if MAX_SEGMENTS reached
    pub fn allocate(&mut self, size: u64) -> crate::Result<Allocation> {
        // Try existing thawed segments
        for info in &mut self.segments {
            if info.has_space_for(size) {
                let offset = info.write_position;
                info.write_position += size;
                return Ok(Allocation {
                    segment_index: info.index,
                    file_offset: u32::try_from(offset).map_err(|_| {
                        crate::StorageError::Archive("segment offset exceeds u32 range".to_string())
                    })?,
                });
            }
        }

        // Create new segment
        let new_index = u16::try_from(self.segments.len())
            .map_err(|_| crate::StorageError::Archive("too many segments".to_string()))?;

        if new_index >= self.max_segments {
            return Err(crate::StorageError::Archive(format!(
                "maximum segment count ({}) reached",
                self.max_segments
            )));
        }

        let header = SegmentHeader::generate(new_index, &self.path_hash);
        let header_bytes = header.to_bytes();

        // Write the segment header to the new data file
        let data_path = segment_data_path(&self.base_path, new_index);
        std::fs::write(&data_path, header_bytes).map_err(|e| {
            crate::StorageError::Archive(format!(
                "failed to create segment file {}: {e}",
                data_path.display()
            ))
        })?;

        let mut info = SegmentInfo::new(new_index, header);
        // write_position starts after the header (set by SegmentInfo::new)

        let offset = info.write_position;
        info.write_position += size;
        self.segments.push(info);

        Ok(Allocation {
            segment_index: new_index,
            file_offset: u32::try_from(offset).map_err(|_| {
                crate::StorageError::Archive("segment offset exceeds u32 range".to_string())
            })?,
        })
    }

    /// Freeze a segment (make it read-only).
    pub fn freeze(&mut self, segment_index: u16) -> bool {
        if let Some(info) = self.segments.get_mut(segment_index as usize)
            && info.state == SegmentState::Thawed
        {
            info.state = SegmentState::Frozen;
            return true;
        }
        false
    }

    /// Thaw a segment (make it writable).
    pub fn thaw(&mut self, segment_index: u16) -> bool {
        if let Some(info) = self.segments.get_mut(segment_index as usize)
            && info.state == SegmentState::Frozen
        {
            info.state = SegmentState::Thawed;
            return true;
        }
        false
    }

    /// Get the number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Get segment info by index.
    pub fn segment(&self, index: u16) -> Option<&SegmentInfo> {
        self.segments.get(index as usize)
    }

    /// Get mutable segment info by index.
    pub fn segment_mut(&mut self, index: u16) -> Option<&mut SegmentInfo> {
        self.segments.get_mut(index as usize)
    }

    /// Acquire a bucket lock for KMT operations.
    ///
    /// Returns a guard that releases the lock on drop.
    pub fn bucket_lock(&self, bucket: u8) -> parking_lot::RwLockReadGuard<'_, ()> {
        self.bucket_locks[(bucket & 0x0F) as usize].read()
    }

    /// Acquire a bucket write lock for KMT flush operations.
    pub fn bucket_write_lock(&self, bucket: u8) -> parking_lot::RwLockWriteGuard<'_, ()> {
        self.bucket_locks[(bucket & 0x0F) as usize].write()
    }

    /// Iterate over all segments.
    pub fn segments(&self) -> &[SegmentInfo] {
        &self.segments
    }
}

impl std::fmt::Debug for SegmentAllocator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SegmentAllocator")
            .field("segment_count", &self.segments.len())
            .field("max_segments", &self.max_segments)
            .field("base_path", &self.base_path)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_bucket_hash_basic() {
        // All zeros with seed 0 → bucket 0
        let key = [0u8; 9];
        assert_eq!(bucket_hash(&key, 0), 0);

        // All zeros with seed 1 → bucket 1
        assert_eq!(bucket_hash(&key, 1), 1);
    }

    #[test]
    fn test_bucket_hash_matches_index_manager() {
        // The IndexManager::get_bucket_index uses the same algorithm with seed 0
        // Verify: XOR all 9 bytes, then ((xor >> 4) ^ xor) & 0x0F
        let key = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        let xor = key.iter().fold(0u8, |acc, &b| acc ^ b);
        let expected = ((xor >> 4) ^ xor) & 0x0F;
        assert_eq!(bucket_hash(&key, 0), expected);
    }

    #[test]
    fn test_bucket_hash_range() {
        // All results must be in [0, 15]
        for i in 0..=255u8 {
            let key = [i; 9];
            for seed in 0..=1u8 {
                let result = bucket_hash(&key, seed);
                assert!(
                    result < 16,
                    "bucket_hash returned {result} for key [{i}; 9], seed {seed}"
                );
            }
        }
    }

    #[test]
    fn test_generate_segment_key_targets_bucket() {
        let path_hash = [0xABu8; 16];

        for target_bucket in 0..16u8 {
            let key = generate_segment_key(&path_hash, 42, target_bucket);
            assert_eq!(
                bucket_hash(&key[..9], 1),
                target_bucket,
                "Generated key doesn't hash to target bucket {target_bucket}"
            );
        }
    }

    #[test]
    fn test_segment_header_round_trip() {
        let path_hash = [0x12u8; 16];
        let header = SegmentHeader::generate(5, &path_hash);

        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), SEGMENT_HEADER_SIZE);

        let parsed = SegmentHeader::from_bytes(&bytes).expect("parse should succeed");

        // Each bucket key should match
        for bucket in 0..16u8 {
            assert_eq!(
                header.bucket_key(bucket),
                parsed.bucket_key(bucket),
                "Bucket {bucket} key mismatch after round-trip"
            );
        }
    }

    #[test]
    fn test_segment_header_keys_target_correct_buckets() {
        let path_hash = [0xDEu8; 16];
        let header = SegmentHeader::generate(100, &path_hash);

        for bucket in 0..16u8 {
            let key = header.bucket_key(bucket);
            assert_eq!(
                bucket_hash(&key[..9], 1),
                bucket,
                "Header key for bucket {bucket} doesn't hash correctly"
            );
        }
    }

    #[test]
    fn test_parse_data_filename() {
        assert_eq!(parse_data_filename("data.000"), Some(0));
        assert_eq!(parse_data_filename("data.001"), Some(1));
        assert_eq!(parse_data_filename("data.999"), Some(999));
        assert_eq!(parse_data_filename("data.1022"), Some(1022));
        assert_eq!(parse_data_filename("data.1023"), None); // >= MAX_SEGMENTS
        assert_eq!(parse_data_filename("data.1024"), None);
        assert_eq!(parse_data_filename("data.abc"), None);
        assert_eq!(parse_data_filename("index.000"), None);
        assert_eq!(parse_data_filename("data.00"), None); // Too short
        assert_eq!(parse_data_filename("data.00000"), None); // Too long
    }

    #[test]
    fn test_segment_data_path() {
        let base = std::path::Path::new("/tmp/data");
        assert_eq!(
            segment_data_path(base, 0),
            std::path::PathBuf::from("/tmp/data/data.000")
        );
        assert_eq!(
            segment_data_path(base, 42),
            std::path::PathBuf::from("/tmp/data/data.042")
        );
        assert_eq!(
            segment_data_path(base, 999),
            std::path::PathBuf::from("/tmp/data/data.999")
        );
    }

    #[test]
    fn test_segment_info_space_check() {
        let header = SegmentHeader::zeroed();
        let mut info = SegmentInfo::new(0, header);

        // New segment should have space (write position starts after header)
        assert!(info.has_space_for(1024));
        assert!(info.has_space_for(SEGMENT_SIZE - SEGMENT_HEADER_SIZE as u64));
        assert!(!info.has_space_for(SEGMENT_SIZE)); // Header takes space

        // Frozen segment should reject writes
        info.state = SegmentState::Frozen;
        assert!(!info.has_space_for(1));
    }

    #[test]
    fn test_too_short_data_rejected() {
        let short = [0u8; 100];
        assert!(SegmentHeader::from_bytes(&short).is_none());
    }

    #[test]
    fn test_segment_allocator_allocate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0xAB; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);

        // First allocation creates a new segment
        let a1 = alloc.allocate(1024).expect("alloc1");
        assert_eq!(a1.segment_index, 0);
        assert_eq!(a1.file_offset, SEGMENT_HEADER_SIZE as u32);

        // Second allocation in the same segment
        let a2 = alloc.allocate(2048).expect("alloc2");
        assert_eq!(a2.segment_index, 0);
        assert_eq!(a2.file_offset, SEGMENT_HEADER_SIZE as u32 + 1024);

        // Data file should exist
        assert!(segment_data_path(dir.path(), 0).exists());
    }

    #[test]
    fn test_segment_allocator_creates_new_segment_when_full() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0xCD; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);

        // Fill first segment almost completely
        let remaining = SEGMENT_SIZE - SEGMENT_HEADER_SIZE as u64;
        let a1 = alloc.allocate(remaining).expect("alloc big");
        assert_eq!(a1.segment_index, 0);

        // Next allocation must create a new segment
        let a2 = alloc.allocate(1024).expect("alloc overflow");
        assert_eq!(a2.segment_index, 1);
        assert_eq!(a2.file_offset, SEGMENT_HEADER_SIZE as u32);

        assert_eq!(alloc.segment_count(), 2);
    }

    #[test]
    fn test_segment_allocator_max_segments_enforced() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0xEF; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 2);

        // Fill two segments
        let remaining = SEGMENT_SIZE - SEGMENT_HEADER_SIZE as u64;
        alloc.allocate(remaining).expect("seg0");
        alloc.allocate(remaining).expect("seg1");

        // Third segment should fail
        let result = alloc.allocate(1024);
        assert!(result.is_err());
    }

    #[test]
    fn test_segment_allocator_freeze_thaw() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0x12; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);

        alloc.allocate(1024).expect("alloc");

        // Segment 0 starts thawed
        assert_eq!(
            alloc.segment(0).expect("segment 0 exists").state,
            SegmentState::Thawed
        );

        // Freeze
        assert!(alloc.freeze(0));
        assert_eq!(
            alloc.segment(0).expect("segment 0 exists").state,
            SegmentState::Frozen
        );

        // Can't allocate in frozen segment
        // But there's space, so allocator creates segment 1
        let a = alloc.allocate(512).expect("alloc after freeze");
        assert_eq!(a.segment_index, 1);

        // Thaw segment 0
        assert!(alloc.thaw(0));
        assert_eq!(
            alloc.segment(0).expect("segment 0 exists").state,
            SegmentState::Thawed
        );
    }

    #[test]
    fn test_segment_allocator_load_existing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0x34; 16];

        // Create some segment files
        {
            let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);
            alloc.allocate(1024).expect("alloc0");
            alloc
                .allocate(SEGMENT_SIZE - SEGMENT_HEADER_SIZE as u64)
                .expect("fill0");
            alloc.allocate(2048).expect("alloc1");
        }

        // Reload
        let mut alloc2 = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);
        alloc2.load_existing().expect("load");

        // Should find the segment files (loaded as frozen)
        assert!(alloc2.segment_count() >= 1);
        // All loaded segments are frozen
        for seg in alloc2.segments() {
            assert_eq!(seg.state, SegmentState::Frozen);
        }
    }

    #[test]
    fn test_storage_offset_encoding_round_trip() {
        let (seg, off) = encode_storage_offset(42, 0x1234);
        let (seg2, off2) = decode_storage_offset(seg, off);
        assert_eq!(seg2, 42);
        assert_eq!(off2, 0x1234);
    }
}
