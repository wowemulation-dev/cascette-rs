//! File format parsers and builders for NGDP/CASC system
//!
#![allow(clippy::cast_possible_truncation)] // Intentional for binary format parsing
#![allow(clippy::cast_possible_wrap)] // Intentional for binary operations
#![allow(clippy::cast_lossless)] // Sometimes clearer than From
#![allow(clippy::uninlined_format_args)] // Backwards compatibility
#![allow(clippy::doc_markdown)] // Many CASC-specific terms don't need backticks
#![allow(clippy::module_name_repetitions)] // Clear naming is preferred
#![allow(clippy::similar_names)] // Domain-specific naming patterns
#![allow(clippy::float_cmp)] // Binary format requirements
#![allow(clippy::no_effect_underscore_binding)] // Test placeholders
#![allow(clippy::used_underscore_binding)] // Test variables
#![allow(clippy::needless_pass_by_value)] // Configuration types
#![allow(clippy::redundant_clone)] // Binary format handling
#![allow(clippy::unused_self)] // Future implementation hooks
#![allow(clippy::map_unwrap_or)] // Binary format patterns
#![allow(clippy::redundant_closure)] // Test setup
#![allow(clippy::cast_precision_loss)] // Performance metrics
#![allow(clippy::derive_partial_eq_without_eq)] // Binary format structs
#![allow(clippy::redundant_closure_for_method_calls)] // Iterator chains
#![allow(clippy::unnecessary_wraps)] // Future error handling
#![allow(clippy::unused_async)] // Future implementation hooks
#![allow(clippy::needless_pass_by_ref_mut)] // Future mutability
#![allow(clippy::return_self_not_must_use)] // Builder patterns
#![allow(clippy::use_self)] // Type clarity
#![allow(clippy::map_entry)] // Error handling patterns
#![allow(clippy::clone_on_copy)] // Binary format handling
#![allow(clippy::future_not_send)] // Threading requirements
//! This crate provides symmetric (parser and builder) implementations for all
//! file formats used in Blizzard's NGDP (Next Generation Distribution Pipeline)
//! and CASC (Content Addressable Storage Container) systems.
//!
//! # Supported Formats
//!
//! - **BPSV**: Blizzard Pipe-Separated Values for version and configuration data
//! - **BLTE**: Block Table Encoded format for compressed and encrypted content
//! - **Root**: Root file format mapping paths/FileDataIDs to content keys
//! - **Encoding**: Encoding file format for content key to encoding key mappings
//! - **Install**: Install manifest format for file tagging and selective installation
//! - **Download**: Download manifest format for priority-based streaming installation
//! - **Config**: Build and CDN configuration file formats
//! - **`ESpec`**: Encoding specification format
//! - **Archive**: Archive index and data file operations for CDN content storage
//!
//! # Design Principles
//!
//! Every format implementation follows these principles:
//! - **Symmetric Operations**: Both parsing and building supported
//! - **Zero-Copy Parsing**: Minimize allocations when possible
//! - **Type Safety**: Use Rust's type system to enforce invariants
//! - **Round-Trip Guarantee**: parse(build(data)) == data

#![warn(missing_docs)]

