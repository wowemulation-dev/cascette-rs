//! Root file format support for NGDP/CASC systems
//!
//! This module provides parsing and building support for the World of Warcraft
//! root file format across versions V1-V4. The root file maps `FileDataID`
//! values and path name hashes to content keys.
//!
//! **Note:** This root format is WoW-specific. Other CASC-based games (e.g.
//! Diablo IV, Overwatch) use different root file formats with distinct
//! structures and semantics.
//!
//! # Root File Versions
//!
//! - **V1** (WoW 6.0-7.2): No header, interleaved format
//! - **V2** (WoW 7.2.5-8.1): MFST/TSFM header, separated arrays
//! - **V3** (WoW 8.2-9.1): Extended header with size/version
//! - **V4** (WoW 9.1+): 40-bit content flags
//!
//! # Key Features
//!
//! - **Automatic version detection** - Identifies format version from file structure
//! - **Mixed endianness support** - Headers use big-endian, blocks use little-endian
//! - **Efficient lookups** - HashMap-based resolution by FileDataID or path name
//! - **Delta compression** - FileDataID sequences use delta encoding for size reduction
//! - **Name hash calculation** - Jenkins96 hashing with WoW-specific word swapping
//! - **Round-trip compatibility** - Parse and rebuild produce identical output
//!
//! # Basic Usage
//!
//! ## Parsing Root Files
//!
//! ```rust,no_run
//! use cascette_formats::root::{RootFile, LocaleFlags, ContentFlags};
//! use cascette_crypto::md5::FileDataId;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let data = std::fs::read("root_file.bin")?;
//! let root = RootFile::parse(&data)?;
//!
//! println!("Root file version: {}", root.version);
//! println!("Total files: {}", root.total_files());
//! println!("Named files: {}", root.named_files());
//!
//! // Resolve file by FileDataID
//! let content_key = root.resolve_by_id(
//!     FileDataId::new(123_456),
//!     LocaleFlags::new(LocaleFlags::ENUS),
//!     ContentFlags::new(ContentFlags::INSTALL),
//! );
//!
//! // Resolve file by path
//! let content_key = root.resolve_by_path(
//!     "Interface\\Icons\\INV_Misc_QuestionMark.blp",
//!     LocaleFlags::new(LocaleFlags::ENUS),
//!     ContentFlags::new(ContentFlags::INSTALL),
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ## Building Root Files
//!
//! ```rust,no_run
//! use cascette_formats::root::{RootBuilder, RootVersion, LocaleFlags, ContentFlags};
//! use cascette_crypto::md5::{FileDataId, ContentKey};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut builder = RootBuilder::new(RootVersion::V2);
//!
//! // Add files to the root
//! builder.add_file(
//!     FileDataId::new(100),
//!     ContentKey::from_hex("0123456789abcdef0123456789abcdef")?,
//!     Some("Interface\\Icons\\INV_Misc_QuestionMark.blp"),
//!     LocaleFlags::new(LocaleFlags::ENUS),
//!     ContentFlags::new(ContentFlags::INSTALL),
//! );
//!
//! // Build the root file
//! let data = builder.build()?;
//! std::fs::write("new_root.bin", data)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Advanced Features
//!
//! ## Multi-Locale Support
//!
//! ```rust,no_run
//! # use cascette_formats::root::{RootBuilder, RootVersion, LocaleFlags, ContentFlags};
//! # use cascette_crypto::md5::{FileDataId, ContentKey};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut builder = RootBuilder::new(RootVersion::V2);
//!
//! // Add file supporting multiple locales
//! builder.add_file(
//!     FileDataId::new(200),
//!     ContentKey::from_hex("fedcba9876543210fedcba9876543210")?,
//!     Some("World\\Maps\\TestMap\\TestMap.wdt"),
//!     LocaleFlags::new(LocaleFlags::ENUS | LocaleFlags::DEDE | LocaleFlags::FRFR),
//!     ContentFlags::new(ContentFlags::INSTALL),
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ## Custom Name Hashes
//!
//! ```rust,no_run
//! # use cascette_formats::root::{RootBuilder, RootVersion, LocaleFlags, ContentFlags};
//! # use cascette_crypto::md5::{FileDataId, ContentKey};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut builder = RootBuilder::new(RootVersion::V2);
//!
//! // Add file with explicit name hash
//! builder.add_file_with_hash(
//!     FileDataId::new(300),
//!     ContentKey::from_hex("abcdefabcdefabcdefabcdefabcdefab")?,
//!     Some(0x1234_567890abcdef), // Custom hash
//!     LocaleFlags::new(LocaleFlags::ALL),
//!     ContentFlags::new(ContentFlags::INSTALL),
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ## Format Conversion
//!
//! ```rust,no_run
//! # use cascette_formats::root::{RootFile, RootBuilder, RootVersion};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Load existing root file
//! let data = std::fs::read("old_root.bin")?;
//! let root = RootFile::parse(&data)?;
//!
//! // Convert to different version
//! let mut builder = RootBuilder::from_root_file(&root);
//! builder.set_version(RootVersion::V4);
//!
//! let new_data = builder.build()?;
//! std::fs::write("converted_root.bin", new_data)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Implementation Details
//!
//! ## Binary Structure
//!
//! Root files use a complex binary format with mixed endianness:
//!
//! - **Headers** are big-endian (network byte order)
//! - **Block data** is little-endian (x86 native order)
//! - **FileDataIDs** are delta-encoded as signed 32-bit integers
//! - **Content keys** are raw 128-bit MD5 hashes
//! - **Name hashes** are 64-bit values with WoW-specific bit swapping
//!
//! ## Lookup Algorithm
//!
//! File resolution uses a two-stage lookup:
//!
//! 1. **Primary lookup** by FileDataID or name hash
//! 2. **Secondary filtering** by locale and content flags
//!
//! This allows files to exist in multiple variants while maintaining efficient O(1) access.
//!
//! ## Performance Considerations
//!
//! - Lookup tables are built once during parsing for O(1) resolution
//! - Delta encoding reduces file size by ~30% for sorted FileDataID sequences
//! - Block organization by flags minimizes memory usage for filtered operations
//! - Round-trip parsing maintains exact byte compatibility with original files
//!
//! # Error Handling
//!
//! All operations return `Result` types with detailed error information.
//! See the error module for complete error documentation.

