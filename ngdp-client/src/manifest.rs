// Manifest abstraction layer for NGDP client
// This module provides a unified interface for working with different manifest types
// while maintaining explicit knowledge of key types throughout the pipeline

use std::fmt;
use tact_parser::download::DownloadManifest;
use tact_parser::encoding::EncodingFile;
use tact_parser::install::InstallManifest;

/// Strongly typed content key (CKey)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentKey(pub Vec<u8>);

impl ContentKey {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
}

impl fmt::Display for ContentKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CKey:{}", self.to_hex())
    }
}

/// Strongly typed encoding key (EKey)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EncodingKey(pub Vec<u8>);

impl EncodingKey {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(&self.0)
    }
}

impl fmt::Display for EncodingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EKey:{}", self.to_hex())
    }
}

/// A file entry from install manifest (has paths and CKeys)
#[derive(Debug, Clone)]
pub struct InstallManifestEntry {
    /// Actual file path in the game installation
    pub path: String,
    /// Content key for this file
    pub content_key: ContentKey,
    /// File size in bytes
    pub size: usize,
}

/// A file entry from download manifest (has EKeys but no paths)
#[derive(Debug, Clone)]
pub struct DownloadManifestEntry {
    /// Generated path (since download manifest has no paths)
    pub generated_path: String,
    /// Encoding key for this file
    pub encoding_key: EncodingKey,
    /// Compressed size in bytes
    pub compressed_size: usize,
    /// Download priority (0 = highest)
    pub priority: i32,
}

/// Type-safe manifest that knows its type at compile time
#[derive(Debug)]
pub enum TypedManifest {
    Install(TypedInstallManifest),
    Download(TypedDownloadManifest),
}

/// Strongly typed install manifest
#[derive(Debug)]
pub struct TypedInstallManifest {
    entries: Vec<InstallManifestEntry>,
}

/// Strongly typed download manifest
#[derive(Debug)]
pub struct TypedDownloadManifest {
    entries: Vec<DownloadManifestEntry>,
}

impl TypedInstallManifest {
    pub fn from_raw(manifest: InstallManifest) -> Self {
        let entries = manifest
            .entries
            .iter()
            .map(|entry| InstallManifestEntry {
                path: entry.path.clone(),
                content_key: ContentKey::new(entry.ckey.clone()),
                size: entry.size as usize,
            })
            .collect();

        Self { entries }
    }

    pub fn entries(&self) -> &[InstallManifestEntry] {
        &self.entries
    }

    /// Get entries that require CKey -> EKey resolution
    pub fn entries_requiring_encoding_lookup(&self) -> Vec<&InstallManifestEntry> {
        self.entries.iter().collect()
    }
}

impl TypedDownloadManifest {
    pub fn from_raw(manifest: DownloadManifest) -> Self {
        let entries = manifest
            .entries
            .iter()
            .enumerate()
            .map(|(i, (ekey, entry))| DownloadManifestEntry {
                generated_path: format!("data/{:08x}", i),
                encoding_key: EncodingKey::new(ekey.clone()),
                compressed_size: entry.compressed_size as usize,
                priority: entry.priority as i32,
            })
            .collect();

        Self { entries }
    }

    pub fn entries(&self) -> &[DownloadManifestEntry] {
        &self.entries
    }

    /// Get entries that can be used directly with archive indices (already have EKeys)
    pub fn entries_with_encoding_keys(&self) -> Vec<&DownloadManifestEntry> {
        self.entries.iter().collect()
    }
}

/// A downloadable file with resolved keys
#[derive(Debug, Clone)]
pub struct DownloadableFile {
    /// Path where file will be saved
    pub path: String,
    /// Encoding key for downloading (what archives use)
    pub encoding_key: EncodingKey,
    /// Content key for verification (optional)
    pub content_key: Option<ContentKey>,
    /// Expected file size
    pub size: usize,
    /// Source manifest type (for debugging)
    pub source: ManifestSource,
}

