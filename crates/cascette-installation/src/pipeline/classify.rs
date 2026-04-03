//! Artifact classification for download planning.
//!
//! Classifies install manifest entries into required, partial (size mismatch),
//! and already-present sets. Sorts by download priority for sequential CDN access.

use std::collections::HashMap;
use std::hash::BuildHasher;

use cascette_crypto::{ContentKey, EncodingKey};
use cascette_formats::download::{DownloadManifest, PriorityCategory};
use cascette_formats::encoding::EncodingFile;
use cascette_formats::install::InstallManifest;

use rand::rng;
use rand::seq::SliceRandom;

use crate::error::InstallationResult;

/// A single artifact to download.
#[derive(Debug, Clone)]
pub struct ArtifactDescriptor {
    /// File path from the install manifest.
    pub path: String,

    /// Content key (MD5 of uncompressed data).
    pub content_key: ContentKey,

    /// Encoding key (MD5 of BLTE-encoded data).
    pub encoding_key: EncodingKey,

    /// File size from the install manifest.
    pub file_size: u32,

    /// Download priority category.
    pub priority: PriorityCategory,

    /// Priority value (lower = higher priority).
    pub priority_value: i8,
}

/// Classification result for the artifact set.
#[derive(Debug)]
pub struct ArtifactSet {
    /// Artifacts that need to be downloaded.
    pub required: Vec<ArtifactDescriptor>,

    /// Number of artifacts already present in local storage.
    pub already_present: usize,

    /// Number of artifacts that could not be resolved in the encoding file.
    pub unresolved: usize,
}

impl ArtifactSet {
    /// Total bytes to download.
    #[must_use]
    pub fn total_download_bytes(&self) -> u64 {
        self.required.iter().map(|a| u64::from(a.file_size)).sum()
    }
}

/// Classify install manifest entries into download sets.
///
/// For each entry in the install manifest matching the given tags:
/// 1. Look up the content key in the encoding file to get the encoding key
/// 2. Check if the encoding key already exists in local storage
/// 3. Assign priority from the download manifest if available
///
/// Returns artifacts sorted by priority (Critical first) then by encoding key
/// for sequential CDN access.
pub fn classify_artifacts<S: BuildHasher>(
    install: &InstallManifest,
    encoding: &EncodingFile,
    download: &DownloadManifest,
    tags: &[&str],
    local_keys: &std::collections::HashSet<String, S>,
) -> InstallationResult<ArtifactSet> {
    // Build a lookup from encoding key -> download entry for priority
    let mut download_priority: HashMap<[u8; 16], (PriorityCategory, i8)> = HashMap::new();
    for entry in &download.entries {
        let priority = entry.priority_category(&download.header);
        let priority_value = entry.effective_priority(&download.header);
        download_priority.insert(*entry.encoding_key.as_bytes(), (priority, priority_value));
    }

    let files = if tags.is_empty() {
        install.entries.iter().enumerate().collect::<Vec<_>>()
    } else {
        // Use tag query with OR-within-group, AND-between-groups logic
        // matching agent.exe's TagTable::ApplyTagQuery behavior.
        install.get_files_for_tag_query(tags)
    };

    let mut required = Vec::new();
    let mut already_present: usize = 0;
    let mut unresolved: usize = 0;

    for (_idx, entry) in &files {
        // Look up encoding key
        let Some(ekey) = encoding.find_encoding(&entry.content_key) else {
            unresolved += 1;
            continue;
        };

        let ekey_hex = hex::encode(ekey.as_bytes());

        // Check if already downloaded
        if local_keys.contains(&ekey_hex) {
            already_present += 1;
            continue;
        }

        // Get priority from download manifest, default to Normal
        let (priority, priority_value) = download_priority
            .get(ekey.as_bytes())
            .copied()
            .unwrap_or((PriorityCategory::Normal, 3));

        required.push(ArtifactDescriptor {
            path: entry.path.clone(),
            content_key: entry.content_key,
            encoding_key: ekey,
            file_size: entry.file_size,
            priority,
            priority_value,
        });
    }

    // Sort by priority (ascending = higher priority first), then by encoding key
    required.sort_by(|a, b| {
        a.priority_value
            .cmp(&b.priority_value)
            .then_with(|| a.encoding_key.as_bytes().cmp(b.encoding_key.as_bytes()))
    });

    // Shuffle within each priority bucket to spread CDN load across clients
    shuffle_within_priority_groups(&mut required);

    Ok(ArtifactSet {
        required,
        already_present,
        unresolved,
    })
}

/// Classify download manifest entries filtered by platform tags.
///
/// The download manifest contains the bulk of game data (CASC archive blobs)
/// that the install manifest does not cover. Tags filter entries to the
/// target platform/architecture/locale using bitmask intersection (AND logic).
///
/// Returns artifacts sorted by priority then encoding key for sequential CDN access.
pub fn classify_download_artifacts<S: BuildHasher>(
    download: &DownloadManifest,
    tags: &[&str],
    local_keys: &std::collections::HashSet<String, S>,
) -> InstallationResult<ArtifactSet> {
    let entries = if tags.is_empty() {
        download.entries.iter().enumerate().collect::<Vec<_>>()
    } else {
        // Use tag query with OR-within-group, AND-between-groups logic
        // matching agent.exe's TagTable::ApplyTagQuery behavior.
        download.entries_by_tag_query(tags)
    };

    let mut required = Vec::new();
    let mut already_present: usize = 0;

    for (_idx, entry) in &entries {
        let ekey_hex = hex::encode(entry.encoding_key.as_bytes());

        if local_keys.contains(&ekey_hex) {
            already_present += 1;
            continue;
        }

        let priority = entry.priority_category(&download.header);
        let priority_value = entry.effective_priority(&download.header);

        required.push(ArtifactDescriptor {
            path: ekey_hex.clone(),
            content_key: ContentKey::from_bytes([0u8; 16]),
            encoding_key: entry.encoding_key,
            file_size: entry.file_size.as_u64() as u32,
            priority,
            priority_value,
        });
    }

    required.sort_by(|a, b| {
        a.priority_value
            .cmp(&b.priority_value)
            .then_with(|| a.encoding_key.as_bytes().cmp(b.encoding_key.as_bytes()))
    });

    // Shuffle within each priority bucket to spread CDN load across clients
    shuffle_within_priority_groups(&mut required);

    Ok(ArtifactSet {
        required,
        already_present,
        unresolved: 0,
    })
}

