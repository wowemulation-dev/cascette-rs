//! Install manifest format support for NGDP/CASC systems
//!
//! This module provides complete parsing and building support for CASC install manifests
//! used to track which game files should be installed on disk and manage file tags
//! for selective installation based on system requirements.
//!
//! # Install Manifest Structure
//!
//! Install manifests use the following binary structure:
//!
//! - **Header** (10 bytes): Magic "IN", version, key length, tag/entry counts
//! - **Tag Section**: Variable-length tags with names, types, and bit masks
//! - **File Entries**: Variable-length file paths, content keys, and sizes
//!
//! # Key Features
//!
//! - **Tag-Based Filtering**: Files are categorized using bit masks for selective installation
//! - **Platform Support**: Tags identify platform, architecture, locale requirements
//! - **Size Calculation**: File sizes enable disk space planning
//! - **Big-Endian Format**: Multi-byte fields use big-endian encoding
//! - **Round-Trip Support**: Parse and rebuild produce identical output
//!
//! # Basic Usage
//!
//! ## Parsing Install Manifests
//!
//! ```rust,no_run
//! use cascette_formats::install::{InstallManifest, TagType};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = std::fs::read("install_manifest.bin")?;
//! let manifest = InstallManifest::parse(&data)?;
//!
//! println!("Install manifest version: {}", manifest.header.version);
//! println!("Total files: {}", manifest.entries.len());
//! println!("Tag count: {}", manifest.tags.len());
//!
//! // Find platform-specific files
//! for tag in &manifest.tags {
//!     if tag.tag_type == TagType::Platform {
//!         println!("Platform tag: {} ({} files)", tag.name, tag.file_count());
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Building Install Manifests
//!
//! ```rust,no_run
//! use cascette_formats::install::{InstallManifestBuilder, TagType};
//! use cascette_crypto::ContentKey;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let manifest = InstallManifestBuilder::new()
//!     .add_tag("Windows".to_string(), TagType::Platform)
//!     .add_tag("x86_64".to_string(), TagType::Architecture)
//!     .add_file(
//!         "data/file1.bin".to_string(),
//!         ContentKey::from_hex("0123456789abcdef0123456789abcdef")?,
//!         1024,
//!     )
//!     .associate_file_with_tag(0, "Windows")?
//!     .associate_file_with_tag(0, "x86_64")?
//!     .build()?;
//!
//! let data = manifest.build()?;
//! std::fs::write("new_install.bin", data)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Advanced Usage
//!
//! ## Platform-Specific Installation
//!
//! ```rust,no_run
//! # use cascette_formats::install::{InstallManifest, TagType};
//! # fn get_platform_files<'a>(manifest: &'a InstallManifest, platform: &str, architecture: &str) -> Vec<&'a cascette_formats::install::InstallFileEntry> {
//! let platform_tag = manifest.tags.iter()
//!     .find(|t| t.tag_type == TagType::Platform && t.name == platform);
//! let arch_tag = manifest.tags.iter()
//!     .find(|t| t.tag_type == TagType::Architecture && t.name == architecture);
//!
//! let mut files = Vec::new();
//!
//! for (index, entry) in manifest.entries.iter().enumerate() {
//!     let has_platform = platform_tag
//!         .map(|t| t.has_file(index))
//!         .unwrap_or(true);
//!     let has_arch = arch_tag
//!         .map(|t| t.has_file(index))
//!         .unwrap_or(true);
//!
//!     if has_platform && has_arch {
//!         files.push(entry);
//!     }
//! }
//!
//! files
//! # }
//! ```
//!
//! ## Size Calculation
//!
//! ```rust,no_run
//! # use cascette_formats::install::InstallManifest;
//! # fn calculate_install_size(manifest: &InstallManifest, tag_filter: &[&str]) -> u64 {
//! let tag_indices: Vec<usize> = tag_filter.iter()
//!     .filter_map(|name| {
//!         manifest.tags.iter().position(|t| &t.name == name)
//!     })
//!     .collect();
//!
//! let mut total_size = 0u64;
//!
//! for (file_index, entry) in manifest.entries.iter().enumerate() {
//!     let should_install = tag_indices.iter().any(|&tag_index| {
//!         manifest.tags[tag_index].has_file(file_index)
//!     });
//!
//!     if should_install {
//!         total_size += entry.file_size as u64;
//!     }
//! }
//!
//! total_size
//! # }
//! ```

pub mod builder;
pub mod entry;
pub mod error;
pub mod header;
pub mod manifest;
pub mod tag;