pub mod block;
pub mod builder;
pub mod entry;
pub mod error;
pub mod file;
pub mod flags;
pub mod header;
pub mod version;

// Re-export main types
pub use block::{RootBlock, RootBlockHeader};
pub use builder::RootBuilder;
pub use entry::{
    RootEntry, RootLookupTables, RootRecord, calculate_name_hash, decode_file_data_ids,
    encode_file_data_ids,
};
pub use error::{Result, RootError};
pub use file::RootFile;
pub use flags::{ContentFlags, LocaleFlags};
pub use header::{RootHeader, RootHeaderInfo, RootMagic};
pub use version::RootVersion;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use cascette_crypto::md5::{ContentKey, FileDataId};

    #[test]
    fn test_module_exports() {
        // Test that all public types are accessible
        // Test that version types are accessible
        let version = RootVersion::V2;
        assert_eq!(version, RootVersion::V2);
        let _flags = ContentFlags::new(ContentFlags::INSTALL);
        let _locale = LocaleFlags::new(LocaleFlags::ENUS);
        let _builder = RootBuilder::new(RootVersion::V2);
    }

    #[test]
    fn test_complete_workflow() {
        // Test complete parse -> modify -> build workflow
        let mut builder = RootBuilder::new(RootVersion::V2);

        // Add test file
        builder.add_file(
            FileDataId::new(12_345),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some("Test\\File\\Path.blp"),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        // Build root file
        let data = builder.build().expect("Operation should succeed");

        // Parse it back
        let root = RootFile::parse(&data).expect("Operation should succeed");

        // Validate structure
        assert_eq!(root.version, RootVersion::V2);
        assert_eq!(root.total_files(), 1);
        assert_eq!(root.named_files(), 1);
        assert!(root.validate().is_ok());

        // Test resolution
        let resolved = root.resolve_by_id(
            FileDataId::new(12_345),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved.is_some());

        let resolved_by_path = root.resolve_by_path(
            "Test\\File\\Path.blp",
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        assert!(resolved_by_path.is_some());
        assert_eq!(resolved, resolved_by_path);
    }

    #[test]
    fn test_version_coverage() {
        // Test all versions can be created and have expected properties
        for version in [
            RootVersion::V1,
            RootVersion::V2,
            RootVersion::V3,
            RootVersion::V4,
        ] {
            let mut builder = RootBuilder::new(version);

            builder.add_file(
                FileDataId::new(100),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                Some("test.txt"),
                LocaleFlags::new(LocaleFlags::ENUS),
                ContentFlags::new(ContentFlags::INSTALL),
            );

            let data = builder.build().expect("Operation should succeed");
            let parsed = RootFile::parse(&data).expect("Operation should succeed");

            assert_eq!(parsed.version, version);
            assert_eq!(parsed.header.is_some(), version.has_header());
            assert!(parsed.total_files() >= 1);
        }
    }

    #[test]
    fn test_flag_combinations() {
        // Test various flag combinations work correctly
        // Note: V2 version detection requires total_files >= 100 to avoid ambiguity
        // with V3+ headers, so we add extra files to ensure correct detection
        let mut builder = RootBuilder::new(RootVersion::V2);

        // Test different content flag combinations
        let content_flags = [
            ContentFlags::INSTALL,
            ContentFlags::INSTALL | ContentFlags::BUNDLE,
            ContentFlags::LOW_VIOLENCE,
            ContentFlags::NO_NAME_HASH,
        ];

        // Test different locale combinations
        let locale_flags = [
            LocaleFlags::ENUS,
            LocaleFlags::ENUS | LocaleFlags::DEDE,
            LocaleFlags::ALL,
        ];

        let mut fdid = 1000u32;

        // Add 100 base files to ensure total_files >= 100 for correct V2 detection
        for i in 0..100 {
            builder.add_file(
                FileDataId::new(fdid),
                ContentKey::from_hex(&format!("{:032x}", i)).expect("Operation should succeed"),
                Some(&format!("base/file{}.txt", i)),
                LocaleFlags::new(LocaleFlags::ENUS),
                ContentFlags::new(ContentFlags::INSTALL),
            );
            fdid += 1;
        }

        // Add files with various flag combinations
        for &content in &content_flags {
            for &locale in &locale_flags {
                builder.add_file(
                    FileDataId::new(fdid),
                    ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                        .expect("Operation should succeed"),
                    if (content & ContentFlags::NO_NAME_HASH) != 0 {
                        None
                    } else {
                        Some("test.txt")
                    },
                    LocaleFlags::new(locale),
                    ContentFlags::new(content),
                );
                fdid += 1;
            }
        }

        let data = builder.build().expect("Operation should succeed");
        let root = RootFile::parse(&data).expect("Operation should succeed");

        assert!(root.validate().is_ok());
        let expected_count = 100
            + u32::try_from(content_flags.len()).expect("Operation should succeed")
                * u32::try_from(locale_flags.len()).expect("Operation should succeed");
        assert!(root.total_files() >= expected_count);
    }

    #[test]
    fn test_name_hash_functionality() {
        // Test name hash calculation works correctly
        let path1 = "Interface\\Icons\\INV_Misc_QuestionMark.blp";
        let path2 = "interface/icons/inv_misc_questionmark.blp";

        let hash1 = calculate_name_hash(path1);
        let hash2 = calculate_name_hash(path2);

        // Should normalize to same hash
        assert_eq!(hash1, hash2);
        assert_ne!(hash1, 0);

        // Test with builder
        let mut builder = RootBuilder::new(RootVersion::V2);
        builder.add_file(
            FileDataId::new(100),
            ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                .expect("Operation should succeed"),
            Some(path1),
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        let data = builder.build().expect("Operation should succeed");
        let root = RootFile::parse(&data).expect("Operation should succeed");

        // Should resolve by both path variations
        let resolved1 = root.resolve_by_path(
            path1,
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );
        let resolved2 = root.resolve_by_path(
            path2,
            LocaleFlags::new(LocaleFlags::ENUS),
            ContentFlags::new(ContentFlags::INSTALL),
        );

        assert!(resolved1.is_some());
        assert!(resolved2.is_some());
        assert_eq!(resolved1, resolved2);
    }

    #[test]
    fn test_delta_encoding_roundtrip() {
        // Test FileDataID delta encoding/decoding
        let original_ids = vec![
            FileDataId::new(100),
            FileDataId::new(101),
            FileDataId::new(105),
            FileDataId::new(110),
            FileDataId::new(1000),
        ];

        let deltas = encode_file_data_ids(&original_ids);
        let decoded_ids = decode_file_data_ids(&deltas);

        assert_eq!(original_ids, decoded_ids);
    }
}