/// Classify download manifest entries for backfill, promoting all remaining
/// files to the highest download priority.
///
/// Agent.exe reads per-entry flag bytes from a local DL manifest copy to find
/// files that were partially or not-yet-downloaded, then promotes them to the
/// top of the queue. cascette uses atomic writes so there is no partial-write
/// state, but the intent is the same: backfill should download remaining files
/// as fast as possible, ignoring their manifest-assigned priority buckets.
///
/// All entries not present in `local_keys` are assigned `priority_value = i8::MIN`
/// so they sort before any freshly-installed files that respect manifest priority.
pub fn classify_backfill_artifacts<S: BuildHasher>(
    download: &DownloadManifest,
    tags: &[&str],
    local_keys: &std::collections::HashSet<String, S>,
) -> InstallationResult<ArtifactSet> {
    let entries = if tags.is_empty() {
        download.entries.iter().enumerate().collect::<Vec<_>>()
    } else {
        download.entries_by_tag_query(tags)
    };

    let mut required = Vec::new();
    let mut already_present: usize = 0;

    for (_idx, entry) in &entries {
        let ekey_hex = hex::encode(entry.encoding_key.as_bytes());

        if local_keys.contains(&ekey_hex) {
            already_present += 1;
            continue;
        }

        // Promote all remaining files to highest priority. This matches
        // Agent.exe's backfill behavior: partially-downloaded files (any
        // flag byte != 0) are placed at the front of the download queue.
        required.push(ArtifactDescriptor {
            path: ekey_hex.clone(),
            content_key: ContentKey::from_bytes([0u8; 16]),
            encoding_key: entry.encoding_key,
            file_size: entry.file_size.as_u64() as u32,
            priority: PriorityCategory::Critical,
            priority_value: i8::MIN,
        });
    }

    // No further sorting needed; all entries are at the same priority.
    // Shuffle to spread CDN load as with normal installs.
    shuffle_within_priority_groups(&mut required);

    Ok(ArtifactSet {
        required,
        already_present,
        unresolved: 0,
    })
}

/// Shuffle artifacts within each priority group.
///
/// Preserves the relative ordering between different priority levels
/// (high-priority files still download first) while randomizing the
/// order within each bucket. This spreads CDN load when many clients
/// download the same build simultaneously.
fn shuffle_within_priority_groups(items: &mut [ArtifactDescriptor]) {
    let mut rng = rng();
    let mut start = 0;
    while start < items.len() {
        let priority = items[start].priority_value;
        let end = items[start..]
            .iter()
            .position(|i| i.priority_value != priority)
            .map_or(items.len(), |p| start + p);
        items[start..end].shuffle(&mut rng);
        start = end;
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_formats::download::PriorityCategory;

    fn make_artifact(priority_value: i8, key_byte: u8) -> ArtifactDescriptor {
        ArtifactDescriptor {
            path: format!("file_{key_byte:02x}"),
            content_key: ContentKey::from_bytes([0u8; 16]),
            encoding_key: EncodingKey::from_bytes([key_byte; 16]),
            file_size: 100,
            priority: PriorityCategory::Normal,
            priority_value,
        }
    }

    #[test]
    fn test_shuffle_preserves_priority_ordering() {
        // Build artifacts: priority 1 (high), priority 3 (normal)
        let high_keys: Vec<u8> = (0..5).collect();
        let normal_keys: Vec<u8> = (10..15).collect();

        let mut items: Vec<ArtifactDescriptor> = high_keys
            .iter()
            .map(|&k| make_artifact(1, k))
            .chain(normal_keys.iter().map(|&k| make_artifact(3, k)))
            .collect();

        shuffle_within_priority_groups(&mut items);

        // All priority-1 items must come before all priority-3 items
        let split = items.iter().position(|i| i.priority_value == 3).unwrap();
        assert_eq!(split, 5);
        for item in &items[..5] {
            assert_eq!(item.priority_value, 1);
        }
        for item in &items[5..] {
            assert_eq!(item.priority_value, 3);
        }
    }

    #[test]
    fn test_shuffle_randomizes_within_bucket() {
        // Run multiple shuffles and check that at least one produces a different order.
        // With 10 items, the probability of getting the same order by chance is 1/10! ≈ 2.8e-7.
        let keys: Vec<u8> = (0..10).collect();
        let original: Vec<ArtifactDescriptor> = keys.iter().map(|&k| make_artifact(1, k)).collect();
        let original_order: Vec<u8> = original
            .iter()
            .map(|a| a.encoding_key.as_bytes()[0])
            .collect();

        let mut found_different = false;
        for _ in 0..10 {
            let mut items = original.clone();
            shuffle_within_priority_groups(&mut items);
            let order: Vec<u8> = items.iter().map(|a| a.encoding_key.as_bytes()[0]).collect();
            if order != original_order {
                found_different = true;
                break;
            }
        }
        assert!(
            found_different,
            "shuffle should produce different orderings"
        );
    }
}
