//! Bidirectional FileDataID resolution service.
//!
//! Wraps the id-to-path mapping from `cascette-import` and adds a reverse
//! (path-to-id) map with case-insensitive lookup.

use std::collections::HashMap;

use crate::content::ContentInfo;
use crate::error::{MetadataError, MetadataResult};

/// Bidirectional FileDataID to path resolution.
///
/// Maintains two hash maps:
/// - `id_to_path`: canonical-case paths keyed by FileDataID
/// - `path_to_id`: lowercase paths keyed to FileDataID for case-insensitive lookup
pub struct FileDataIdService {
    id_to_path: HashMap<u32, String>,
    path_to_id: HashMap<String, u32>,
}

impl FileDataIdService {
    /// Build from an existing id-to-path mapping.
    pub fn from_map(mappings: HashMap<u32, String>) -> Self {
        let path_to_id = mappings
            .iter()
            .map(|(&id, path)| (path.to_ascii_lowercase(), id))
            .collect();

        Self {
            id_to_path: mappings,
            path_to_id,
        }
    }

    /// Build from a `ListfileProvider` by cloning its in-memory mappings.
    #[cfg(feature = "import")]
    pub fn from_listfile_provider(provider: &cascette_import::ListfileProvider) -> Self {
        Self::from_map(provider.file_mappings().clone())
    }

    /// Resolve a FileDataID to its path.
    pub fn resolve_id(&self, id: u32) -> MetadataResult<&str> {
        self.id_to_path
            .get(&id)
            .map(String::as_str)
            .ok_or(MetadataError::FileDataIdNotFound(id))
    }

    /// Resolve a file path to its FileDataID (case-insensitive).
    pub fn resolve_path(&self, path: &str) -> MetadataResult<u32> {
        self.path_to_id
            .get(&path.to_ascii_lowercase())
            .copied()
            .ok_or_else(|| MetadataError::PathNotFound(path.to_string()))
    }

    /// Check whether a FileDataID is known.
    pub fn contains_id(&self, id: u32) -> bool {
        self.id_to_path.contains_key(&id)
    }

    /// Check whether a file path is known (case-insensitive).
    pub fn contains_path(&self, path: &str) -> bool {
        self.path_to_id.contains_key(&path.to_ascii_lowercase())
    }

    /// Get content metadata for a FileDataID.
    pub fn content_info(&self, id: u32) -> MetadataResult<ContentInfo> {
        let path = self.resolve_id(id)?;
        Ok(ContentInfo::from_path(path))
    }

    /// Number of file mappings.
    pub fn len(&self) -> usize {
        self.id_to_path.len()
    }

    /// Whether the service has no mappings.
    pub fn is_empty(&self) -> bool {
        self.id_to_path.is_empty()
    }

    /// Iterate over all `(id, path)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> {
        self.id_to_path
            .iter()
            .map(|(&id, path)| (id, path.as_str()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::content::ContentCategory;

    fn sample_mappings() -> HashMap<u32, String> {
        let mut m = HashMap::new();
        m.insert(100, "world/maps/azeroth/azeroth.wmo".to_string());
        m.insert(200, "sound/music/zone.mp3".to_string());
        m.insert(300, "interface/icons/spell_holy_heal.blp".to_string());
        m
    }

    #[test]
    fn test_resolve_id() {
        let svc = FileDataIdService::from_map(sample_mappings());
        assert_eq!(
            svc.resolve_id(100).unwrap(),
            "world/maps/azeroth/azeroth.wmo"
        );
        assert!(svc.resolve_id(999).is_err());
    }

    #[test]
    fn test_resolve_path_case_insensitive() {
        let svc = FileDataIdService::from_map(sample_mappings());
        assert_eq!(svc.resolve_path("sound/music/zone.mp3").unwrap(), 200);
        assert_eq!(svc.resolve_path("SOUND/MUSIC/ZONE.MP3").unwrap(), 200);
        assert_eq!(svc.resolve_path("Sound/Music/Zone.Mp3").unwrap(), 200);
        assert!(svc.resolve_path("nonexistent/file.txt").is_err());
    }

    #[test]
    fn test_contains() {
        let svc = FileDataIdService::from_map(sample_mappings());
        assert!(svc.contains_id(100));
        assert!(!svc.contains_id(999));
        assert!(svc.contains_path("sound/music/zone.mp3"));
        assert!(svc.contains_path("SOUND/MUSIC/ZONE.MP3"));
        assert!(!svc.contains_path("missing.txt"));
    }

    #[test]
    fn test_content_info() {
        let svc = FileDataIdService::from_map(sample_mappings());
        let info = svc.content_info(300).unwrap();
        assert_eq!(info.category, ContentCategory::Interface);
        assert_eq!(info.extension, "blp");
    }

    #[test]
    fn test_len_and_iter() {
        let svc = FileDataIdService::from_map(sample_mappings());
        assert_eq!(svc.len(), 3);
        assert!(!svc.is_empty());

        assert_eq!(svc.iter().count(), 3);
    }

    #[test]
    fn test_empty_service() {
        let svc = FileDataIdService::from_map(HashMap::new());
        assert!(svc.is_empty());
        assert_eq!(svc.len(), 0);
    }
}
