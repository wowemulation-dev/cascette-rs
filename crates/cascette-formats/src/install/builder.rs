//! Builder pattern for creating install manifests

use crate::install::{
    entry::InstallFileEntry,
    error::{InstallError, Result},
    header::InstallHeader,
    manifest::InstallManifest,
    tag::{InstallTag, TagType},
};
use cascette_crypto::ContentKey;
use std::collections::HashMap;

/// Builder for creating install manifests
///
/// Provides a convenient way to construct install manifests step by step
/// with automatic handling of bit mask sizing and tag associations.
///
/// # Example
///
/// ```rust,no_run
/// use cascette_formats::install::{InstallManifestBuilder, TagType};
/// use cascette_crypto::ContentKey;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let manifest = InstallManifestBuilder::new()
///     .add_tag("Windows".to_string(), TagType::Platform)
///     .add_tag("x86_64".to_string(), TagType::Architecture)
///     .add_file(
///         "data/file1.bin".to_string(),
///         ContentKey::from_hex("0123456789abcdef0123456789abcdef")?,
///         1024,
///     )
///     .associate_file_with_tag(0, "Windows")?
///     .associate_file_with_tag(0, "x86_64")?
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct InstallManifestBuilder {
    tags: Vec<InstallTag>,
    entries: Vec<InstallFileEntry>,
    tag_name_to_index: HashMap<String, usize>,
}

impl InstallManifestBuilder {
    /// Create a new empty builder
    pub fn new() -> Self {
        Self {
            tags: Vec::new(),
            entries: Vec::new(),
            tag_name_to_index: HashMap::new(),
        }
    }

    /// Create builder from existing manifest
    pub fn from_manifest(manifest: &InstallManifest) -> Self {
        let tag_name_to_index = manifest
            .tags
            .iter()
            .enumerate()
            .map(|(i, tag)| (tag.name.clone(), i))
            .collect();

        Self {
            tags: manifest.tags.clone(),
            entries: manifest.entries.clone(),
            tag_name_to_index,
        }
    }

    /// Add a tag to the manifest
    ///
    /// Tags are used to categorize files for selective installation.
    /// The bit mask is automatically sized to accommodate current files.
    #[must_use]
    pub fn add_tag(mut self, name: String, tag_type: TagType) -> Self {
        let tag_index = self.tags.len();
        let bit_mask_size = self.entries.len().div_ceil(8);

        let tag = InstallTag {
            name: name.clone(),
            tag_type,
            bit_mask: vec![0u8; bit_mask_size],
        };

        self.tag_name_to_index.insert(name, tag_index);
        self.tags.push(tag);
        self
    }

    /// Add a file entry to the manifest
    ///
    /// Files are automatically assigned the next available index.
    /// All existing tag bit masks are resized to accommodate the new file.
    #[must_use]
    pub fn add_file(mut self, path: String, content_key: ContentKey, file_size: u32) -> Self {
        let entry = InstallFileEntry::new(path, content_key, file_size);
        self.entries.push(entry);

        // Resize all tag bit masks to accommodate new file
        let new_bit_mask_size = self.entries.len().div_ceil(8);
        for tag in &mut self.tags {
            if tag.bit_mask.len() < new_bit_mask_size {
                tag.bit_mask.resize(new_bit_mask_size, 0);
            }
        }

        self
    }

    /// Associate a file with a tag by index
    ///
    /// # Errors
    /// - `FileIndexOutOfBounds` if `file_index` is invalid
    /// - `TagNotFound` if `tag_name` doesn't exist
    pub fn associate_file_with_tag_by_index(
        mut self,
        file_index: usize,
        tag_index: usize,
    ) -> Result<Self> {
        if file_index >= self.entries.len() {
            return Err(InstallError::FileIndexOutOfBounds(file_index));
        }

        if tag_index >= self.tags.len() {
            return Err(InstallError::TagNotFound(format!("tag index {tag_index}")));
        }

        self.tags[tag_index].add_file(file_index);
        Ok(self)
    }

