//! Install manifest parser for TACT
//!
//! The install manifest lists files that need to be installed for the game to run.
//! Files are associated with tags (e.g., "Windows", "Mac", "enUS") using a bitmask system.

use byteorder::{BigEndian, ReadBytesExt};
use std::io::{Cursor, Read};
use tracing::{debug, trace};

use crate::{Error, Result};
use crate::utils::read_cstring_from;

/// Magic bytes for install manifest: "IN"
const INSTALL_MAGIC: [u8; 2] = [0x49, 0x4E]; // 'I', 'N'

/// Install manifest header
#[derive(Debug, Clone)]
pub struct InstallHeader {
    /// Magic bytes "IN"
    pub magic: [u8; 2],
    /// Version (should be 1)
    pub version: u8,
    /// Hash size (usually 16 for MD5)
    pub hash_size: u8,
    /// Number of tags
    pub tag_count: u16,
    /// Number of file entries
    pub entry_count: u32,
}

/// Install tag information
#[derive(Debug, Clone)]
pub struct InstallTag {
    /// Tag name (e.g., "Windows", "Mac", "enUS")
    pub name: String,
    /// Tag type/flags
    pub tag_type: u16,
    /// Bitmask indicating which files have this tag
    pub files_mask: Vec<bool>,
}

/// Install file entry
#[derive(Debug, Clone)]
pub struct InstallEntry {
    /// File path relative to game root
    pub path: String,
    /// Content key (CKey)
    pub ckey: Vec<u8>,
    /// File size
    pub size: u32,
    /// Tags associated with this file
    pub tags: Vec<String>,
}

/// Install manifest
pub struct InstallManifest {
    /// File header
    pub header: InstallHeader,
    /// List of tags
    pub tags: Vec<InstallTag>,
    /// List of file entries
    pub entries: Vec<InstallEntry>,
}

/// Common platform tags
#[derive(Debug, Clone, PartialEq)]
pub enum Platform {
    Windows,
    Mac,
    Linux,
    All,
}

impl Platform {
    /// Get the tag name for this platform
    pub fn tag_name(&self) -> &str {
        match self {
            Platform::Windows => "Windows",
            Platform::Mac => "OSX",
            Platform::Linux => "Linux",
            Platform::All => "",
        }
    }
}

impl InstallManifest {
    /// Parse an install manifest from raw data
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut cursor = Cursor::new(data);
        
        // Parse header
        let header = Self::parse_header(&mut cursor)?;
        debug!("Parsed install header: version={}, tags={}, entries={}", 
               header.version, header.tag_count, header.entry_count);
        
        // Calculate bytes per tag for bitmask
        let bytes_per_tag = ((header.entry_count + 7) / 8) as usize;
        
        // Parse tags
        let mut tags = Vec::with_capacity(header.tag_count as usize);
        for i in 0..header.tag_count {
            let name = read_cstring_from(&mut cursor)?;
            let tag_type = cursor.read_u16::<BigEndian>()?;
            
            // Read bitmask for this tag
            let mut mask_bytes = vec![0u8; bytes_per_tag];
            cursor.read_exact(&mut mask_bytes)?;
            
            // Convert bytes to bool vector
            let mut files_mask = Vec::with_capacity(header.entry_count as usize);
            for byte in mask_bytes {
                for bit in 0..8 {
                    if files_mask.len() < header.entry_count as usize {
                        files_mask.push((byte & (1 << bit)) != 0);
                    }
                }
            }
            
            trace!("Tag {}: name='{}', type={:#06x}, files_with_tag={}", 
                   i, name, tag_type, files_mask.iter().filter(|&&b| b).count());
            
            tags.push(InstallTag {
                name,
                tag_type,
                files_mask,
            });
        }
        
        // Parse file entries
        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for i in 0..header.entry_count {
            let path = read_cstring_from(&mut cursor)?;
            
            let mut ckey = vec![0u8; header.hash_size as usize];
            cursor.read_exact(&mut ckey)?;
            
            let size = cursor.read_u32::<BigEndian>()?;
            
            // Resolve tags for this entry
            let mut entry_tags = Vec::new();
            for tag in &tags {
                if tag.files_mask[i as usize] {
                    entry_tags.push(tag.name.clone());
                }
            }
            
            entries.push(InstallEntry {
                path,
                ckey,
                size,
                tags: entry_tags,
            });
        }
        
        debug!("Parsed {} install entries", entries.len());
        