/// Archive system for NGDP/CASC content storage and retrieval
///
/// This module provides complete support for CDN archive files (.data) and their
/// corresponding index files (.index). Archive files are the primary storage
/// mechanism for game content in NGDP/CASC systems.
///
/// Key features:
/// - **Archive Index Parsing**: Binary format parsing with chunked structure
/// - **Variable-Length Key Support**: Full encoding key support based on footer specification
/// - **Binary Search Operations**: Fast content location with O(log n) lookups
/// - **HTTP Range Requests**: Efficient partial content downloads
/// - **BLTE Integration**: Seamless decompression and decryption support
/// - **CDN Client Operations**: Complete CDN interaction support
/// - **Memory Efficient**: Chunked loading for large indices
///
/// See the [`archive`] module for detailed usage examples and integration patterns.
pub mod archive;
pub mod blte;
pub mod bpsv;
/// Configuration file formats (Build Config and CDN Config)
pub mod config;
/// Download manifest format for priority-based streaming installation
///
/// This module provides complete parsing and building support for CASC download manifests
/// used to manage content streaming and prioritization during game installation and updates.
/// Unlike Install manifests which track installed files, Download manifests enable
/// priority-based streaming installation where essential files are downloaded first.
///
/// Key features:
/// - **Priority-Based Streaming**: Downloads critical files first to minimize time-to-playability
/// - **Three Format Versions**: Supports v1, v2, v3 with incremental features
/// - **40-Bit File Sizes**: Supports files larger than 4GB using 5-byte size fields
/// - **EncodingKey Usage**: Uses encoding keys instead of content keys
/// - **Entries-First Layout**: Unlike Install manifest, entries come before tags
///
/// See the [`download`] module for detailed usage examples and priority system documentation.
pub mod download;
/// Encoding file format for content key to encoding key mappings
pub mod encoding;
pub mod espec;
/// Install manifest format for file tagging and selective installation
///
/// This module provides complete parsing and building support for CASC install manifests
/// that define which files should be installed and organize them using tags for
/// platform-specific and selective installation.
///
/// See the [`install`] module for detailed usage examples and tag system documentation.
pub mod install;
/// Patch Archive (PA) format for differential patch manifests
///
/// This module provides complete parsing and building support for Patch Archive files
/// that describe differential patches between different versions of NGDP content.
/// Patch Archives enable incremental updates by providing mappings between old content
/// keys, new content keys, and patch data.
///
/// Key features:
/// - **Differential Updates**: Patch mappings for incremental content updates
/// - **Variable-Length Entries**: Support for complex compression specifications
/// - **Mixed Endianness**: Big-endian header with little-endian entry data
/// - **Streaming Support**: Process large patch archives without full memory load
/// - **Content Addressing**: MD5-based content key system
/// - **Patch Chains**: Support for multi-step patch sequences
///
/// See the [`patch_archive`] module for detailed usage examples and patch application patterns.
pub mod patch_archive;
/// Root file format for mapping paths/FileDataIDs to content keys
///
/// This module provides complete parsing and building support for CASC root files
/// across all four format versions (V1-V4) used throughout World of Warcraft's history.
///
/// See the [`root`] module for detailed usage examples and error handling documentation.
pub mod root;
/// TVFS (TACT Virtual File System) format for unified content management
///
/// This module provides complete parsing and building support for TVFS manifests
/// introduced in WoW 8.2 (CASC v3). TVFS provides a hierarchical virtual file system
/// that enables content deduplication and multi-product support.
///
/// Key features:
/// - **Hierarchical Paths**: Prefix tree structure for efficient path storage
/// - **Content Deduplication**: Same content referenced by multiple paths
/// - **Multi-Product Support**: Manages files across different products
/// - **BLTE Integration**: Seamless decompression of compressed manifests
/// - **Streaming Support**: Lazy loading for large manifests
///
/// See the [`tvfs`] module for detailed usage examples and integration patterns.
pub mod tvfs;
/// ZBSDIFF1 (Zlib-compressed Binary Differential) format for efficient binary patches
///
/// This module provides complete parsing and building support for ZBSDIFF1 binary
/// differential patches used by NGDP/TACT for efficient file updates. Based on the
/// bsdiff algorithm by Colin Percival, with zlib compression applied to all data blocks.
///
/// Key features:
/// - **Streaming Application**: Apply patches without loading entire files into memory
/// - **Memory-Efficient**: Chunked processing for large binary diffs
/// - **Zlib Compression**: All data blocks use zlib compression for minimal size
/// - **Big-Endian Header**: 32-byte header with format signature and block sizes
/// - **Round-Trip Validation**: Complete parser-builder pattern implementation
/// - **BLTE Integration**: Seamless integration with compressed patch data
///
/// See the [`zbsdiff`] module for detailed usage examples and patch creation workflows.
pub mod zbsdiff;

// Test utilities module
#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
pub(crate) mod test_utils;

// All format modules implemented

/// Common format trait that all formats should implement
pub trait CascFormat: Sized {
    /// Parse from bytes
    fn parse(data: &[u8]) -> Result<Self, Box<dyn std::error::Error>>;

    /// Build to bytes
    fn build(&self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;

    /// Verify round-trip correctness
    fn verify_round_trip(data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let parsed = Self::parse(data)?;
        let rebuilt = parsed.build()?;
        if data != rebuilt.as_slice() {
            return Err("Round-trip verification failed".into());
        }
        Ok(())
    }
}