    /// Associate a file with a tag by name
    ///
    /// # Errors
    /// - `FileIndexOutOfBounds` if `file_index` is invalid
    /// - `TagNotFound` if `tag_name` doesn't exist
    pub fn associate_file_with_tag(mut self, file_index: usize, tag_name: &str) -> Result<Self> {
        if file_index >= self.entries.len() {
            return Err(InstallError::FileIndexOutOfBounds(file_index));
        }

        let tag_index = *self
            .tag_name_to_index
            .get(tag_name)
            .ok_or_else(|| InstallError::TagNotFound(tag_name.to_string()))?;

        self.tags[tag_index].add_file(file_index);
        Ok(self)
    }

    /// Remove file association with a tag
    pub fn remove_file_from_tag(mut self, file_index: usize, tag_name: &str) -> Result<Self> {
        if file_index >= self.entries.len() {
            return Err(InstallError::FileIndexOutOfBounds(file_index));
        }

        let tag_index = *self
            .tag_name_to_index
            .get(tag_name)
            .ok_or_else(|| InstallError::TagNotFound(tag_name.to_string()))?;

        self.tags[tag_index].remove_file(file_index);
        Ok(self)
    }

    /// Associate the most recently added file with a tag
    ///
    /// Convenience method for associating the last file added via `add_file`.
    ///
    /// # Errors
    /// - `FileIndexOutOfBounds` if no files have been added
    /// - `TagNotFound` if `tag_name` doesn't exist
    pub fn associate_last_file_with_tag(self, tag_name: &str) -> Result<Self> {
        if self.entries.is_empty() {
            return Err(InstallError::FileIndexOutOfBounds(0));
        }

        let last_index = self.entries.len() - 1;
        self.associate_file_with_tag(last_index, tag_name)
    }

    /// Add file and immediately associate it with tags
    ///
    /// Convenience method that combines `add_file` with multiple tag associations.
    pub fn add_file_with_tags(
        mut self,
        path: String,
        content_key: ContentKey,
        file_size: u32,
        tag_names: &[&str],
    ) -> Result<Self> {
        self = self.add_file(path, content_key, file_size);
        let file_index = self.entries.len() - 1;

        for &tag_name in tag_names {
            self = self.associate_file_with_tag(file_index, tag_name)?;
        }

        Ok(self)
    }

    /// Batch associate multiple files with a tag
    pub fn associate_files_with_tag(
        mut self,
        file_indices: &[usize],
        tag_name: &str,
    ) -> Result<Self> {
        for &file_index in file_indices {
            self = self.associate_file_with_tag(file_index, tag_name)?;
        }
        Ok(self)
    }

    /// Get current file count
    pub fn file_count(&self) -> usize {
        self.entries.len()
    }

