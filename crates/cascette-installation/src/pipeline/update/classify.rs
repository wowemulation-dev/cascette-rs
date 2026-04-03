//! Extended artifact classification for build updates.
//!
//! Classifies install manifest entries into four categories for an update
//! operation: required (full download), partial (resume), inflight (patchable),
//! and leechable (available from alternate installation).

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use tracing::debug;

use cascette_client_storage::container::residency::ResidencyContainer;
use cascette_formats::download::DownloadManifest;
use cascette_formats::encoding::EncodingFile;
use cascette_formats::install::InstallManifest;
use cascette_formats::patch_chain::PatchChain;

use rand::rng;
use rand::seq::SliceRandom;

use crate::error::InstallationResult;
use crate::pipeline::classify::ArtifactDescriptor;

/// Classification category for an update artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateCategory {
    /// Not in any container, must download fully.
    Required,
    /// Partially downloaded, resume remaining blocks.
    Partial,
    /// Has in-progress patch chains.
    Inflight,
}

/// A single artifact in the update plan.
#[derive(Debug, Clone)]
pub struct UpdateArtifact {
    /// Base artifact descriptor (path, keys, size, priority).
    pub descriptor: ArtifactDescriptor,
    /// Update category.
    pub category: UpdateCategory,
    /// Bytes already present locally (>0 for partial files).
    pub bytes_present: u64,
    /// Whether this artifact is available in an alternate container.
    pub leechable: bool,
    /// Patch chain from base to target version, if one exists.
    pub patch_chain: Option<PatchChain>,
}

/// A file path that exists in the base build but not in the target build.
///
/// Status 6 (obsolete) in the update pipeline.
#[derive(Debug, Clone)]
pub struct ObsoleteFile {
    /// File path from the base install manifest.
    pub path: String,
}

/// Classification result for update artifacts.
#[derive(Debug)]
pub struct UpdateArtifactSet {
    /// Artifacts that must be fully downloaded.
    pub required: Vec<UpdateArtifact>,
    /// Partially-downloaded artifacts, sorted by completion priority.
    pub partial: Vec<UpdateArtifact>,
    /// Artifacts with patch chains from the base version.
    pub inflight: Vec<UpdateArtifact>,
    /// Subset of artifacts available via alternate container leeching.
    pub leechable: Vec<UpdateArtifact>,
    /// Files present in base build but absent from target build.
    pub obsolete: Vec<ObsoleteFile>,
    /// Number of artifacts already fully present.
    pub already_present: usize,
    /// Number of artifacts that could not be resolved in the encoding file.
    pub unresolved: usize,
}

impl UpdateArtifactSet {
    /// Total bytes to download across all categories.
    #[must_use]
    pub fn total_download_bytes(&self) -> u64 {
        let required: u64 = self
            .required
            .iter()
            .map(|a| u64::from(a.descriptor.file_size))
            .sum();
        let partial: u64 = self
            .partial
            .iter()
            .map(|a| u64::from(a.descriptor.file_size).saturating_sub(a.bytes_present))
            .sum();
        let inflight: u64 = self
            .inflight
            .iter()
            .map(|a| u64::from(a.descriptor.file_size))
            .sum();
        required + partial + inflight
    }
}

