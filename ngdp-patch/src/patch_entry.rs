//! Patch entry structures for patch manifests

use crate::error::Result;
use crate::{ContentKey, EncodingKey};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Read, Write};
use tracing::trace;

/// A single patch entry mapping old content to new content via a patch
#[derive(Debug, Clone)]
pub struct PatchEntry {
    /// Content key of the old file
    pub old_ckey: ContentKey,
    /// Content key of the new file
    pub new_ckey: ContentKey,
    /// Encoding key of the patch file
    pub patch_ekey: EncodingKey,
    /// Size of the old file
    pub old_size: u64,
    /// Size of the new file
    pub new_size: u64,
    /// Patch records for incremental patching
    pub records: Vec<PatchRecord>,
}

impl PatchEntry {
    /// Create a new patch entry
    pub fn new(
        old_ckey: ContentKey,
        new_ckey: ContentKey,
        patch_ekey: EncodingKey,
        old_size: u64,
        new_size: u64,
    ) -> Self {
        Self {
            old_ckey,
            new_ckey,
            patch_ekey,
            old_size,
            new_size,
            records: Vec::new(),
        }
    }

    /// Read a patch entry from a stream
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut old_ckey = [0u8; 16];
        let mut new_ckey = [0u8; 16];
        let mut patch_ekey = [0u8; 16];

        reader.read_exact(&mut old_ckey)?;
        reader.read_exact(&mut new_ckey)?;
        reader.read_exact(&mut patch_ekey)?;

        let old_size = reader.read_u64::<BigEndian>()?;
        let new_size = reader.read_u64::<BigEndian>()?;

        // Read record count
        let record_count = reader.read_u32::<LittleEndian>()? as usize;

        let mut records = Vec::with_capacity(record_count);
        for _ in 0..record_count {
            records.push(PatchRecord::read(reader)?);
        }

        Ok(Self {
            old_ckey,
            new_ckey,
            patch_ekey,
            old_size,
            new_size,
            records,
        })
    }

    /// Write a patch entry to a stream
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.old_ckey)?;
        writer.write_all(&self.new_ckey)?;
        writer.write_all(&self.patch_ekey)?;

        writer.write_u64::<BigEndian>(self.old_size)?;
        writer.write_u64::<BigEndian>(self.new_size)?;

        writer.write_u32::<LittleEndian>(self.records.len() as u32)?;

        for record in &self.records {
            record.write(writer)?;
        }

        Ok(())
    }

    /// Check if this entry can patch from the given content key
    pub fn can_patch_from(&self, ckey: &ContentKey) -> bool {
        self.old_ckey == *ckey
    }

    /// Check if this entry produces the given content key
    pub fn patches_to(&self, ckey: &ContentKey) -> bool {
        self.new_ckey == *ckey
    }

    /// Add a patch record for incremental patching
    pub fn add_record(&mut self, record: PatchRecord) {
        self.records.push(record);
        // Keep records sorted by ordinal
        self.records.sort_by_key(|r| r.ordinal);
    }
}

/// A single patch record within a patch entry
/// Used for incremental patching where multiple patches are applied in sequence
#[derive(Debug, Clone)]
pub struct PatchRecord {
    /// Encoding key of this specific patch
    pub patch_ekey: EncodingKey,
    /// Size of the patch file
    pub patch_size: u32,
    /// Order in which to apply this patch (0-based)
    pub ordinal: u8,
    /// MD5 hash of the resulting file after this patch
    pub result_ckey: ContentKey,
}

impl PatchRecord {
    /// Create a new patch record
    pub fn new(
        patch_ekey: EncodingKey,
        patch_size: u32,
        ordinal: u8,
        result_ckey: ContentKey,
    ) -> Self {
        Self {
            patch_ekey,
            patch_size,
            ordinal,
            result_ckey,
        }
    }

    /// Read a patch record from a stream
    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let mut patch_ekey = [0u8; 16];
        reader.read_exact(&mut patch_ekey)?;

        let patch_size = reader.read_u32::<LittleEndian>()?;
        let ordinal = reader.read_u8()?;

        let mut result_ckey = [0u8; 16];
        reader.read_exact(&mut result_ckey)?;