// Re-export main types
pub use builder::InstallManifestBuilder;
pub use entry::InstallFileEntry;
pub use error::{InstallError, Result};
pub use header::InstallHeader;
pub use manifest::InstallManifest;
pub use tag::{InstallTag, TagType};

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::ContentKey;

    #[test]
    fn test_module_exports() {
        // Test that all public types are accessible
        let _header = InstallHeader::new(0, 0);
        let _builder = InstallManifestBuilder::new();
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use cascette_crypto::ContentKey;
        use proptest::prelude::*;

        /// Generate arbitrary content keys
        fn content_key() -> impl Strategy<Value = ContentKey> {
            prop::array::uniform16(0u8..255).prop_map(ContentKey::from_bytes)
        }

        /// Generate valid file paths for installation
        fn file_path() -> impl Strategy<Value = String> {
            prop_oneof![
                // Predefined valid paths to avoid Unicode issues
                Just("data/file1.blp".to_string()),
                Just("data/file2.m2".to_string()),
                Just("data/maps/map1.adt".to_string()),
                Just("data/models/model1.wmo".to_string()),
                Just("data/db/game.dbc".to_string()),
                Just("app/game.exe".to_string()),
                Just("app/library.dll".to_string()),
                Just("data/content.pak".to_string()),
                Just("Interface/AddOns/addon.lua".to_string()),
                Just("Interface/AddOns/addon.toc".to_string()),
                Just("data/archives/data.mpq".to_string()),
            ]
        }

        /// Generate file sizes (up to 4GB for install manifests)
        fn file_size() -> impl Strategy<Value = u32> {
            0u32..=0xFFFF_FFFF
        }

        /// Generate arbitrary tag types
        fn tag_type() -> impl Strategy<Value = TagType> {
            prop_oneof![
                Just(TagType::Platform),
                Just(TagType::Architecture),
                Just(TagType::Locale),
                Just(TagType::Category),
                Just(TagType::Component),
                Just(TagType::Version),
                Just(TagType::Feature),
                Just(TagType::Region),
                Just(TagType::Device)
            ]
        }

        /// Generate valid tag names
        fn tag_name() -> impl Strategy<Value = String> {
            prop_oneof![
                // Platform names
                Just("Windows".to_string()),
                Just("OSX".to_string()),
                Just("Linux".to_string()),
                // Architecture names
                Just("x86".to_string()),
                Just("x86_64".to_string()),
                Just("ARM64".to_string()),
                // Locale names
                Just("enUS".to_string()),
                Just("deDE".to_string()),
                Just("frFR".to_string()),
                Just("esES".to_string()),
                // Generic names
                "[A-Za-z0-9_.-]{1,50}"
            ]
        }

        /// Generate install file entries
        fn install_entry() -> impl Strategy<Value = (String, ContentKey, u32)> {
            (file_path(), content_key(), file_size())
        }

        /// Generate install tags
        fn install_tag() -> impl Strategy<Value = (String, TagType)> {
            (tag_name(), tag_type())
        }

        proptest! {
            /// Test that install manifests round-trip correctly
            #[test]
            fn install_manifest_round_trip(
                entries in prop::collection::vec(install_entry(), 1..20),
                raw_tags in prop::collection::vec(install_tag(), 0..10)
            ) {
                let mut builder = InstallManifestBuilder::new();

                // Make tag names unique to avoid conflicts
                let mut unique_tags = Vec::new();
                let mut seen_names = std::collections::HashSet::new();
                for (name, tag_type) in raw_tags {
                    let unique_name = if seen_names.contains(&name) {
                        format!("{}-{}", name, unique_tags.len())
                    } else {
                        name
                    };
                    seen_names.insert(unique_name.clone());
                    unique_tags.push((unique_name, tag_type));
                }
                let tags = unique_tags;

                // Add tags
                for (name, tag_type) in &tags {
                    builder = builder.add_tag(name.clone(), *tag_type);
                }

                // Add files
                for (path, content_key, size) in &entries {
                    builder = builder.add_file(path.clone(), *content_key, *size);
                }

                // Associate some files with tags (ensure indices are valid)
                for file_idx in 0..entries.len().min(3) {
                    for tag in tags.iter().take(3) {
                        let tag_name = &tag.0;
                        builder = builder.associate_file_with_tag(file_idx, tag_name)?;
                    }
                }

                let manifest = builder.build()?;
                let data = manifest.build()?;
                let parsed = InstallManifest::parse(&data)?;

                // Verify structure (builder produces V1 by default)
                prop_assert_eq!(parsed.header.version, 1);
                prop_assert_eq!(parsed.entries.len(), entries.len());
                prop_assert_eq!(parsed.tags.len(), tags.len());

                // Verify entries
                for (i, (original_path, original_key, original_size)) in entries.iter().enumerate() {
                    let entry = &parsed.entries[i];
                    prop_assert_eq!(&entry.path, original_path);
                    prop_assert_eq!(entry.content_key.as_bytes(), original_key.as_bytes());
                    prop_assert_eq!(entry.file_size, *original_size);
                }

                // Verify tags
                for (original_name, original_type) in &tags {
                    let found_tag = parsed.tags.iter().find(|t| &t.name == original_name);
                    prop_assert!(found_tag.is_some());
                    let found_tag = found_tag.expect("Tag should exist");
                    prop_assert_eq!(found_tag.tag_type, *original_type);
                }
            }

            /// Test bit mask operations work correctly
            #[test]
            fn bit_mask_operations(
                file_indices in prop::collection::hash_set(0usize..100, 0..20)
            ) {
                let max_files = file_indices.iter().copied().max().unwrap_or(0) + 1;
                let mut tag = InstallTag {
                    name: "TestTag".to_string(),
                    tag_type: TagType::Platform,
                    bit_mask: vec![0u8; max_files.div_ceil(8)], // Enough bytes for all files
                };

                // Add all files to the tag
                for &file_index in &file_indices {
                    tag.add_file(file_index);
                }

                // Verify all files are associated
                for &file_index in &file_indices {
                    prop_assert!(tag.has_file(file_index));
                }

                // Verify non-associated files are not present
                for i in 0..max_files {
                    if !file_indices.contains(&i) {
                        prop_assert!(!tag.has_file(i));
                    }
                }

                // Verify file count
                prop_assert_eq!(tag.file_count(), file_indices.len());
            }

            /// Test tag type conversion is bijective
            #[test]
            fn tag_type_conversion_bijective(
                tag_type in tag_type()
            ) {
                let value = tag_type as u16;
                let converted_back = TagType::from_u16(value);
                prop_assert_eq!(converted_back, Some(tag_type));
            }

            /// Test that invalid tag type values return None
            #[test]
            fn invalid_tag_type_values(
                invalid_value in any::<u16>().prop_filter(
                    "Not a valid tag type",
                    |&v| !matches!(v, 0x0001 | 0x0002 | 0x0003 | 0x0004 | 0x0005 | 0x0010 | 0x0020 | 0x0040 | 0x0080 | 0x0100 | 0x0200 | 0x0400 | 0x0800 | 0x1000 | 0x2000 | 0x4000 | 0x8000)
                )
            ) {
                let result = TagType::from_u16(invalid_value);
                prop_assert_eq!(result, None);
            }

            /// Test file size calculations
            #[test]
            fn file_size_calculations(
                entries in prop::collection::vec(install_entry(), 1..50)
            ) {
                let mut builder = InstallManifestBuilder::new();
                let mut expected_total = 0u64;

                // Add files and calculate expected total
                for (path, content_key, size) in &entries {
                    builder = builder.add_file(path.clone(), *content_key, *size);
                    expected_total += *size as u64;
                }

                let manifest = builder.build()?;

                // Calculate actual total from manifest
                let actual_total: u64 = manifest.entries.iter()
                    .map(|e| e.file_size as u64)
                    .sum();

                prop_assert_eq!(actual_total, expected_total);
            }

            /// Test that invalid magic bytes are rejected
            #[test]
            fn invalid_magic_rejected(
                magic in prop::array::uniform2(0u8..255).prop_filter("Not IN magic", |m| m != b"IN")
            ) {
                let mut data = vec![0u8; 20];
                data[0..2].copy_from_slice(&magic);
                data[2] = 1; // Valid version

                let result = InstallManifest::parse(&data);
                prop_assert!(result.is_err());
            }

            /// Test tag associations with multiple files and tags
            #[test]
            fn complex_tag_associations(
                files in prop::collection::vec(install_entry(), 5..15),
                raw_tags in prop::collection::vec(install_tag(), 2..8),
                associations in prop::collection::vec((0usize..15, 0usize..8), 10..30)
            ) {
                let mut builder = InstallManifestBuilder::new();

                // Make tag names unique to avoid conflicts
                let mut unique_tags = Vec::new();
                let mut seen_names = std::collections::HashSet::new();
                for (name, tag_type) in raw_tags {
                    let unique_name = if seen_names.contains(&name) {
                        format!("{}-{}", name, unique_tags.len())
                    } else {
                        name
                    };
                    seen_names.insert(unique_name.clone());
                    unique_tags.push((unique_name, tag_type));
                }
                let tags = unique_tags;

                // Add tags
                for (name, tag_type) in &tags {
                    builder = builder.add_tag(name.clone(), *tag_type);
                }

                // Add files
                for (path, content_key, size) in &files {
                    builder = builder.add_file(path.clone(), *content_key, *size);
                }

                // Add associations (with bounds checking)
                for &(file_idx, tag_idx) in &associations {
                    if file_idx < files.len() && tag_idx < tags.len() {
                        let tag_name = &tags[tag_idx].0;
                        builder = builder.associate_file_with_tag(file_idx, tag_name)?;
                    }
                }

                let manifest = builder.build()?;
                let data = manifest.build()?;
                let parsed = InstallManifest::parse(&data)?;

                // Verify associations are preserved
                for &(file_idx, tag_idx) in &associations {
                    if file_idx < files.len() && tag_idx < tags.len() {
                        let tag_name = &tags[tag_idx].0;
                        let found_tag = parsed.tags.iter().find(|t| &t.name == tag_name);
                        prop_assert!(found_tag.is_some());
                        prop_assert!(found_tag.expect("Tag should exist").has_file(file_idx));
                    }
                }
            }

            /// Test header validation
            #[test]
            fn header_validation(
                tag_count in 0u16..=1000,
                entry_count in 0u32..=1000
            ) {
                let header = InstallHeader::new(tag_count, entry_count);

                prop_assert_eq!(header.magic, *b"IN");
                prop_assert_eq!(header.version, 1);
                prop_assert_eq!(header.ckey_length, 16); // MD5 size
                prop_assert_eq!(header.tag_count, tag_count);
                prop_assert_eq!(header.entry_count, entry_count);
            }

            /// Test that different manifests produce different serializations
            #[test]
            fn different_manifests_different_data(
                entries1 in prop::collection::vec(install_entry(), 1..5),
                entries2 in prop::collection::vec(install_entry(), 1..5)
            ) {
                prop_assume!(entries1 != entries2);

                let mut builder1 = InstallManifestBuilder::new();
                let mut builder2 = InstallManifestBuilder::new();

                for (path, content_key, size) in &entries1 {
                    builder1 = builder1.add_file(path.clone(), *content_key, *size);
                }
                for (path, content_key, size) in &entries2 {
                    builder2 = builder2.add_file(path.clone(), *content_key, *size);
                }

                let manifest1 = builder1.build()?;
                let manifest2 = builder2.build()?;

                let data1 = manifest1.build()?;
                let data2 = manifest2.build()?;

                prop_assert_ne!(data1, data2);
            }

            /// Test manifest validation
            #[test]
            fn manifest_validation(
                entries in prop::collection::vec(install_entry(), 0..20),
                tags in prop::collection::vec(install_tag(), 0..10)
            ) {
                let mut builder = InstallManifestBuilder::new();

                // Add tags
                for (name, tag_type) in &tags {
                    builder = builder.add_tag(name.clone(), *tag_type);
                }

                // Add files
                for (path, content_key, size) in &entries {
                    builder = builder.add_file(path.clone(), *content_key, *size);
                }

                let manifest = builder.build()?;

                // Validation should pass for well-formed manifests
                prop_assert!(manifest.validate().is_ok());
            }

            /// Test bit mask size calculations
            #[test]
            fn bit_mask_size_calculations(
                max_files in 1usize..=1000
            ) {
                let expected_bytes = max_files.div_ceil(8);
                let tag = InstallTag {
                    name: "Test".to_string(),
                    tag_type: TagType::Platform,
                    bit_mask: vec![0u8; expected_bytes],
                };

                // Should be able to handle files up to max_files-1
                prop_assert_eq!(tag.bit_mask.len(), expected_bytes);
                prop_assert!(tag.bit_mask.len() * 8 >= max_files);
            }

            /// Test large file collections
            #[test]
            fn large_file_collections(
                entries in prop::collection::vec(install_entry(), 100..200)
            ) {
                let mut builder = InstallManifestBuilder::new();

                // Add a tag to test bit mask expansion
                builder = builder.add_tag("LargeTest".to_string(), TagType::Platform);

                // Add files
                for (path, content_key, size) in &entries {
                    builder = builder.add_file(path.clone(), *content_key, *size);
                }

                // Associate every 10th file with the tag
                for i in (0..entries.len()).step_by(10) {
                    builder = builder.associate_file_with_tag(i, "LargeTest")?;
                }

                let manifest = builder.build()?;
                let data = manifest.build()?;
                let parsed = InstallManifest::parse(&data)?;

                // Verify large collection handling
                prop_assert_eq!(parsed.entries.len(), entries.len());
                let large_tag = parsed.tags.iter().find(|t| t.name == "LargeTest").expect("LargeTest tag should exist");

                // Check associations
                for i in (0..entries.len()).step_by(10) {
                    prop_assert!(large_tag.has_file(i));
                }
            }
        }
    }

    #[test]
    fn test_complete_workflow() {
        // Test complete parse -> modify -> build workflow
        let manifest = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("x86_64".to_string(), TagType::Architecture)
            .add_file(
                "Test\\File\\Path.blp".to_string(),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                1024,
            )
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .associate_file_with_tag(0, "x86_64")
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        // Build to bytes
        let data = manifest.build().expect("Operation should succeed");

        // Parse it back
        let parsed = InstallManifest::parse(&data).expect("Operation should succeed");

        // Validate structure
        assert_eq!(parsed.header.version, 1);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.tags.len(), 2);
        assert!(parsed.validate().is_ok());

        // Test tag associations
        let windows_tag = parsed
            .tags
            .iter()
            .find(|t| t.name == "Windows")
            .expect("Operation should succeed");
        assert!(windows_tag.has_file(0));

        let arch_tag = parsed
            .tags
            .iter()
            .find(|t| t.name == "x86_64")
            .expect("Operation should succeed");
        assert!(arch_tag.has_file(0));
    }

    #[test]
    fn test_tag_types() {
        // Test all tag types can be created and converted
        let tag_types = [
            (TagType::Platform, 0x0001),
            (TagType::Architecture, 0x0002),
            (TagType::Locale, 0x0003),
            (TagType::Category, 0x0004),
            (TagType::Unknown, 0x0005),
            (TagType::Component, 0x0010),
            (TagType::Version, 0x0020),
            (TagType::Optimization, 0x0040),
            (TagType::Region, 0x0080),
            (TagType::Device, 0x0100),
            (TagType::Mode, 0x0200),
            (TagType::Branch, 0x0400),
            (TagType::Content, 0x0800),
            (TagType::Feature, 0x1000),
            (TagType::Expansion, 0x2000),
            (TagType::Alternate, 0x4000),
            (TagType::Option, 0x8000),
        ];

        for (tag_type, expected_value) in tag_types {
            assert_eq!(tag_type as u16, expected_value);
            assert_eq!(TagType::from_u16(expected_value), Some(tag_type));
        }
    }

    #[test]
    fn test_bit_mask_operations() {
        let mut tag = InstallTag {
            name: "Windows".to_string(),
            tag_type: TagType::Platform,
            bit_mask: vec![0u8; 2], // 16 files max
        };

        // Add files 0, 1, and 9
        tag.add_file(0);
        tag.add_file(1);
        tag.add_file(9);

        // Check bit patterns (big-endian/MSB-first bit ordering)
        assert_eq!(tag.bit_mask[0], 0b1100_0000); // Files 0 and 1 (bits 7, 6)
        assert_eq!(tag.bit_mask[1], 0b0100_0000); // File 9 (byte 1, bit 6)

        // Verify file associations
        assert!(tag.has_file(0));
        assert!(tag.has_file(1));
        assert!(!tag.has_file(2));
        assert!(tag.has_file(9));
        assert!(!tag.has_file(10));

        assert_eq!(tag.file_count(), 3);
    }

    #[test]
    fn test_round_trip() {
        let original = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file(
                "data/file1.bin".to_string(),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                1024,
            )
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        // Serialize
        let data = original.build().expect("Operation should succeed");

        // Deserialize
        let parsed = InstallManifest::parse(&data).expect("Operation should succeed");

        // Verify equality
        assert_eq!(parsed.header.tag_count, original.header.tag_count);
        assert_eq!(parsed.header.entry_count, original.header.entry_count);
        assert_eq!(parsed.tags.len(), original.tags.len());
        assert_eq!(parsed.entries.len(), original.entries.len());

        // Verify tag associations match
        let windows_tag_orig = original
            .tags
            .iter()
            .find(|t| t.name == "Windows")
            .expect("Operation should succeed");
        let windows_tag_parsed = parsed
            .tags
            .iter()
            .find(|t| t.name == "Windows")
            .expect("Operation should succeed");
        assert_eq!(windows_tag_orig.bit_mask, windows_tag_parsed.bit_mask);
    }
}
