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
        let mut headers =
            std::array::from_fn(|i| LocalHeader::new([0u8; 16], 0, i * LOCAL_HEADER_SIZE));

        for (bucket, header) in headers.iter_mut().enumerate() {
            let key = generate_segment_key(path_hash, segment_index, bucket as u8);
            *header = LocalHeader::new(key, 0, bucket * LOCAL_HEADER_SIZE);
        }

        Self { headers }
    }

    /// Create a zeroed segment header (for new/empty segments).
    pub fn zeroed() -> Self {
        Self {
            headers: std::array::from_fn(|i| LocalHeader::new([0u8; 16], 0, i * LOCAL_HEADER_SIZE)),
        }
    }

    /// Parse a segment header from 480 bytes.
    ///
    /// Returns `None` if the data is too short.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < SEGMENT_HEADER_SIZE {
            return None;
        }

        let mut headers =
            std::array::from_fn(|i| LocalHeader::new([0u8; 16], 0, i * LOCAL_HEADER_SIZE));

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

    /// Set the reconstruction header for a specific bucket.
    pub fn set_bucket_header(&mut self, bucket: u8, header: LocalHeader) {
        self.headers[(bucket as usize) & 0x0F] = header;
    }

    /// Get a mutable reference to the reconstruction header for a specific bucket.
    pub fn get_bucket_header(&self, bucket: u8) -> &LocalHeader {
        &self.headers[(bucket as usize) & 0x0F]
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

// ---------------------------------------------------------------------------
// Free space tracking
// ---------------------------------------------------------------------------

/// A contiguous free byte range within a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeSpan {
    /// Byte offset within the segment's data file.
    pub offset: u32,
    /// Length of the free range in bytes.
    pub length: u32,
}

impl FreeSpan {
    /// End offset (exclusive).
    pub fn end(&self) -> u32 {
        self.offset.saturating_add(self.length)
    }
}

/// Per-segment free list with coalescing insert and first-fit allocation.
///
/// Spans are kept sorted by offset. Adjacent or overlapping spans are
/// merged on insert to prevent fragmentation.
#[derive(Debug, Clone, Default)]
pub struct FreeList {
    spans: Vec<FreeSpan>,
}

impl FreeList {
    /// Create an empty free list.
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    /// Number of tracked free spans.
    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    /// Total free bytes across all spans.
    pub fn total_free(&self) -> u64 {
        self.spans.iter().map(|s| u64::from(s.length)).sum()
    }

    /// Insert a free span, coalescing with adjacent or overlapping neighbors.
    pub fn insert(&mut self, offset: u32, length: u32) {
        if length == 0 {
            return;
        }

        let new_end = offset.saturating_add(length);

        // Find insertion point (sorted by offset).
        let pos = self.spans.partition_point(|s| s.offset < offset);

        // Determine merge range: which existing spans overlap or touch the new one.
        let mut merge_start = pos;
        let mut merge_end = pos;

        // Check left neighbor
        if merge_start > 0 && self.spans[merge_start - 1].end() >= offset {
            merge_start -= 1;
        }

        // Check right neighbors
        while merge_end < self.spans.len() && self.spans[merge_end].offset <= new_end {
            merge_end += 1;
        }

        if merge_start < merge_end {
            // Merge: compute the union of all overlapping/adjacent spans + new span.
            let combined_offset = self.spans[merge_start].offset.min(offset);
            let combined_end = self.spans[merge_end - 1].end().max(new_end);
            self.spans[merge_start] = FreeSpan {
                offset: combined_offset,
                length: combined_end - combined_offset,
            };
            // Remove the spans that were merged (all except merge_start).
            if merge_end - merge_start > 1 {
                self.spans.drain(merge_start + 1..merge_end);
            }
        } else {
            // No overlap: insert at position.
            self.spans.insert(pos, FreeSpan { offset, length });
        }
    }

    /// Try to allocate `size` bytes using first-fit.
    ///
    /// Returns the offset of the allocated block, or `None` if no span
    /// is large enough.
    pub fn allocate(&mut self, size: u32) -> Option<u32> {
        for i in 0..self.spans.len() {
            if self.spans[i].length >= size {
                let offset = self.spans[i].offset;
                if self.spans[i].length == size {
                    self.spans.remove(i);
                } else {
                    self.spans[i].offset += size;
                    self.spans[i].length -= size;
                }
                return Some(offset);
            }
        }
        None
    }