        Ok(Self {
            patch_ekey,
            patch_size,
            ordinal,
            result_ckey,
        })
    }

    /// Write a patch record to a stream
    pub fn write<W: Write>(&self, writer: &mut W) -> Result<()> {
        writer.write_all(&self.patch_ekey)?;
        writer.write_u32::<LittleEndian>(self.patch_size)?;
        writer.write_u8(self.ordinal)?;
        writer.write_all(&self.result_ckey)?;
        Ok(())
    }
}

/// Collection of patch entries indexed by content key
#[derive(Debug, Clone, Default)]
pub struct PatchIndex {
    /// Map from old content key to patch entries
    entries: Vec<PatchEntry>,
}

impl PatchIndex {
    /// Create a new empty patch index
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a patch entry to the index
    pub fn add_entry(&mut self, entry: PatchEntry) {
        trace!(
            "Adding patch entry: {:02x?} -> {:02x?}",
            &entry.old_ckey[..4],
            &entry.new_ckey[..4]
        );
        self.entries.push(entry);
    }

    /// Find a patch entry that can update from the given content key
    pub fn find_patch_from(&self, old_ckey: &ContentKey) -> Option<&PatchEntry> {
        self.entries.iter().find(|e| e.can_patch_from(old_ckey))
    }

    /// Find all patch entries that produce the given content key
    pub fn find_patches_to(&self, new_ckey: &ContentKey) -> Vec<&PatchEntry> {
        self.entries
            .iter()
            .filter(|e| e.patches_to(new_ckey))
            .collect()
    }

    /// Find a patch chain from old to new content key
    /// Returns None if no path exists
    pub fn find_patch_chain(
        &self,
        from_ckey: &ContentKey,
        to_ckey: &ContentKey,
    ) -> Option<Vec<&PatchEntry>> {
        // Simple BFS to find shortest patch chain
        use std::collections::{HashSet, VecDeque};

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with direct patches from source
        if let Some(entry) = self.find_patch_from(from_ckey) {
            if entry.patches_to(to_ckey) {
                // Direct patch found
                return Some(vec![entry]);
            }
            queue.push_back((entry, vec![entry]));
            visited.insert(entry.new_ckey);
        }

        // BFS for patch chain
        while let Some((current, path)) = queue.pop_front() {
            // Look for patches from current result
            if let Some(next) = self.find_patch_from(&current.new_ckey) {
                if next.patches_to(to_ckey) {
                    // Found complete chain
                    let mut result = path;
                    result.push(next);
                    return Some(result);
                }

                if !visited.contains(&next.new_ckey) && path.len() < 10 {
                    // Limit chain length to prevent infinite loops
                    visited.insert(next.new_ckey);
                    let mut new_path = path.clone();
                    new_path.push(next);
                    queue.push_back((next, new_path));
                }
            }
        }

        None
    }

    /// Get total number of patch entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over all entries
    pub fn iter(&self) -> impl Iterator<Item = &PatchEntry> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_entry_roundtrip() {
        let entry = PatchEntry::new([1u8; 16], [2u8; 16], [3u8; 16], 1000, 1500);

        let mut buffer = Vec::new();
        entry.write(&mut buffer).unwrap();

        let mut cursor = std::io::Cursor::new(buffer);
        let read_entry = PatchEntry::read(&mut cursor).unwrap();

        assert_eq!(entry.old_ckey, read_entry.old_ckey);
        assert_eq!(entry.new_ckey, read_entry.new_ckey);
        assert_eq!(entry.patch_ekey, read_entry.patch_ekey);
        assert_eq!(entry.old_size, read_entry.old_size);
        assert_eq!(entry.new_size, read_entry.new_size);
    }

    #[test]
    fn test_patch_index() {
        let mut index = PatchIndex::new();

        let entry1 = PatchEntry::new([1u8; 16], [2u8; 16], [10u8; 16], 100, 150);
        let entry2 = PatchEntry::new([2u8; 16], [3u8; 16], [11u8; 16], 150, 200);

        index.add_entry(entry1);
        index.add_entry(entry2);

        // Find direct patch
        assert!(index.find_patch_from(&[1u8; 16]).is_some());
        assert!(index.find_patch_from(&[99u8; 16]).is_none());

        // Find patch chain
        let chain = index.find_patch_chain(&[1u8; 16], &[3u8; 16]);
        assert!(chain.is_some());
        assert_eq!(chain.unwrap().len(), 2);
    }
}