        Ok(InstallManifest {
            header,
            tags,
            entries,
        })
    }
    
    /// Parse the install manifest header
    fn parse_header<R: Read>(reader: &mut R) -> Result<InstallHeader> {
        let mut magic = [0u8; 2];
        reader.read_exact(&mut magic)?;
        
        if magic != INSTALL_MAGIC {
            return Err(Error::BadMagic);
        }
        
        let version = reader.read_u8()?;
        let hash_size = reader.read_u8()?;
        let tag_count = reader.read_u16::<BigEndian>()?;
        let entry_count = reader.read_u32::<BigEndian>()?;
        
        Ok(InstallHeader {
            magic,
            version,
            hash_size,
            tag_count,
            entry_count,
        })
    }
    
    /// Get all files that have specific tags
    pub fn get_files_for_tags(&self, required_tags: &[&str]) -> Vec<&InstallEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                required_tags.iter().all(|tag| entry.tags.contains(&tag.to_string()))
            })
            .collect()
    }
    
    /// Get all files for a specific platform
    pub fn get_files_for_platform(&self, platform: Platform) -> Vec<&InstallEntry> {
        if platform == Platform::All {
            return self.entries.iter().collect();
        }
        
        let tag_name = platform.tag_name();
        self.get_files_for_tags(&[tag_name])
    }
    
    /// Get all unique tags in the manifest
    pub fn get_all_tags(&self) -> Vec<&str> {
        self.tags.iter().map(|t| t.name.as_str()).collect()
    }
    
    /// Get a specific file by path
    pub fn get_file_by_path(&self, path: &str) -> Option<&InstallEntry> {
        self.entries.iter().find(|e| e.path == path)
    }
    
    /// Calculate total size for files with specific tags
    pub fn calculate_size_for_tags(&self, tags: &[&str]) -> u64 {
        self.get_files_for_tags(tags)
            .iter()
            .map(|entry| entry.size as u64)
            .sum()
    }
    
    /// Calculate total size for a platform
    pub fn calculate_size_for_platform(&self, platform: Platform) -> u64 {
        self.get_files_for_platform(platform)
            .iter()
            .map(|entry| entry.size as u64)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_install_header_size() {
        // Header should be exactly 10 bytes
        let header_size = 2 + 1 + 1 + 2 + 4;
        assert_eq!(header_size, 10);
    }
    
    #[test]
    fn test_parse_empty_install() {
        // Create a minimal valid install manifest
        let mut data = Vec::new();
        
        // Magic
        data.extend_from_slice(&INSTALL_MAGIC);
        // Version
        data.push(1);
        // Hash size
        data.push(16);
        // Tag count (big-endian!)
        data.extend_from_slice(&0u16.to_be_bytes());
        // Entry count (big-endian!)
        data.extend_from_slice(&0u32.to_be_bytes());
        
        let result = InstallManifest::parse(&data);
        assert!(result.is_ok());
        
        let manifest = result.unwrap();
        assert_eq!(manifest.header.version, 1);
        assert_eq!(manifest.header.hash_size, 16);
        assert_eq!(manifest.tags.len(), 0);
        assert_eq!(manifest.entries.len(), 0);
    }
    
    #[test]
    fn test_invalid_magic() {
        let mut data = vec![0xFF, 0xFF]; // Wrong magic
        data.push(1); // Version
        
        let result = InstallManifest::parse(&data);
        assert!(matches!(result, Err(Error::BadMagic)));
    }
    
    #[test]
    fn test_parse_with_tags() {
        // Create an install manifest with one tag and one file
        let mut data = Vec::new();
        
        // Header
        data.extend_from_slice(&INSTALL_MAGIC);
        data.push(1); // Version
        data.push(16); // Hash size
        data.extend_from_slice(&1u16.to_be_bytes()); // 1 tag
        data.extend_from_slice(&1u32.to_be_bytes()); // 1 entry
        
        // Tag
        data.extend_from_slice(b"Windows\0"); // Tag name
        data.extend_from_slice(&0u16.to_be_bytes()); // Tag type
        data.push(0x01); // Bitmask: first file has this tag
        
        // Entry
        data.extend_from_slice(b"test.exe\0"); // File path
        data.extend_from_slice(&[0u8; 16]); // CKey (16 bytes of zeros)
        data.extend_from_slice(&1024u32.to_be_bytes()); // Size
        
        let result = InstallManifest::parse(&data);
        assert!(result.is_ok());
        
        let manifest = result.unwrap();
        assert_eq!(manifest.tags.len(), 1);
        assert_eq!(manifest.tags[0].name, "Windows");
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].path, "test.exe");
        assert_eq!(manifest.entries[0].size, 1024);
        assert!(manifest.entries[0].tags.contains(&"Windows".to_string()));
    }
    
    #[test]
    fn test_platform_filtering() {
        // Create a manifest with Windows and Mac files
        let mut data = Vec::new();
        
        // Header
        data.extend_from_slice(&INSTALL_MAGIC);
        data.push(1); // Version
        data.push(16); // Hash size
        data.extend_from_slice(&2u16.to_be_bytes()); // 2 tags
        data.extend_from_slice(&2u32.to_be_bytes()); // 2 entries
        
        // Tag 1: Windows
        data.extend_from_slice(b"Windows\0");
        data.extend_from_slice(&0u16.to_be_bytes());
        data.push(0x01); // First file has Windows tag
        
        // Tag 2: OSX
        data.extend_from_slice(b"OSX\0");
        data.extend_from_slice(&0u16.to_be_bytes());
        data.push(0x02); // Second file has OSX tag
        
        // Entry 1: Windows file
        data.extend_from_slice(b"windows.exe\0");
        data.extend_from_slice(&[1u8; 16]);
        data.extend_from_slice(&1000u32.to_be_bytes());
        
        // Entry 2: Mac file
        data.extend_from_slice(b"mac.app\0");
        data.extend_from_slice(&[2u8; 16]);
        data.extend_from_slice(&2000u32.to_be_bytes());
        
        let manifest = InstallManifest::parse(&data).unwrap();
        
        // Test platform filtering
        let windows_files = manifest.get_files_for_platform(Platform::Windows);
        assert_eq!(windows_files.len(), 1);
        assert_eq!(windows_files[0].path, "windows.exe");
        
        let mac_files = manifest.get_files_for_platform(Platform::Mac);
        assert_eq!(mac_files.len(), 1);
        assert_eq!(mac_files[0].path, "mac.app");
        
        // Test size calculation
        assert_eq!(manifest.calculate_size_for_platform(Platform::Windows), 1000);
        assert_eq!(manifest.calculate_size_for_platform(Platform::Mac), 2000);
        assert_eq!(manifest.calculate_size_for_platform(Platform::All), 3000);
    }
}