/// Classify artifacts for an update operation.
///
/// For each install manifest entry matching the given tags:
/// 1. Look up encoding key via encoding file
/// 2. Query residency container: fully resident -> already_present, partial -> Partial
/// 3. Query alternate residency (if provided): resident -> mark leechable
/// 4. Check patch_chains map: if chain exists -> Inflight
/// 5. Non-block-encoded partials -> moved to Required with log message
#[allow(clippy::too_many_arguments)]
pub fn classify_update_artifacts<S: BuildHasher, S2: BuildHasher>(
    install: &InstallManifest,
    encoding: &EncodingFile,
    download: &DownloadManifest,
    tags: &[&str],
    residency: &ResidencyContainer,
    alternate_residency: Option<&ResidencyContainer>,
    patch_chains: &HashMap<[u8; 16], PatchChain, S>,
    known_keys: &HashSet<String, S2>,
    is_resuming: bool,
) -> InstallationResult<UpdateArtifactSet> {
    use cascette_formats::download::PriorityCategory;

    // Build download priority lookup
    let mut download_priority: HashMap<[u8; 16], (PriorityCategory, i8)> = HashMap::new();
    for entry in &download.entries {
        let priority = entry.priority_category(&download.header);
        let priority_value = entry.effective_priority(&download.header);
        download_priority.insert(*entry.encoding_key.as_bytes(), (priority, priority_value));
    }

    let files = if tags.is_empty() {
        install.entries.iter().enumerate().collect::<Vec<_>>()
    } else {
        install.get_files_for_tag_query(tags)
    };

    let mut required = Vec::new();
    let mut partial = Vec::new();
    let mut inflight = Vec::new();
    let mut leechable = Vec::new();
    let mut already_present: usize = 0;
    let mut unresolved: usize = 0;

    for (_idx, entry) in &files {
        let Some(ekey) = encoding.find_encoding(&entry.content_key) else {
            unresolved += 1;
            continue;
        };

        let ekey_bytes = *ekey.as_bytes();

        // Check residency
        let is_resident = residency.is_resident(&ekey_bytes);
        if is_resident {
            already_present += 1;
            continue;
        }

        // Skip keys already completed in a previous checkpoint
        let ekey_hex = hex::encode(ekey_bytes);
        if known_keys.contains(&ekey_hex) {
            already_present += 1;
            continue;
        }

        let (priority, priority_value) = download_priority
            .get(&ekey_bytes)
            .copied()
            .unwrap_or((PriorityCategory::Normal, 3));

        let descriptor = ArtifactDescriptor {
            path: entry.path.clone(),
            content_key: entry.content_key,
            encoding_key: ekey,
            file_size: entry.file_size,
            priority,
            priority_value,
        };

        // Check alternate residency for leeching
        let is_leechable = alternate_residency.is_some_and(|alt| alt.is_resident(&ekey_bytes));

        // Check patch chains
        if let Some(chain) = patch_chains.get(&ekey_bytes) {
            let artifact = UpdateArtifact {
                descriptor,
                category: UpdateCategory::Inflight,
                bytes_present: 0,
                leechable: is_leechable,
                patch_chain: Some(chain.clone()),
            };
            if is_leechable {
                leechable.push(artifact.clone());
            }
            inflight.push(artifact);
            continue;
        }

        // When resuming an interrupted session, non-patchable files that were
        // queued but not completed get Partial category (highest download priority).
        let category = if is_resuming {
            UpdateCategory::Partial
        } else {
            UpdateCategory::Required
        };

        if is_leechable {
            let artifact = UpdateArtifact {
                descriptor,
                category,
                bytes_present: 0,
                leechable: true,
                patch_chain: None,
            };
            leechable.push(artifact.clone());
            if is_resuming {
                partial.push(artifact);
            } else {
                required.push(artifact);
            }
        } else {
            let artifact = UpdateArtifact {
                descriptor,
                category,
                bytes_present: 0,
                leechable: false,
                patch_chain: None,
            };
            if is_resuming {
                partial.push(artifact);
            } else {
                required.push(artifact);
            }
        }
    }

    // Sort required by priority then encoding key
    required.sort_by(|a, b| {
        a.descriptor
            .priority_value
            .cmp(&b.descriptor.priority_value)
            .then_with(|| {
                a.descriptor
                    .encoding_key
                    .as_bytes()
                    .cmp(b.descriptor.encoding_key.as_bytes())
            })
    });

    // Shuffle within each priority bucket to spread CDN load across clients
    {
        let mut rng = rng();
        let mut start = 0;
        while start < required.len() {
            let priority = required[start].descriptor.priority_value;
            let end = required[start..]
                .iter()
                .position(|i| i.descriptor.priority_value != priority)
                .map_or(required.len(), |p| start + p);
            required[start..end].shuffle(&mut rng);
            start = end;
        }
    }

    // Sort partial by bytes remaining (ascending = closest to completion first)
    partial.sort_by(|a: &UpdateArtifact, b: &UpdateArtifact| {
        let remaining_a = u64::from(a.descriptor.file_size).saturating_sub(a.bytes_present);
        let remaining_b = u64::from(b.descriptor.file_size).saturating_sub(b.bytes_present);
        remaining_a.cmp(&remaining_b)
    });

    debug!(
        required = required.len(),
        partial = partial.len(),
        inflight = inflight.len(),
        leechable = leechable.len(),
        already_present = already_present,
        unresolved = unresolved,
        "update artifact classification complete"
    );

    Ok(UpdateArtifactSet {
        required,
        partial,
        inflight,
        leechable,
        obsolete: Vec::new(), // Populated by detect_obsolete_files() in the pipeline
        already_present,
        unresolved,
    })
}