    /// Get the free spans (for inspection/testing).
    pub fn spans(&self) -> &[FreeSpan] {
        &self.spans
    }
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
    /// Per-segment free lists for space reclamation.
    free_lists: Vec<FreeList>,
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
            free_lists: Vec::new(),
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
    /// When `use_free_list` is true, free lists are checked first (first-fit
    /// across all segments) before falling back to bump allocation.
    ///
    /// Strategy:
    /// 1. If `use_free_list`, try free lists across all segments
    /// 2. Try thawed segments in order (bump allocation)
    /// 3. If none have space, create a new segment
    /// 4. Returns error if MAX_SEGMENTS reached
    pub fn allocate(&mut self, size: u64, use_free_list: bool) -> crate::Result<Allocation> {
        let size32 = u32::try_from(size).map_err(|_| {
            crate::StorageError::Archive("allocation size exceeds u32 range".into())
        })?;

        // Try free lists first
        if use_free_list {
            for (idx, free_list) in self.free_lists.iter_mut().enumerate() {
                if let Some(offset) = free_list.allocate(size32) {
                    return Ok(Allocation {
                        segment_index: u16::try_from(idx).map_err(|_| {
                            crate::StorageError::Archive(
                                "segment index exceeds u16 range".to_string(),
                            )
                        })?,
                        file_offset: offset,
                    });
                }
            }
        }

        // Try existing thawed segments (bump allocation)
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
        self.free_lists.push(FreeList::new());

        Ok(Allocation {
            segment_index: new_index,
            file_offset: u32::try_from(offset).map_err(|_| {
                crate::StorageError::Archive("segment offset exceeds u32 range".to_string())
            })?,
        })
    }

    /// Return a byte range to the free list for a given segment.
    ///
    /// The span will be coalesced with adjacent free spans automatically.
    pub fn free_span(&mut self, segment_index: u16, offset: u32, length: u32) {
        let idx = segment_index as usize;
        // Grow free_lists to cover this segment index.
        while self.free_lists.len() <= idx {
            self.free_lists.push(FreeList::new());
        }
        self.free_lists[idx].insert(offset, length);
        // TODO: sync free space table to shmem when shared memory is active
    }

    /// Get the free list for a segment (for inspection).
    pub fn free_list(&self, segment_index: u16) -> Option<&FreeList> {
        self.free_lists.get(segment_index as usize)
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
        let a1 = alloc.allocate(1024, false).expect("alloc1");
        assert_eq!(a1.segment_index, 0);
        assert_eq!(a1.file_offset, SEGMENT_HEADER_SIZE as u32);

        // Second allocation in the same segment
        let a2 = alloc.allocate(2048, false).expect("alloc2");
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
        let a1 = alloc.allocate(remaining, false).expect("alloc big");
        assert_eq!(a1.segment_index, 0);

        // Next allocation must create a new segment
        let a2 = alloc.allocate(1024, false).expect("alloc overflow");
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
        alloc.allocate(remaining, false).expect("seg0");
        alloc.allocate(remaining, false).expect("seg1");

        // Third segment should fail
        let result = alloc.allocate(1024, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_segment_allocator_freeze_thaw() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0x12; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);

        alloc.allocate(1024, false).expect("alloc");

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
        let a = alloc.allocate(512, false).expect("alloc after freeze");
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
            alloc.allocate(1024, false).expect("alloc0");
            alloc
                .allocate(SEGMENT_SIZE - SEGMENT_HEADER_SIZE as u64, false)
                .expect("fill0");
            alloc.allocate(2048, false).expect("alloc1");
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

    #[test]
    fn test_set_bucket_header() {
        let mut header = SegmentHeader::zeroed();
        let key = [0xAA; 16];
        let local = LocalHeader::new(key, 1000, 0);

        header.set_bucket_header(5, local);

        let stored = header.bucket_header(5);
        assert_eq!(stored.original_encoding_key(), key);
        assert_eq!(stored.size_with_header, 1000 + LOCAL_HEADER_SIZE as u32);
    }

    #[test]
    fn test_set_bucket_header_masks_to_4_bits() {
        let mut header = SegmentHeader::zeroed();
        let key = [0xBB; 16];
        let local = LocalHeader::new(key, 500, 0);

        // bucket 0x13 should be masked to 0x03
        header.set_bucket_header(0x13, local);

        let stored = header.bucket_header(3);
        assert_eq!(stored.original_encoding_key(), key);
    }

    #[test]
    fn test_segment_header_checksums_valid_after_set() {
        let mut header = SegmentHeader::zeroed();
        let key = [0xCC; 16];
        let base_offset = 0;
        let local = LocalHeader::new(key, 2000, base_offset);

        header.set_bucket_header(7, local);

        let stored = header.bucket_header(7);
        assert!(
            stored.validate_checksums(base_offset),
            "Jenkins and XOR checksums should be valid after set_bucket_header"
        );
    }

    #[test]
    fn test_multiple_set_bucket_headers_different_buckets() {
        let mut header = SegmentHeader::zeroed();

        let key_a = [0x11; 16];
        let key_b = [0x22; 16];

        header.set_bucket_header(2, LocalHeader::new(key_a, 100, 0));
        header.set_bucket_header(9, LocalHeader::new(key_b, 200, 0));

        // Both should be present
        assert_eq!(header.bucket_header(2).original_encoding_key(), key_a);
        assert_eq!(header.bucket_header(9).original_encoding_key(), key_b);

        // Round-trip through bytes should preserve both
        let bytes = header.to_bytes();
        let parsed = SegmentHeader::from_bytes(&bytes).expect("parse");

        assert_eq!(parsed.bucket_header(2).original_encoding_key(), key_a);
        assert_eq!(parsed.bucket_header(9).original_encoding_key(), key_b);
    }

    // --- Free list tests ---

    #[test]
    fn test_free_list_insert_and_allocate() {
        let mut fl = FreeList::new();
        fl.insert(100, 50);
        assert_eq!(fl.span_count(), 1);
        assert_eq!(fl.total_free(), 50);

        // First-fit allocation
        let offset = fl.allocate(30).expect("alloc 30");
        assert_eq!(offset, 100);
        assert_eq!(fl.total_free(), 20);

        // Remaining span starts at 130, length 20
        assert_eq!(fl.spans()[0].offset, 130);
        assert_eq!(fl.spans()[0].length, 20);
    }

    #[test]
    fn test_free_list_exact_fit() {
        let mut fl = FreeList::new();
        fl.insert(200, 100);
        let offset = fl.allocate(100).expect("exact fit");
        assert_eq!(offset, 200);
        assert_eq!(fl.span_count(), 0);
    }

    #[test]
    fn test_free_list_no_fit() {
        let mut fl = FreeList::new();
        fl.insert(0, 10);
        assert!(fl.allocate(20).is_none());
    }

    #[test]
    fn test_free_list_coalesce_adjacent() {
        let mut fl = FreeList::new();
        fl.insert(100, 50); // [100..150)
        fl.insert(150, 50); // [150..200) — adjacent to first

        // Should coalesce into one span [100..200)
        assert_eq!(fl.span_count(), 1);
        assert_eq!(fl.spans()[0].offset, 100);
        assert_eq!(fl.spans()[0].length, 100);
    }

    #[test]
    fn test_free_list_coalesce_overlapping() {
        let mut fl = FreeList::new();
        fl.insert(100, 60); // [100..160)
        fl.insert(140, 60); // [140..200) — overlaps

        assert_eq!(fl.span_count(), 1);
        assert_eq!(fl.spans()[0].offset, 100);
        assert_eq!(fl.spans()[0].length, 100);
    }

    #[test]
    fn test_free_list_coalesce_bridge() {
        let mut fl = FreeList::new();
        fl.insert(100, 20); // [100..120)
        fl.insert(200, 20); // [200..220)
        assert_eq!(fl.span_count(), 2);

        // Insert span that bridges both: [120..200)
        fl.insert(120, 80);
        assert_eq!(fl.span_count(), 1);
        assert_eq!(fl.spans()[0].offset, 100);
        assert_eq!(fl.spans()[0].length, 120);
    }

    #[test]
    fn test_free_list_sorted_order() {
        let mut fl = FreeList::new();
        fl.insert(300, 10);
        fl.insert(100, 10);
        fl.insert(200, 10);

        assert_eq!(fl.span_count(), 3);
        assert_eq!(fl.spans()[0].offset, 100);
        assert_eq!(fl.spans()[1].offset, 200);
        assert_eq!(fl.spans()[2].offset, 300);
    }

    #[test]
    fn test_free_list_zero_length_ignored() {
        let mut fl = FreeList::new();
        fl.insert(100, 0);
        assert_eq!(fl.span_count(), 0);
    }

    #[test]
    fn test_segment_allocator_free_span() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0xAB; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);

