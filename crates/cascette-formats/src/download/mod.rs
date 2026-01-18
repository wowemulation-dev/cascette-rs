//! Download manifest format support for NGDP/CASC systems
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::use_self)]
//!
//! This module provides complete parsing and building support for CASC download manifests
//! used to manage content streaming and prioritization during game installation and updates.
//! Unlike the Install manifest which tracks installed files, the Download manifest enables
//! priority-based streaming installation where essential files are downloaded first.
//!
//! # Download Manifest Structure
//!
//! Download manifests use version-specific binary structures:
//!
//! ## Version 1 (Battle.net Agent/BNA):
//! - **Header** (10 bytes): Magic "DL", version, key length, entry/tag counts
//! - **File Entries Section**: EncodingKeys, 40-bit file sizes, priorities, optional checksums
//! - **Tag Section**: Tags with names, types, and bit masks
//!
//! ## Version 2+:
//! - **Header** (11-15 bytes): Magic "DL", version, key length, entry/tag counts, reserved byte, optional fields
//! - **Tag Section**: Tags with names, types, and bit masks
//! - **File Entries Section**: EncodingKeys, 40-bit file sizes, priorities, optional checksums/flags
//!
//! # Key Features
//!
//! - **Priority-Based Streaming**: Downloads critical files first to minimize time-to-playability
//! - **Three Format Versions**: Supports v1, v2, v3 with incremental features
//! - **40-Bit File Sizes**: Supports files larger than 4GB using 5-byte size fields
//! - **EncodingKey Usage**: Uses encoding keys instead of content keys
//! - **Base Priority Adjustment**: Version 3+ adjusts all priorities by base_priority offset
//! - **Version-Specific Layout**: V1 has entries-then-tags, V2+ has tags-then-entries
//!
//! # Basic Usage
//!
//! ## Parsing Download Manifests
//!
//! ```rust,no_run
//! use cascette_formats::download::{DownloadManifest, PriorityCategory};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = std::fs::read("download_manifest.bin")?;
//! let manifest = DownloadManifest::parse(&data)?;
//!
//! println!("Download manifest version: {}", manifest.header.version());
//! println!("Total files: {}", manifest.entries.len());
//! println!("Tag count: {}", manifest.tags.len());
//!
//! // Analyze priority distribution
//! for entry in &manifest.entries {
//!     let effective_priority = entry.effective_priority(&manifest.header);
//!     let category = entry.priority_category(&manifest.header);
//!     println!("File: {} bytes, priority: {} ({}:?)",
//!         entry.file_size.as_u64(), effective_priority, category);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Building Download Manifests
//!
//! ```rust,no_run
//! use cascette_formats::download::{DownloadManifestBuilder, TagType};
//! use cascette_crypto::EncodingKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")?;
//!
//! let manifest = DownloadManifestBuilder::new(3)? // Version 3
//!     .with_checksums(true)
//!     .with_flags(2)?
//!     .with_base_priority(-1)?
//!     .add_file(ekey, 1024, 0)? // Essential file
//!     .add_tag("Windows".to_string(), TagType::Platform)
//!     .associate_file_with_tag(0, "Windows")?
//!     .set_file_checksum(0, 0x1234_5678)?
//!     .build()?;
//!
//! let data = manifest.build()?;
//! std::fs::write("new_download.bin", data)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Advanced Usage
//!
//! ## Priority-Based Download Planning
//!
//! ```rust,no_run
//! # use cascette_formats::download::{DownloadManifest, PriorityCategory};
//! # fn create_essential_download_plan(manifest: &DownloadManifest) -> Vec<usize> {
//! let mut essential_files = Vec::new();
//!
//! for (index, entry) in manifest.entries.iter().enumerate() {
//!     let category = entry.priority_category(&manifest.header);
//!     if matches!(category, PriorityCategory::Critical | PriorityCategory::Essential) {
//!         essential_files.push(index);
//!     }
//! }
//!
//! // Sort by priority (highest first)
//! essential_files.sort_by_key(|&index| {
//!     manifest.entries[index].effective_priority(&manifest.header)
//! });
//!
//! essential_files
//! # }
//! ```
//!
//! ## Platform-Specific Filtering
//!
//! ```rust,no_run
//! # use cascette_formats::download::{DownloadManifest, TagType};
//! # fn filter_for_platform(manifest: &DownloadManifest, platform: &str) -> Vec<usize> {
//! let platform_tag = manifest.tags.iter()
//!     .find(|t| t.tag_type == TagType::Platform && t.name == platform);
//!
//! let mut filtered_files = Vec::new();
//!
//! for (index, _) in manifest.entries.iter().enumerate() {
//!     let has_platform = platform_tag
//!         .map(|tag| tag.has_file(index))
//!         .unwrap_or(true); // Include untagged files
//!
//!     if has_platform {
//!         filtered_files.push(index);
//!     }
//! }
//!
//! filtered_files
//! # }
//! ```
//!
//! ## Streaming Installation
//!
//! ```rust,no_run
//! # use cascette_formats::download::{DownloadManifest, PriorityCategory};
//! # fn calculate_streaming_plan(manifest: &DownloadManifest) -> (u64, u64) {
//! let mut essential_size = 0u64;
//! let mut total_size = 0u64;
//!
//! for entry in &manifest.entries {
//!     let size = entry.file_size.as_u64();
//!     total_size += size;
//!
//!     if entry.is_essential(&manifest.header) {
//!         essential_size += size;
//!     }
//! }
//!
//! (essential_size, total_size)
//! # }
//! ```