    /// Get current tag count
    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }

    /// Check if a tag exists
    pub fn has_tag(&self, tag_name: &str) -> bool {
        self.tag_name_to_index.contains_key(tag_name)
    }

    /// Get tag names
    pub fn tag_names(&self) -> Vec<&String> {
        self.tag_name_to_index.keys().collect()
    }

    /// Preview total install size
    pub fn total_size(&self) -> u64 {
        self.entries
            .iter()
            .map(|entry| u64::from(entry.file_size))
            .sum()
    }

    /// Build the final install manifest
    ///
    /// Creates the header with current counts and validates the result.
    pub fn build(self) -> Result<InstallManifest> {
        let header = InstallHeader::new(
            u16::try_from(self.tags.len())
                .map_err(|_| InstallError::TagNotFound("Too many tags".to_string()))?,
            u32::try_from(self.entries.len())
                .map_err(|_| InstallError::TagNotFound("Too many entries".to_string()))?,
        );

        let manifest = InstallManifest {
            header,
            tags: self.tags,
            entries: self.entries,
        };

        // Validate the built manifest
        manifest.validate()?;

        Ok(manifest)
    }

    /// Reset the builder to empty state
    #[must_use]
    pub fn clear(mut self) -> Self {
        self.tags.clear();
        self.entries.clear();
        self.tag_name_to_index.clear();
        self
    }

    /// Remove a tag by name
    ///
    /// This removes the tag and updates indices for remaining tags.
    pub fn remove_tag(mut self, tag_name: &str) -> Result<Self> {
        let tag_index = *self
            .tag_name_to_index
            .get(tag_name)
            .ok_or_else(|| InstallError::TagNotFound(tag_name.to_string()))?;

        // Remove the tag
        self.tags.remove(tag_index);
        self.tag_name_to_index.remove(tag_name);

        // Update indices for remaining tags
        for index in self.tag_name_to_index.values_mut() {
            if *index > tag_index {
                *index -= 1;
            }
        }

        Ok(self)
    }

    /// Remove a file by index
    ///
    /// This removes the file and updates all tag bit masks accordingly.
    pub fn remove_file(mut self, file_index: usize) -> Result<Self> {
        if file_index >= self.entries.len() {
            return Err(InstallError::FileIndexOutOfBounds(file_index));
        }

        // Remove the file
        self.entries.remove(file_index);

        // Update all tag bit masks to remove the file
        for tag in &mut self.tags {
            // Remove the bit at file_index by shifting all subsequent bits down
            let mut new_mask = Vec::new();
            let total_bits = self.entries.len();
            let new_mask_size = total_bits.div_ceil(8);

            for byte_idx in 0..new_mask_size {
                let mut new_byte = 0u8;

                for bit_idx in 0..8 {
                    let old_file_idx = byte_idx * 8 + bit_idx;
                    if old_file_idx >= total_bits {
                        break;
                    }

                    // Map new file index to old file index
                    let old_mapped_idx = if old_file_idx < file_index {
                        old_file_idx // Before removed file, unchanged
                    } else {
                        old_file_idx + 1 // After removed file, shift up
                    };

                    // Check if old index was set in original mask
                    if old_mapped_idx < tag.bit_mask.len() * 8 {
                        let old_byte_idx = old_mapped_idx / 8;
                        let old_bit_idx = old_mapped_idx % 8;

                        if old_byte_idx < tag.bit_mask.len()
                            && (tag.bit_mask[old_byte_idx] & (0x80 >> old_bit_idx)) != 0
                        {
                            new_byte |= 0x80 >> bit_idx;
                        }
                    }
                }

                new_mask.push(new_byte);
            }

            tag.bit_mask = new_mask;
        }

        Ok(self)
    }

    /// Clone current state for branching
    #[must_use]
    pub fn snapshot(&self) -> Self {
        Self {
            tags: self.tags.clone(),
            entries: self.entries.clone(),
            tag_name_to_index: self.tag_name_to_index.clone(),
        }
    }
}