#[derive(Debug, Clone, Copy)]
pub enum ManifestSource {
    InstallManifest,
    DownloadManifest,
}

/// Key resolver that maintains type safety
pub struct TypedKeyResolver<'a> {
    encoding_file: &'a EncodingFile,
}

impl<'a> TypedKeyResolver<'a> {
    pub fn new(encoding_file: &'a EncodingFile) -> Self {
        Self { encoding_file }
    }

    /// Convert install manifest entry to downloadable file (CKey -> EKey resolution)
    pub fn resolve_install_entry(
        &self,
        entry: &InstallManifestEntry,
    ) -> Result<DownloadableFile, String> {
        // Look up EKey from CKey
        let encoding_entry = self
            .encoding_file
            .lookup_by_ckey(entry.content_key.as_bytes())
            .ok_or_else(|| format!("No encoding entry for {}", entry.content_key))?;

        let encoding_key = encoding_entry
            .encoding_keys
            .first()
            .ok_or_else(|| format!("No encoding keys for {}", entry.content_key))?;

        Ok(DownloadableFile {
            path: entry.path.clone(),
            encoding_key: EncodingKey::new(encoding_key.clone()),
            content_key: Some(entry.content_key.clone()),
            size: self
                .encoding_file
                .get_file_size(entry.content_key.as_bytes())
                .map(|s| s as usize)
                .unwrap_or(entry.size),
            source: ManifestSource::InstallManifest,
        })
    }

    /// Convert download manifest entry to downloadable file (already has EKey)
    pub fn resolve_download_entry(
        &self,
        entry: &DownloadManifestEntry,
    ) -> Result<DownloadableFile, String> {
        // Download manifest already has EKey, optionally look up CKey
        let content_key = self
            .encoding_file
            .lookup_by_ekey(entry.encoding_key.as_bytes())
            .map(|ckey| ContentKey::new(ckey.clone()));

        let size = content_key
            .as_ref()
            .and_then(|ckey| self.encoding_file.get_file_size(ckey.as_bytes()))
            .map(|s| s as usize)
            .unwrap_or(entry.compressed_size);

        Ok(DownloadableFile {
            path: entry.generated_path.clone(),
            encoding_key: entry.encoding_key.clone(),
            content_key,
            size,
            source: ManifestSource::DownloadManifest,
        })
    }

    /// Resolve any manifest to downloadable files
    pub fn resolve_manifest(
        &self,
        manifest: &TypedManifest,
    ) -> Vec<Result<DownloadableFile, String>> {
        match manifest {
            TypedManifest::Install(m) => m
                .entries()
                .iter()
                .map(|e| self.resolve_install_entry(e))
                .collect(),
            TypedManifest::Download(m) => m
                .entries()
                .iter()
                .map(|e| self.resolve_download_entry(e))
                .collect(),
        }
    }
}

/// Installation strategy that knows which manifest to use
#[derive(Debug, Clone, Copy)]
pub enum InstallStrategy {
    /// Use install manifest for accurate file paths and content verification
    UseInstallManifest,
    /// Use download manifest for streaming/partial downloads
    UseDownloadManifest,
    /// Automatically choose based on install type
    Auto,
}

impl InstallStrategy {
    /// Determine which manifest to use based on install type
    pub fn select_manifest(self, install_type: InstallType) -> ManifestChoice {
        match self {
            Self::UseInstallManifest => ManifestChoice::Install,
            Self::UseDownloadManifest => ManifestChoice::Download,
            Self::Auto => {
                match install_type {
                    InstallType::Minimal => ManifestChoice::Download, // Better for partial
                    InstallType::Full => ManifestChoice::Install,     // Better for complete
                    InstallType::Custom(_) => ManifestChoice::Download, // Flexible priority
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ManifestChoice {
    Install,
    Download,
}

/// Installation type
#[derive(Debug, Clone, Copy)]
pub enum InstallType {
    Minimal,
    Full,
    Custom(i32), // Priority threshold
}