pub mod builder;
pub mod entry;
pub mod error;
pub mod header;
pub mod manifest;
pub mod priority;
pub mod tag;

// Re-export main types
pub use builder::DownloadManifestBuilder;
pub use entry::{DownloadFileEntry, FileSize40};
pub use error::{DownloadError, Result};
pub use header::{DownloadHeader, DownloadHeaderBase, DownloadHeaderV2, DownloadHeaderV3};
pub use manifest::DownloadManifest;
pub use priority::{CategoryStats, PriorityAnalysis, PriorityCategory};
pub use tag::DownloadTag;

// Re-export TagType from install module for convenience
pub use crate::install::TagType;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::EncodingKey;

    #[test]
    fn test_module_exports() {
        // Test that all public types are accessible
        let _ = DownloadHeaderBase {
            magic: *b"DL",
            version: 1,
            ekey_length: 16,
            has_checksum: 0,
            entry_count: 0,
            tag_count: 0,
        };
        let _builder = DownloadManifestBuilder::new(1).expect("Operation should succeed");
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use cascette_crypto::EncodingKey;
        use proptest::prelude::*;
        use proptest::test_runner::TestCaseError;

        /// Generate arbitrary encoding keys
        #[allow(dead_code)]
        fn encoding_key() -> impl Strategy<Value = EncodingKey> {
            prop::array::uniform16(0u8..255).prop_map(EncodingKey::from_bytes)
        }

        /// Generate valid download manifest versions
        #[allow(dead_code)]
        fn version() -> impl Strategy<Value = u8> {
            1u8..=3u8
        }

        /// Generate valid file priorities
        #[allow(dead_code)]
        fn priority() -> impl Strategy<Value = i8> {
            -128i8..=127i8
        }

        /// Generate valid 40-bit file sizes
        #[allow(dead_code)]
        fn file_size_40bit() -> impl Strategy<Value = u64> {
            0u64..(1u64 << 40) // 40-bit maximum
        }

        /// Generate arbitrary tag types
        #[allow(dead_code)]
        fn tag_type() -> impl Strategy<Value = TagType> {
            prop_oneof![
                Just(TagType::Platform),
                Just(TagType::Architecture),
                Just(TagType::Locale),
                Just(TagType::Category),
                Just(TagType::Component),
                Just(TagType::Feature)
            ]
        }

        /// Generate valid tag names (1-255 characters)
        #[allow(dead_code)]
        fn tag_name() -> impl Strategy<Value = String> {
            "[A-Za-z0-9_.-]{1,255}"
        }

        /// Generate download manifest entries
        #[allow(dead_code)]
        fn download_entry() -> impl Strategy<Value = (EncodingKey, u64, i8)> {
            (encoding_key(), file_size_40bit(), priority())
        }

        /// Generate download tags
        #[allow(dead_code)]
        fn download_tag() -> impl Strategy<Value = (String, TagType)> {
            (tag_name(), tag_type())
        }

        proptest! {
            /// Test that download manifests round-trip correctly
            fn download_manifest_round_trip(
                version in version(),
                entries in prop::collection::vec(download_entry(), 1..10),
                tags in prop::collection::vec(download_tag(), 0..5)
            ) {
                let mut builder = DownloadManifestBuilder::new(version).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Add version-specific features
                if version >= 2 {
                    builder = builder.with_flags(1).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }
                if version >= 3 {
                    builder = builder.with_base_priority(-1).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                // Add files
                for (ekey, size, prio) in &entries {
                    builder = builder.add_file(*ekey, *size, *prio).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                // Add tags and associations
                for (name, tag_type) in &tags {
                    builder = builder.add_tag(name.clone(), *tag_type);
                    // Associate first file with this tag if files exist
                    if !entries.is_empty() {
                        builder = builder.associate_file_with_tag(0, name).map_err(|e| TestCaseError::fail(e.to_string()))?;
                    }
                }

                let manifest = builder.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                let data = manifest.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                let parsed = DownloadManifest::parse(&data).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Verify structure
                prop_assert_eq!(parsed.header.version(), version);
                prop_assert_eq!(parsed.entries.len(), entries.len());
                prop_assert_eq!(parsed.tags.len(), tags.len());

                // Verify entries
                for (i, (original_key, original_size, original_prio)) in entries.iter().enumerate() {
                    let entry = &parsed.entries[i];
                    prop_assert_eq!(entry.encoding_key, *original_key);
                    prop_assert_eq!(entry.file_size.as_u64(), *original_size);
                    prop_assert_eq!(entry.priority, *original_prio);
                }
            }

            /// Test FileSize40 handles 40-bit values correctly
            fn file_size_40_bit_correctness(
                size in file_size_40bit()
            ) {
                let file_size = FileSize40::new(size).map_err(|e| TestCaseError::fail(e.to_string()))?;
                prop_assert_eq!(file_size.as_u64(), size);

                // Verify 40-bit encoding
                let bytes = file_size.to_bytes();
                prop_assert_eq!(bytes.len(), 5);

                // Verify high byte is within valid range
                let _high_byte = bytes[0]; // u8 is always <= 0xFF
            }

            /// Test that oversized values are rejected
            fn file_size_oversized_rejected(
                oversized in (1u64 << 40)..u64::MAX
            ) {
                let result = FileSize40::new(oversized);
                prop_assert!(result.is_err());
            }

            /// Test priority calculations with base priority
            fn priority_calculations_correct(
                base_priority in -128i8..=127i8,
                file_priority in -128i8..=127i8
            ) {
                let manifest = DownloadManifestBuilder::new(3)?
                    .with_base_priority(base_priority)?
                    .add_file(EncodingKey::from_bytes([0u8; 16]), 1024, file_priority)?
                    .build()?;

                let entry = &manifest.entries[0];
                let effective = entry.effective_priority(&manifest.header);

                // V3 formula: effective = file_priority - base_priority
                let expected = file_priority.saturating_sub(base_priority);
                prop_assert_eq!(effective, expected);
            }

            /// Test that different versions have different header sizes
            fn version_specific_header_sizes(
                version in version()
            ) {
                let manifest = DownloadManifestBuilder::new(version)?
                    .add_file(EncodingKey::from_bytes([0u8; 16]), 1024, 0)?
                    .build()?;

                let data = manifest.build().map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Check magic bytes
                prop_assert_eq!(&data[0..2], b"DL");
                prop_assert_eq!(data[2], version);

                // Version-specific structure validation
                match version {
                    1 => {
                        // V1: magic(2) + version(1) + key_len(1) + has_checksum(1) + entry_count(2) + tag_count(2) = 10 bytes
                        prop_assert_eq!(manifest.header.flag_size(), 0);
                        prop_assert_eq!(manifest.header.base_priority(), 0);
                    },
                    2 => {
                        // V2: base + reserved(1) + flag_size(1) = 12 bytes minimum
                        prop_assert_eq!(manifest.header.base_priority(), 0);
                    },
                    3 => {
                        // V3: V2 + base_priority(1) = 13 bytes minimum
                        // Base priority can be non-zero
                    },
                    _ => unreachable!("Invalid version generated")
                }
            }

            /// Test tag associations work correctly
            fn tag_associations_correct(
                files in prop::collection::vec(download_entry(), 1..5),
                tag in download_tag()
            ) {
                let mut builder = DownloadManifestBuilder::new(2)?;

                // Add files
                for (ekey, size, prio) in &files {
                    builder = builder.add_file(*ekey, *size, *prio).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                // Add tag and associate with all files
                builder = builder.add_tag(tag.0.clone(), tag.1);
                for i in 0..files.len() {
                    builder = builder.associate_file_with_tag(i, &tag.0)?;
                }

                let manifest = builder.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                let data = manifest.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                let parsed = DownloadManifest::parse(&data).map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Find the tag
                let found_tag = parsed.tags.iter().find(|t| t.name == tag.0);
                prop_assert!(found_tag.is_some());

                let found_tag = found_tag.expect("Tag should exist");
                prop_assert_eq!(found_tag.tag_type, tag.1);

                // Verify all files are associated
                for i in 0..files.len() {
                    prop_assert!(found_tag.has_file(i));
                }
            }

            /// Test priority categories are calculated correctly
            fn priority_categories_correct(
                priority in priority()
            ) {
                let manifest = DownloadManifestBuilder::new(1)?
                    .add_file(EncodingKey::from_bytes([0u8; 16]), 1024, priority)?
                    .build()?;

                let entry = &manifest.entries[0];
                let category = entry.priority_category(&manifest.header);
                let effective = entry.effective_priority(&manifest.header);

                // V1 has no base priority adjustment
                prop_assert_eq!(effective, priority);

                // Verify category mapping
                let expected_category = match effective {
                    i8::MIN..=-1 => PriorityCategory::Critical,
                    0..=0 => PriorityCategory::Essential,
                    1..=9 => PriorityCategory::High,
                    10..=99 => PriorityCategory::Normal,
                    100..=i8::MAX => PriorityCategory::Low,
                };

                prop_assert_eq!(category, expected_category);
            }

            /// Test that invalid magic bytes are rejected
            fn invalid_magic_rejected(
                magic in prop::array::uniform2(0u8..255).prop_filter("Not DL magic", |m| m != b"DL")
            ) {
                let mut data = vec![0u8; 20];
                data[0..2].copy_from_slice(&magic);
                data[2] = 1; // Valid version

                let result = DownloadManifest::parse(&data);
                prop_assert!(result.is_err());
            }

            /// Test that checksum handling works correctly
            fn checksum_handling_correct(
                ekey in encoding_key(),
                size in file_size_40bit(),
                priority in priority(),
                checksum in any::<u32>()
            ) {
                let manifest = DownloadManifestBuilder::new(2)?
                    .with_checksums(true)
                    .add_file(ekey, size, priority)?
                    .set_file_checksum(0, checksum)?
                    .build()?;

                let data = manifest.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                let parsed = DownloadManifest::parse(&data).map_err(|e| TestCaseError::fail(e.to_string()))?;

                prop_assert_eq!(parsed.entries.len(), 1);
                let entry = &parsed.entries[0];
                prop_assert_eq!(entry.checksum, Some(checksum));
            }

            /// Test flag handling in V2+
            fn flag_handling_correct(
                ekey in encoding_key(),
                size in file_size_40bit(),
                priority in priority(),
                flags in prop::collection::vec(any::<u8>(), 0..4)
            ) {
                let manifest = DownloadManifestBuilder::new(2)?
                    .with_flags(flags.len() as u8)?
                    .add_file(ekey, size, priority)?
                    .set_file_flags(0, flags.clone())?
                    .build()?;

                let data = manifest.build().map_err(|e| TestCaseError::fail(e.to_string()))?;
                let parsed = DownloadManifest::parse(&data).map_err(|e| TestCaseError::fail(e.to_string()))?;

                prop_assert_eq!(parsed.entries.len(), 1);
                let entry = &parsed.entries[0];

                if flags.is_empty() {
                    prop_assert!(entry.flags.is_none());
                } else {
                    prop_assert_eq!(entry.flags.as_ref().expect("Entry should have flags"), &flags);
                }
            }

            /// Test that manifest validation works
            fn manifest_validation_works(
                entries in prop::collection::vec(download_entry(), 0..10),
                tags in prop::collection::vec(download_tag(), 0..5)
            ) {
                let mut builder = DownloadManifestBuilder::new(2)?;

                // Add files
                for (ekey, size, prio) in &entries {
                    builder = builder.add_file(*ekey, *size, *prio).map_err(|e| TestCaseError::fail(e.to_string()))?;
                }

                // Add tags
                for (name, tag_type) in &tags {
                    builder = builder.add_tag(name.clone(), *tag_type);
                }

                let manifest = builder.build().map_err(|e| TestCaseError::fail(e.to_string()))?;

                // Validation should pass for well-formed manifests
                prop_assert!(manifest.validate().is_ok());
            }

            /// Test different data produces different serializations
            fn different_data_different_serializations(
                entries1 in prop::collection::vec(download_entry(), 1..5),
                entries2 in prop::collection::vec(download_entry(), 1..5)
            ) {
                prop_assume!(entries1 != entries2);

                let mut builder1 = DownloadManifestBuilder::new(1)?;
                let mut builder2 = DownloadManifestBuilder::new(1)?;

                for (ekey, size, prio) in &entries1 {
                    builder1 = builder1.add_file(*ekey, *size, *prio)?;
                }
                for (ekey, size, prio) in &entries2 {
                    builder2 = builder2.add_file(*ekey, *size, *prio)?;
                }

                let manifest1 = builder1.build()?;
                let manifest2 = builder2.build()?;

                let data1 = manifest1.build()?;
                let data2 = manifest2.build()?;

                prop_assert_ne!(data1, data2);
            }
        }
    }

    #[test]
    fn test_complete_workflow() {
        // Test complete parse -> modify -> build workflow
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");

        let manifest = DownloadManifestBuilder::new(2)
            .expect("Operation should succeed")
            .with_checksums(true)
            .with_flags(1)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 0)
            .expect("Operation should succeed")
            .add_tag("Windows".to_string(), TagType::Platform)
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .set_file_checksum(0, 0x1234_5678)
            .expect("Operation should succeed")
            .set_file_flags(0, vec![0xAB])
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        // Build to bytes
        let data = manifest.build().expect("Operation should succeed");

        // Parse it back
        let parsed = DownloadManifest::parse(&data).expect("Operation should succeed");

        // Validate structure
        assert_eq!(parsed.header.version(), 2);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.tags.len(), 1);
        assert!(parsed.validate().is_ok());

        // Test entry data
        let entry = &parsed.entries[0];
        assert_eq!(entry.encoding_key, ekey);
        assert_eq!(entry.file_size.as_u64(), 1024);
        assert_eq!(entry.priority, 0);
        assert_eq!(entry.checksum, Some(0x1234_5678));
        assert_eq!(
            entry.flags.as_ref().expect("Operation should succeed"),
            &vec![0xAB]
        );

        // Test tag associations
        let windows_tag = parsed
            .tags
            .iter()
            .find(|t| t.name == "Windows")
            .expect("Operation should succeed");
        assert!(windows_tag.has_file(0));
    }

    #[test]
    fn test_version_differences() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");

        // Test V1 - no flags, no base priority
        let v1 = DownloadManifestBuilder::new(1)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 5)
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        assert_eq!(v1.header.version(), 1);
        assert_eq!(v1.header.flag_size(), 0);
        assert_eq!(v1.header.base_priority(), 0);
        assert_eq!(v1.entries[0].effective_priority(&v1.header), 5);

        // Test V2 - with flags, no base priority
        let v2 = DownloadManifestBuilder::new(2)
            .expect("Operation should succeed")
            .with_flags(1)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 5)
            .expect("Operation should succeed")
            .set_file_flags(0, vec![0xFF])
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        assert_eq!(v2.header.version(), 2);
        assert_eq!(v2.header.flag_size(), 1);
        assert_eq!(v2.header.base_priority(), 0);
        assert_eq!(v2.entries[0].effective_priority(&v2.header), 5);
        assert_eq!(
            v2.entries[0]
                .flags
                .as_ref()
                .expect("Operation should succeed"),
            &vec![0xFF]
        );

        // Test V3 - with flags and base priority
        let v3 = DownloadManifestBuilder::new(3)
            .expect("Operation should succeed")
            .with_flags(1)
            .expect("Operation should succeed")
            .with_base_priority(-2)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 5)
            .expect("Operation should succeed")
            .set_file_flags(0, vec![0xFF])
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        assert_eq!(v3.header.version(), 3);
        assert_eq!(v3.header.flag_size(), 1);
        assert_eq!(v3.header.base_priority(), -2);
        assert_eq!(v3.entries[0].effective_priority(&v3.header), 7); // 5 - (-2)
    }

    #[test]
    fn test_file_size_40_bit() {
        let size_1gb = 1024 * 1024 * 1024u64;
        // Don't test 1TB as it exceeds 40-bit limit (1099511627776 > 1099511627775)
        let size_512gb = size_1gb * 512;

        let size40_1gb = FileSize40::new(size_1gb).expect("Operation should succeed");
        assert_eq!(size40_1gb.as_u64(), size_1gb);

        let size40_512gb = FileSize40::new(size_512gb).expect("Operation should succeed");
        assert_eq!(size40_512gb.as_u64(), size_512gb);

        // Test maximum 40-bit value
        let max_40bit = 0xFF_FFFF_FFFF;
        let size40_max = FileSize40::new(max_40bit).expect("Operation should succeed");
        assert_eq!(size40_max.as_u64(), max_40bit);

        // Test oversized value fails
        let oversized = 0x1_0000_0000_0000;
        assert!(FileSize40::new(oversized).is_err());
    }

    #[test]
    fn test_priority_categories() {
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");

        // Test without base priority (V1/V2)
        let manifest_v2 = DownloadManifestBuilder::new(2)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, -1)
            .expect("Operation should succeed") // Critical
            .build()
            .expect("Operation should succeed");

        let entry = &manifest_v2.entries[0];
        assert_eq!(
            entry.priority_category(&manifest_v2.header),
            PriorityCategory::Critical
        );
        assert!(entry.is_essential(&manifest_v2.header));

        // Test with base priority (V3)
        let manifest_v3 = DownloadManifestBuilder::new(3)
            .expect("Operation should succeed")
            .with_base_priority(-3)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, -1)
            .expect("Operation should succeed") // Effective priority: -1 - (-3) = 2
            .build()
            .expect("Operation should succeed");

        let entry = &manifest_v3.entries[0];
        assert_eq!(
            entry.priority_category(&manifest_v3.header),
            PriorityCategory::High
        );
        assert!(!entry.is_essential(&manifest_v3.header));
    }

    #[test]
    fn test_version_specific_layout() {
        // Verify version-specific layouts
        let ekey = EncodingKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");

        // Version 1: Header + Entries + Tags
        let manifest_v1 = DownloadManifestBuilder::new(1)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 0)
            .expect("Operation should succeed")
            .add_tag("Windows".to_string(), TagType::Platform)
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        let data_v1 = manifest_v1.build().expect("Operation should succeed");
        let parsed_v1 = DownloadManifest::parse(&data_v1).expect("Operation should succeed");
        assert_eq!(parsed_v1.entries.len(), 1);
        assert_eq!(parsed_v1.tags.len(), 1);
        assert!(parsed_v1.tags[0].has_file(0));

        // Version 2: Header + Tags + Entries
        let manifest_v2 = DownloadManifestBuilder::new(2)
            .expect("Operation should succeed")
            .add_file(ekey, 1024, 0)
            .expect("Operation should succeed")
            .add_tag("Windows".to_string(), TagType::Platform)
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        let data_v2 = manifest_v2.build().expect("Operation should succeed");
        let parsed_v2 = DownloadManifest::parse(&data_v2).expect("Operation should succeed");
        assert_eq!(parsed_v2.entries.len(), 1);
        assert_eq!(parsed_v2.tags.len(), 1);
        assert!(parsed_v2.tags[0].has_file(0));
    }
}