impl Default for InstallManifestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_new() {
        let builder = InstallManifestBuilder::new();
        assert_eq!(builder.file_count(), 0);
        assert_eq!(builder.tag_count(), 0);
        assert!(builder.tag_names().is_empty());
    }

    #[test]
    fn test_add_tag() {
        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("x86_64".to_string(), TagType::Architecture);

        assert_eq!(builder.tag_count(), 2);
        assert!(builder.has_tag("Windows"));
        assert!(builder.has_tag("x86_64"));
        assert!(!builder.has_tag("Mac"));
    }

    #[test]
    fn test_add_file() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let builder =
            InstallManifestBuilder::new().add_file("test.txt".to_string(), content_key, 1024);

        assert_eq!(builder.file_count(), 1);
        assert_eq!(builder.total_size(), 1024);
    }

    #[test]
    fn test_associate_file_with_tag() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file("test.txt".to_string(), content_key, 1024)
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed");

        let manifest = builder.build().expect("Operation should succeed");
        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");
        assert!(windows_tag.has_file(0));
    }

    #[test]
    fn test_associate_nonexistent_tag() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let result = InstallManifestBuilder::new()
            .add_file("test.txt".to_string(), content_key, 1024)
            .associate_file_with_tag(0, "NonExistent");

        assert!(matches!(result, Err(InstallError::TagNotFound(_))));
    }

    #[test]
    fn test_associate_invalid_file_index() {
        let result = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .associate_file_with_tag(99, "Windows");

        assert!(matches!(
            result,
            Err(InstallError::FileIndexOutOfBounds(99))
        ));
    }

    #[test]
    fn test_associate_last_file_with_tag() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file("test.txt".to_string(), content_key, 1024)
            .associate_last_file_with_tag("Windows")
            .expect("Operation should succeed");

        let manifest = builder.build().expect("Operation should succeed");
        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");
        assert!(windows_tag.has_file(0));
    }

    #[test]
    fn test_associate_last_file_no_files() {
        let result = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .associate_last_file_with_tag("Windows");

        assert!(matches!(result, Err(InstallError::FileIndexOutOfBounds(0))));
    }

    #[test]
    fn test_add_file_with_tags() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("x86_64".to_string(), TagType::Architecture)
            .add_file_with_tags(
                "test.txt".to_string(),
                content_key,
                1024,
                &["Windows", "x86_64"],
            )
            .expect("Operation should succeed");

        let manifest = builder.build().expect("Operation should succeed");
        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");
        let arch_tag = manifest
            .find_tag("x86_64")
            .expect("Operation should succeed");

        assert!(windows_tag.has_file(0));
        assert!(arch_tag.has_file(0));
    }

    #[test]
    fn test_batch_associate() {
        let content_key1 = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let content_key2 = ContentKey::from_hex("fedcba9876543210fedcba9876543210")
            .expect("Operation should succeed");

        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file("test1.txt".to_string(), content_key1, 1024)
            .add_file("test2.txt".to_string(), content_key2, 2048)
            .associate_files_with_tag(&[0, 1], "Windows")
            .expect("Operation should succeed");

        let manifest = builder.build().expect("Operation should succeed");
        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");

        assert!(windows_tag.has_file(0));
        assert!(windows_tag.has_file(1));
        assert_eq!(windows_tag.file_count(), 2);
    }

    #[test]
    fn test_remove_file_from_tag() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file("test.txt".to_string(), content_key, 1024)
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .remove_file_from_tag(0, "Windows")
            .expect("Operation should succeed");

        let manifest = builder.build().expect("Operation should succeed");
        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");
        assert!(!windows_tag.has_file(0));
    }

    #[test]
    fn test_remove_tag() {
        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("Linux".to_string(), TagType::Platform)
            .remove_tag("Windows")
            .expect("Operation should succeed");

        assert_eq!(builder.tag_count(), 1);
        assert!(!builder.has_tag("Windows"));
        assert!(builder.has_tag("Linux"));
    }

    #[test]
    fn test_remove_file() {
        let content_key1 = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let content_key2 = ContentKey::from_hex("fedcba9876543210fedcba9876543210")
            .expect("Operation should succeed");
        let content_key3 = ContentKey::from_hex("11111111111111112222222222222222")
            .expect("Operation should succeed");

        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file("file1.txt".to_string(), content_key1, 1024)
            .add_file("file2.txt".to_string(), content_key2, 2048)
            .add_file("file3.txt".to_string(), content_key3, 4096)
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .associate_file_with_tag(2, "Windows")
            .expect("Operation should succeed")
            .remove_file(1) // Remove middle file
            .expect("Operation should succeed");

        assert_eq!(builder.file_count(), 2);

        let manifest = builder.build().expect("Operation should succeed");
        assert_eq!(manifest.entries.len(), 2);
        assert_eq!(manifest.entries[0].path, "file1.txt");
        assert_eq!(manifest.entries[1].path, "file3.txt"); // file3 moved to index 1

        // Check that tag associations were updated correctly
        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");
        assert!(windows_tag.has_file(0)); // file1 still at index 0
        assert!(windows_tag.has_file(1)); // file3 moved to index 1
        assert_eq!(windows_tag.file_count(), 2);
    }

    #[test]
    fn test_clear() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let builder = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file("test.txt".to_string(), content_key, 1024)
            .clear();

        assert_eq!(builder.file_count(), 0);
        assert_eq!(builder.tag_count(), 0);
        assert_eq!(builder.total_size(), 0);
    }

    #[test]
    fn test_snapshot() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");
        let builder1 = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_file("test.txt".to_string(), content_key, 1024);

        let builder2 = builder1.snapshot();

        assert_eq!(builder1.file_count(), builder2.file_count());
        assert_eq!(builder1.tag_count(), builder2.tag_count());
        assert_eq!(builder1.total_size(), builder2.total_size());
    }

    #[test]
    fn test_from_manifest() {
        // Create original manifest
        let original = InstallManifestBuilder::new()
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("x86_64".to_string(), TagType::Architecture)
            .add_file(
                "test.txt".to_string(),
                ContentKey::from_hex("0123456789abcdef0123456789abcdef")
                    .expect("Operation should succeed"),
                1024,
            )
            .associate_file_with_tag(0, "Windows")
            .expect("Operation should succeed")
            .build()
            .expect("Operation should succeed");

        // Create builder from manifest
        let builder = InstallManifestBuilder::from_manifest(&original);
        assert_eq!(builder.file_count(), 1);
        assert_eq!(builder.tag_count(), 2);
        assert!(builder.has_tag("Windows"));
        assert!(builder.has_tag("x86_64"));

        // Verify we can build identical manifest
        let rebuilt = builder.build().expect("Operation should succeed");
        assert_eq!(original, rebuilt);
    }

    #[test]
    fn test_bit_mask_resize() {
        let content_key = ContentKey::from_hex("0123456789abcdef0123456789abcdef")
            .expect("Operation should succeed");

        let builder =
            InstallManifestBuilder::new().add_tag("Windows".to_string(), TagType::Platform); // Empty bit mask initially

        // Add many files to test bit mask resizing
        let mut builder = builder;
        for i in 0..100 {
            let path = format!("file_{i}.txt");
            builder = builder.add_file(path, content_key, 1024);

            // Associate every 10th file with Windows
            if i % 10 == 0 {
                builder = builder
                    .associate_file_with_tag(i, "Windows")
                    .expect("Operation should succeed");
            }
        }

        let manifest = builder.build().expect("Operation should succeed");
        let windows_tag = manifest
            .find_tag("Windows")
            .expect("Operation should succeed");

        assert_eq!(manifest.entries.len(), 100);
        assert_eq!(windows_tag.bit_mask.len(), 13); // (100 + 7) / 8 = 13 bytes
        assert_eq!(windows_tag.file_count(), 10); // Every 10th file

        // Verify specific files are associated
        for i in (0..100).step_by(10) {
            assert!(windows_tag.has_file(i));
        }
    }

    #[test]
    fn test_complex_workflow() {
        // Test a complex realistic workflow
        let mut builder = InstallManifestBuilder::new();

        // Add platform tags
        builder = builder
            .add_tag("Windows".to_string(), TagType::Platform)
            .add_tag("Mac".to_string(), TagType::Platform)
            .add_tag("x86_64".to_string(), TagType::Architecture)
            .add_tag("arm64".to_string(), TagType::Architecture)
            .add_tag("enUS".to_string(), TagType::Locale)
            .add_tag("deDE".to_string(), TagType::Locale);

        // Add files for different categories
        let files = [
            ("core/engine.dll", "Windows", vec!["x86_64"]),
            ("core/engine.dylib", "Mac", vec!["arm64"]),
            ("data/strings_en.db", "enUS", vec!["Windows", "Mac"]),
            ("data/strings_de.db", "deDE", vec!["Windows", "Mac"]),
            (
                "assets/textures.pak",
                "",
                vec!["Windows", "Mac", "x86_64", "arm64"],
            ),
        ];

        for (i, (path, primary_tag, secondary_tags)) in files.iter().enumerate() {
            let content_key = ContentKey::from_data(path.as_bytes());
            #[allow(clippy::cast_possible_truncation)]
            let file_size = (i as u32 + 1) * 1024;
            builder = builder.add_file((*path).to_string(), content_key, file_size);

            if !primary_tag.is_empty() {
                builder = builder
                    .associate_file_with_tag(i, primary_tag)
                    .expect("Operation should succeed");
            }

            for &secondary_tag in secondary_tags {
                builder = builder
                    .associate_file_with_tag(i, secondary_tag)
                    .expect("Operation should succeed");
            }
        }

        let manifest = builder.build().expect("Operation should succeed");
        assert_eq!(manifest.entries.len(), 5);
        assert_eq!(manifest.tags.len(), 6);

        // Verify associations
        let windows_files = manifest.get_files_for_tag("Windows");
        assert_eq!(windows_files.len(), 4); // engine.dll, strings_en, strings_de, textures

        let mac_x64_files = manifest.get_files_for_tags(&["Mac", "x86_64"]);
        assert_eq!(mac_x64_files.len(), 1); // Only textures.pak has both

        assert!(manifest.validate().is_ok());
    }
}