        // Allocate to create segment 0
        let a = alloc.allocate(1024, false).expect("alloc");
        assert_eq!(a.segment_index, 0);

        // Free the allocated span
        alloc.free_span(0, a.file_offset, 1024);

        let fl = alloc.free_list(0).expect("free list for seg 0");
        assert_eq!(fl.total_free(), 1024);
    }

    #[test]
    fn test_segment_allocator_allocate_from_free_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0xCD; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);

        // Allocate and free a block
        let a = alloc.allocate(2048, false).expect("alloc");
        alloc.free_span(a.segment_index, a.file_offset, 2048);

        // Allocate with free list enabled — should reuse the freed block
        let b = alloc.allocate(1024, true).expect("alloc from free list");
        assert_eq!(b.segment_index, a.segment_index);
        assert_eq!(b.file_offset, a.file_offset);

        // Free list should have remaining 1024 bytes
        let fl = alloc.free_list(0).expect("free list");
        assert_eq!(fl.total_free(), 1024);
    }

    #[test]
    fn test_segment_allocator_allocate_falls_through_when_free_list_too_small() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path_hash = [0xEF; 16];
        let mut alloc = SegmentAllocator::new(dir.path().to_path_buf(), path_hash, 10);

        // Allocate and free a small block
        let a = alloc.allocate(100, false).expect("alloc");
        alloc.free_span(a.segment_index, a.file_offset, 100);

        // Request more than the free span — should fall through to bump allocation
        let b = alloc.allocate(200, true).expect("bump alloc");
        // Should be placed after the first allocation's bump position
        assert!(b.file_offset > a.file_offset);
    }
}
