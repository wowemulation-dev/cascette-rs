//! Root file records and lookup structures

use crate::root::flags::{ContentFlags, LocaleFlags};
use cascette_crypto::jenkins::Jenkins96;
use cascette_crypto::md5::{ContentKey, FileDataId};
use std::collections::HashMap;

/// Individual root record mapping FileDataID/name to content key
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootRecord {
    /// File data identifier
    pub file_data_id: FileDataId,
    /// Content key (MD5 hash)
    pub content_key: ContentKey,
    /// Name hash (optional, based on flags and version)
    pub name_hash: Option<u64>,
}

impl RootRecord {
    /// Create new record
    pub fn new(file_data_id: FileDataId, content_key: ContentKey, name_hash: Option<u64>) -> Self {
        Self {
            file_data_id,
            content_key,
            name_hash,
        }
    }

    /// Create record with name hash calculated from path
    pub fn with_path(
        file_data_id: FileDataId,
        content_key: ContentKey,
        path: Option<&str>,
    ) -> Self {
        let name_hash = path.map(calculate_name_hash);
        Self::new(file_data_id, content_key, name_hash)
    }

    /// Check if record has name hash
    pub const fn has_name_hash(&self) -> bool {
        self.name_hash.is_some()
    }
}

/// Lookup entry for efficient content resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootEntry {
    /// Index of containing block
    pub block_index: usize,
    /// Content key for this entry
    pub content_key: ContentKey,
    /// Locale flags from containing block
    pub locale_flags: LocaleFlags,
    /// Content flags from containing block
    pub content_flags: ContentFlags,
}

impl RootEntry {
    /// Create new lookup entry
    pub const fn new(
        block_index: usize,
        content_key: ContentKey,
        locale_flags: LocaleFlags,
        content_flags: ContentFlags,
    ) -> Self {
        Self {
            block_index,
            content_key,
            locale_flags,
            content_flags,
        }
    }

    /// Check if entry matches locale requirements
    pub const fn matches_locale(&self, required_locale: LocaleFlags) -> bool {
        self.locale_flags.matches(required_locale)
    }

    /// Check if entry matches content requirements
    pub const fn matches_content(&self, required_content: ContentFlags) -> bool {
        (self.content_flags.value & required_content.value) == required_content.value
    }

    /// Check if entry matches both locale and content requirements
    pub const fn matches(&self, locale: LocaleFlags, content: ContentFlags) -> bool {
        self.matches_locale(locale) && self.matches_content(content)
    }
}

/// Lookup tables for efficient file resolution
#[derive(Debug, Default)]
pub struct RootLookupTables {
    /// `FileDataID` to entries mapping
    pub fdid_map: HashMap<FileDataId, Vec<RootEntry>>,
    /// Name hash to entries mapping
    pub name_map: HashMap<u64, Vec<RootEntry>>,
}

impl RootLookupTables {
    /// Create new empty lookup tables
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with capacity hints
    pub fn with_capacity(fdid_capacity: usize, name_capacity: usize) -> Self {
        Self {
            fdid_map: HashMap::with_capacity(fdid_capacity),
            name_map: HashMap::with_capacity(name_capacity),
        }
    }

    /// Add entry to lookup tables
    pub fn add_entry(&mut self, fdid: FileDataId, name_hash: Option<u64>, entry: RootEntry) {
        // Add to FileDataID lookup
        self.fdid_map.entry(fdid).or_default().push(entry.clone());

        // Add to name hash lookup if present
        if let Some(hash) = name_hash {
            self.name_map.entry(hash).or_default().push(entry);
        }
    }

    /// Clear all lookup tables
    pub fn clear(&mut self) {
        self.fdid_map.clear();
        self.name_map.clear();
    }

    /// Get total number of `FileDataID` entries
    pub fn fdid_count(&self) -> usize {
        self.fdid_map.len()
    }

    /// Get total number of name hash entries
    pub fn name_count(&self) -> usize {
        self.name_map.len()
    }