/// Detect files that exist in the base build but not in the target build.
///
/// Compares tag-filtered file sets from both install manifests. Files present
/// in `base` but absent from `target` are marked obsolete.
///
/// Only meaningful for updates — fresh installs have no base manifest.
pub fn detect_obsolete_files(
    base: &InstallManifest,
    target: &InstallManifest,
    tags: &[&str],
) -> Vec<ObsoleteFile> {
    let base_files = if tags.is_empty() {
        base.entries.iter().enumerate().collect::<Vec<_>>()
    } else {
        base.get_files_for_tag_query(tags)
    };

    let target_files = if tags.is_empty() {
        target.entries.iter().enumerate().collect::<Vec<_>>()
    } else {
        target.get_files_for_tag_query(tags)
    };

    // Build set of target file paths for O(1) lookup
    let target_paths: HashSet<&str> = target_files
        .iter()
        .map(|(_, entry)| entry.path.as_str())
        .collect();

    let mut obsolete = Vec::new();
    for (_, entry) in &base_files {
        if !target_paths.contains(entry.path.as_str()) {
            obsolete.push(ObsoleteFile {
                path: entry.path.clone(),
            });
        }
    }

    if !obsolete.is_empty() {
        debug!(count = obsolete.len(), "detected obsolete files");
    }

    obsolete
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_update_category_equality() {
        assert_eq!(UpdateCategory::Required, UpdateCategory::Required);
        assert_ne!(UpdateCategory::Required, UpdateCategory::Partial);
        assert_ne!(UpdateCategory::Partial, UpdateCategory::Inflight);
    }

    #[test]
    fn test_artifact_set_total_bytes() {
        use cascette_crypto::{ContentKey, EncodingKey};
        use cascette_formats::download::PriorityCategory;

        let make_artifact =
            |size: u32, bytes_present: u64, category: UpdateCategory| UpdateArtifact {
                descriptor: ArtifactDescriptor {
                    path: "test".to_string(),
                    content_key: ContentKey::from_bytes([0u8; 16]),
                    encoding_key: EncodingKey::from_bytes([0u8; 16]),
                    file_size: size,
                    priority: PriorityCategory::Normal,
                    priority_value: 3,
                },
                category,
                bytes_present,
                leechable: false,
                patch_chain: None,
            };

        let set = UpdateArtifactSet {
            required: vec![make_artifact(1000, 0, UpdateCategory::Required)],
            partial: vec![make_artifact(500, 200, UpdateCategory::Partial)],
            inflight: vec![make_artifact(800, 0, UpdateCategory::Inflight)],
            leechable: vec![],
            obsolete: vec![],
            already_present: 5,
            unresolved: 1,
        };

        // required=1000, partial=500-200=300, inflight=800 => 2100
        assert_eq!(set.total_download_bytes(), 2100);
    }

    fn make_manifest(paths: &[&str]) -> InstallManifest {
        use cascette_crypto::ContentKey;
        use cascette_formats::install::{InstallFileEntry, InstallHeader};

        InstallManifest {
            header: InstallHeader {
                magic: *b"IN",
                version: 1,
                ckey_length: 16,
                content_key_size: Some(16),
                tag_count: 0,
                entry_count: paths.len() as u32,
                entry_count_v2: None,
                v2_unknown: None,
            },
            tags: vec![],
            entries: paths
                .iter()
                .map(|p| InstallFileEntry {
                    path: p.to_string(),
                    content_key: ContentKey::from_bytes([0u8; 16]),
                    file_size: 100,
                    file_type: Some(0),
                })
                .collect(),
        }
    }

    #[test]
    fn test_detect_obsolete_files_finds_removed() {
        let base = make_manifest(&["WoW.exe", "Data/common.db2", "Data/old_texture.blp"]);
        let target = make_manifest(&["WoW.exe", "Data/common.db2"]);

        let obsolete = detect_obsolete_files(&base, &target, &[]);
        assert_eq!(obsolete.len(), 1);
        assert_eq!(obsolete[0].path, "Data/old_texture.blp");
    }

    #[test]
    fn test_detect_obsolete_files_empty_when_identical() {
        let base = make_manifest(&["WoW.exe", "Data/common.db2"]);
        let target = make_manifest(&["WoW.exe", "Data/common.db2"]);

        let obsolete = detect_obsolete_files(&base, &target, &[]);
        assert!(obsolete.is_empty());
    }

    #[test]
    fn test_detect_obsolete_files_empty_when_target_adds_files() {
        let base = make_manifest(&["WoW.exe"]);
        let target = make_manifest(&["WoW.exe", "Data/new_file.db2"]);

        let obsolete = detect_obsolete_files(&base, &target, &[]);
        assert!(obsolete.is_empty());
    }

    #[test]
    fn test_detect_obsolete_files_multiple_removals() {
        let base = make_manifest(&["a.exe", "b.dll", "c.dat", "d.cfg"]);
        let target = make_manifest(&["a.exe"]);

        let obsolete = detect_obsolete_files(&base, &target, &[]);
        assert_eq!(obsolete.len(), 3);
        let paths: Vec<&str> = obsolete.iter().map(|o| o.path.as_str()).collect();
        assert!(paths.contains(&"b.dll"));
        assert!(paths.contains(&"c.dat"));
        assert!(paths.contains(&"d.cfg"));
    }
}