    /// Resolve file by `FileDataID` with locale and content filtering
    pub fn resolve_by_id(
        &self,
        fdid: FileDataId,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> Option<ContentKey> {
        self.fdid_map
            .get(&fdid)?
            .iter()
            .find(|entry| entry.matches(locale, content))
            .map(|entry| entry.content_key)
    }

    /// Resolve file by name hash with locale and content filtering
    pub fn resolve_by_hash(
        &self,
        name_hash: u64,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> Option<ContentKey> {
        self.name_map
            .get(&name_hash)?
            .iter()
            .find(|entry| entry.matches(locale, content))
            .map(|entry| entry.content_key)
    }

    /// Resolve file by path with locale and content filtering
    pub fn resolve_by_path(
        &self,
        path: &str,
        locale: LocaleFlags,
        content: ContentFlags,
    ) -> Option<ContentKey> {
        let name_hash = calculate_name_hash(path);
        self.resolve_by_hash(name_hash, locale, content)
    }

    /// Get all entries for a `FileDataID`
    pub fn get_entries_by_id(&self, fdid: FileDataId) -> Option<&Vec<RootEntry>> {
        self.fdid_map.get(&fdid)
    }

    /// Get all entries for a name hash
    pub fn get_entries_by_hash(&self, name_hash: u64) -> Option<&Vec<RootEntry>> {
        self.name_map.get(&name_hash)
    }

    /// Get all entries for a path
    pub fn get_entries_by_path(&self, path: &str) -> Option<&Vec<RootEntry>> {
        let name_hash = calculate_name_hash(path);
        self.get_entries_by_hash(name_hash)
    }
}

/// Calculate name hash for file path using Jenkins96 with `WoW`-specific transform
pub fn calculate_name_hash(path: &str) -> u64 {
    // Normalize path: uppercase and forward slashes
    let normalized = path.to_uppercase().replace('\\', "/");

    // Calculate Jenkins96 hash
    let hash = Jenkins96::hash(normalized.as_bytes());

    // WoW swaps the high and low 32-bit parts of the hash
    let high = (hash.hash64 >> 32) as u32;
    let low = (hash.hash64 & 0xFFFF_FFFF) as u32;

    (u64::from(low) << 32) | u64::from(high)
}

/// Decode `FileDataID` delta sequence
pub fn decode_file_data_ids(deltas: &[i32]) -> Vec<FileDataId> {
    let mut ids = Vec::with_capacity(deltas.len());
    let mut current_id: i32 = -1;

    for &delta in deltas {
        // Use checked arithmetic to handle potential overflow gracefully
        current_id = match current_id.checked_add(delta).and_then(|x| x.checked_add(1)) {
            Some(new_id) => new_id,
            None => {
                // On overflow, skip this entry and continue processing
                // This allows parsing to continue even with malformed data
                continue;
            }
        };
        // FileDataID encoding uses specific i32->u32 conversion as part of CASC format
        #[allow(clippy::cast_sign_loss)]
        {
            ids.push(FileDataId::new(current_id as u32));
        }
    }

    ids
}

/// Encode `FileDataIDs` as delta sequence
pub fn encode_file_data_ids(ids: &[FileDataId]) -> Vec<i32> {
    let mut deltas = Vec::with_capacity(ids.len());
    let mut prev_id: i32 = -1;

    for &id in ids {
        // FileDataID encoding uses specific u32->i32 conversion as part of CASC format
        #[allow(clippy::cast_possible_wrap)]
        let current_id = id.get() as i32;
        deltas.push(current_id - prev_id - 1);
        prev_id = current_id;
    }

    deltas
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::md5::ContentKey;

    #[test]
    fn test_root_record_creation() {
        let fdid = FileDataId::new(12_345);
        let ckey = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let path = "Interface\\Icons\\INV_Misc_QuestionMark.blp";

        let record = RootRecord::with_path(fdid, ckey, Some(path));

        assert_eq!(record.file_data_id, fdid);
        assert_eq!(record.content_key, ckey);
        assert!(record.has_name_hash());
        assert!(record.name_hash.is_some());
    }

    #[test]
    fn test_root_entry_matching() {
        let entry = RootEntry::new(
            0,
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            LocaleFlags::new(LocaleFlags::ENUS | LocaleFlags::DEDE),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        // Should match ENUS locale
        assert!(entry.matches_locale(LocaleFlags::new(LocaleFlags::ENUS)));

        // Should not match FRFR locale
        assert!(!entry.matches_locale(LocaleFlags::new(LocaleFlags::FRFR)));

        // Should match INSTALL content
        assert!(entry.matches_content(ContentFlags::new(ContentFlags::INSTALL)));

        // Should not match BUNDLE content
        assert!(!entry.matches_content(ContentFlags::new(ContentFlags::BUNDLE)));

        // Combined matching
        assert!(entry.matches(
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL)
        ));
    }

    #[test]
    fn test_name_hash_calculation() {
        let path1 = "Interface\\Icons\\INV_Misc_QuestionMark.blp";
        let path2 = "interface/icons/inv_misc_questionmark.blp"; // Different case/slashes

        let hash1 = calculate_name_hash(path1);
        let hash2 = calculate_name_hash(path2);

        // Should produce same hash after normalization
        assert_eq!(hash1, hash2);

        // Should not be zero
        assert_ne!(hash1, 0);
    }

    #[test]
    fn test_file_data_id_delta_encoding() {
        let original_ids = vec![
            FileDataId::new(100),
            FileDataId::new(101),
            FileDataId::new(105),
            FileDataId::new(110),
        ];

        let deltas = encode_file_data_ids(&original_ids);
        let decoded_ids = decode_file_data_ids(&deltas);

        assert_eq!(original_ids, decoded_ids);

        // Check expected delta values
        assert_eq!(deltas, vec![100, 0, 3, 4]);
    }

    #[test]
    fn test_lookup_tables() {
        let mut tables = RootLookupTables::new();

        let fdid = FileDataId::new(12_345);
        let ckey = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let name_hash = calculate_name_hash("test/file.txt");

        let entry = RootEntry::new(
            0,
            ckey,
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        tables.add_entry(fdid, Some(name_hash), entry);

        // Test FileDataID resolution
        let resolved = tables.resolve_by_id(
            fdid,
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert_eq!(resolved, Some(ckey));

        // Test name hash resolution
        let resolved = tables.resolve_by_hash(
            name_hash,
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert_eq!(resolved, Some(ckey));

        // Test path resolution
        let resolved = tables.resolve_by_path(
            "test/file.txt",
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert_eq!(resolved, Some(ckey));

        // Test failed resolution (wrong locale)
        let resolved = tables.resolve_by_id(
            fdid,
            LocaleFlags::new(LocaleFlags::FRFR),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert_eq!(resolved, None);
    }

    #[test]
    fn test_delta_encoding_edge_cases() {
        // Test empty sequence
        let empty_ids: Vec<FileDataId> = vec![];
        let deltas = encode_file_data_ids(&empty_ids);
        let decoded = decode_file_data_ids(&deltas);
        assert_eq!(decoded, empty_ids);

        // Test single element
        let single_id = vec![FileDataId::new(42)];
        let deltas = encode_file_data_ids(&single_id);
        let decoded = decode_file_data_ids(&deltas);
        assert_eq!(decoded, single_id);
        assert_eq!(deltas, vec![42]); // First ID encodes as its value

        // Test consecutive IDs
        let consecutive = vec![
            FileDataId::new(10),
            FileDataId::new(11),
            FileDataId::new(12),
        ];
        let deltas = encode_file_data_ids(&consecutive);
        assert_eq!(deltas, vec![10, 0, 0]); // 10, then +1-1=0, then +1-1=0
    }
}
