# Key Service Implementation Guide

## Overview

The Key Service manages encryption keys for BLTE decryption. This guide provides implementation instructions based on the prototype's comprehensive key management system.

## Architecture

```
KeyService
├── Hardcoded keys (known WoW keys)
├── File-based keys (multiple formats)
├── Directory searching (standard locations)
└── Runtime key addition
```

## Implementation

### Step 1: Core Structure

```rust
// src/key_service.rs
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct KeyService {
    /// Map of key_name (u64) to key_data ([u8; 16])
    keys: HashMap<u64, [u8; 16]>,
}

impl KeyService {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }
    
    pub fn with_default_keys() -> Self {
        let mut service = Self::new();
        service.add_hardcoded_keys();
        service.load_from_standard_directories();
        service
    }
    
    pub fn get_key(&self, key_name: u64) -> Option<&[u8; 16]> {
        self.keys.get(&key_name)
    }
    
    pub fn add_key(&mut self, key_name: u64, key_data: [u8; 16]) {
        self.keys.insert(key_name, key_data);
    }
    
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }
}
```

### Step 2: Hardcoded Keys

```rust
impl KeyService {
    fn add_hardcoded_keys(&mut self) {
        // Known BLTE encryption keys from various WoW builds
        // These are publicly known keys extracted from game clients
        let known_keys = [
            // Battle for Azeroth
            (0xFA505078126ACB3E, "BDC51862ABED79B2DE48C8E7E66C6200"),
            (0xFF813F7D062AC0BC, "AA0B5C77F088CCC2D39049BD267F066D"),
            (0xD1E9B5EDF9283668, "8E4A2579894E341081FFF96BC5B0FDFA"),
            (0xB76729B17E61372C, "9849D1AA7B1FD09819C5C66283A326EC"),
            (0xFFB9469FF41B8B9B, "D514BD1909A9E5DC8703F4B8BB1DFD9A"),
            
            // Shadowlands
            (0x23C5B5DF837A226C, "1406E2D873B6FC99217A180881DA8D62"),
            (0x3AE403EF40AC3037, "67197BCD9D0EF0C4085378FAA69A3264"),
            (0xE2854581EFE608FB, "0196CB6F5ECBAD7CB5283891B9712B4D"),
            (0x8C9106108AA84F07, "93B9CE598DB087F2829A2F74E6E51FBB"),
            
            // Dragonflight
            (0x49166D358A34D815, "667868CE64CB0E2EC6EDD8797175C140"),
            (0xA2F7BB6737FAF348, "C14C9D0A5067EA5942B83FCB21692FAD"),
            (0x5E5D896B3E163DEA, "23676C2F0AF0E41F43D0BB8F66BB6A6C"),
            (0x0EBE8B5D4ADB42C7, "317B8CE7B8B9C8B7F8F3F3B2B5B7C5C7"),
            
            // Add more keys as discovered
        ];
        
        for (key_name, key_hex) in known_keys.iter() {
            if let Ok(key_data) = hex_to_array(key_hex) {
                self.add_key(*key_name, key_data);
            }
        }
    }
}

fn hex_to_array(hex: &str) -> Result<[u8; 16]> {
    let bytes = hex::decode(hex)
        .context("Invalid hex string")?;
    
    if bytes.len() != 16 {
        anyhow::bail!("Key must be exactly 16 bytes, got {}", bytes.len());
    }
    
    let mut array = [0u8; 16];
    array.copy_from_slice(&bytes);
    Ok(array)
}
```

### Step 3: Directory Search

```rust
impl KeyService {
    fn get_search_directories() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        
        // Config directory (~/.config on Linux/Mac, %APPDATA% on Windows)
        if let Some(config_dir) = dirs::config_dir() {
            paths.push(config_dir.join("cascette"));
            paths.push(config_dir.join("TactKeys"));
            paths.push(config_dir.join("wow-tools"));
        }
        
        // Data directory
        if let Some(data_dir) = dirs::data_dir() {
            paths.push(data_dir.join("cascette"));
            paths.push(data_dir.join("TactKeys"));
        }
        
        // Home directory
        if let Some(home_dir) = dirs::home_dir() {
            paths.push(home_dir.join(".cascette"));
            paths.push(home_dir.join(".tactkeys"));
        }
        
        // Current directory
        paths.push(PathBuf::from("."));
        paths.push(PathBuf::from("keys"));
        
        // Environment variable override
        if let Ok(custom_path) = std::env::var("CASCETTE_KEYS_PATH") {
            paths.push(PathBuf::from(custom_path));
        }
        
        paths
    }
    
    fn load_from_standard_directories(&mut self) {
        let search_paths = Self::get_search_directories();
        
        for dir in search_paths {
            if !dir.exists() || !dir.is_dir() {
                continue;
            }
            
            // Try common key file names
            let key_files = [
                "TactKeys.csv",
                "TactKeys.txt",
                "WoW.txt",
                "keys.txt",
                "keys.csv",
            ];
            
            for filename in key_files.iter() {
                let file_path = dir.join(filename);
                if file_path.exists() {
                    if let Err(e) = self.load_key_file(&file_path) {
                        log::debug!("Failed to load {}: {}", file_path.display(), e);
                    }
                }
            }
        }
    }
}
```

### Step 4: File Loading

```rust
impl KeyService {
    pub fn load_key_file(&mut self, path: &Path) -> Result<usize> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read key file: {}", path.display()))?;
        
        let mut loaded = 0;
        
        for line in content.lines() {
            // Skip comments and empty lines
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            
            // Try different formats
            if let Some(loaded_count) = self.try_parse_line(line) {
                loaded += loaded_count;
            }
        }
        
        log::info!("Loaded {} keys from {}", loaded, path.display());
        Ok(loaded)
    }
    
    fn try_parse_line(&mut self, line: &str) -> Option<usize> {
        // Format 1: CSV (keyname,keyhex)
        if let Some((name, key)) = line.split_once(',') {
            return self.try_add_key_pair(name.trim(), key.trim());
        }
        
        // Format 2: Space-separated (keyname keyhex)
        if let Some((name, key)) = line.split_once(' ') {
            return self.try_add_key_pair(name.trim(), key.trim());
        }
        
        // Format 3: Tab-separated (keyname\tkeyhex)
        if let Some((name, key)) = line.split_once('\t') {
            return self.try_add_key_pair(name.trim(), key.trim());
        }
        
        // Format 4: Equals-separated (keyname=keyhex)
        if let Some((name, key)) = line.split_once('=') {
            return self.try_add_key_pair(name.trim(), key.trim());
        }
        
        // Format 5: WoW.txt format (keyname key description)
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            return self.try_add_key_pair(parts[0], parts[1]);
        }
        
        None
    }
    
    fn try_add_key_pair(&mut self, name_str: &str, key_str: &str) -> Option<usize> {
        // Parse key name (hex u64)
        let key_name = if name_str.starts_with("0x") || name_str.starts_with("0X") {
            u64::from_str_radix(&name_str[2..], 16).ok()?
        } else {
            u64::from_str_radix(name_str, 16).ok()?
        };
        
        // Parse key data (hex string to [u8; 16])
        let key_data = hex_to_array(key_str).ok()?;
        
        self.add_key(key_name, key_data);
        Some(1)
    }
}
```

### Step 5: Builder Pattern

```rust
pub struct KeyServiceBuilder {
    service: KeyService,
    load_defaults: bool,
    search_directories: bool,
    custom_paths: Vec<PathBuf>,
}

impl KeyServiceBuilder {
    pub fn new() -> Self {
        Self {
            service: KeyService::new(),
            load_defaults: true,
            search_directories: true,
            custom_paths: Vec::new(),
        }
    }
    
    pub fn without_defaults(mut self) -> Self {
        self.load_defaults = false;
        self
    }
    
    pub fn without_directory_search(mut self) -> Self {
        self.search_directories = false;
        self
    }
    
    pub fn add_key_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.custom_paths.push(path.into());
        self
    }
    
    pub fn add_key(mut self, key_name: u64, key_data: [u8; 16]) -> Self {
        self.service.add_key(key_name, key_data);
        self
    }
    
    pub fn build(mut self) -> KeyService {
        if self.load_defaults {
            self.service.add_hardcoded_keys();
        }
        
        if self.search_directories {
            self.service.load_from_standard_directories();
        }
        
        for path in self.custom_paths {
            if let Err(e) = self.service.load_key_file(&path) {
                log::warn!("Failed to load key file {}: {}", path.display(), e);
            }
        }
        
        self.service
    }
}
```

## Usage Examples

### Basic Usage

```rust
// Create with all defaults
let key_service = KeyService::with_default_keys();

// Get a key
if let Some(key) = key_service.get_key(0xFA505078126ACB3E) {
    // Use key for decryption
}

// Add a runtime key
let mut key_service = KeyService::new();
key_service.add_key(0x1234567890ABCDEF, [0x01; 16]);
```

### Custom Configuration

```rust
// Build with specific configuration
let key_service = KeyServiceBuilder::new()
    .without_directory_search()  // Don't search standard dirs
    .add_key_file("/path/to/custom/keys.txt")
    .add_key(0xABCDEF, [0x42; 16])  // Add specific key
    .build();
```

### Loading from Environment

```rust
// Set environment variable
std::env::set_var("CASCETTE_KEYS_PATH", "/custom/keys/directory");

// Will automatically search this directory
let key_service = KeyService::with_default_keys();
```

## Key File Formats

### TactKeys.csv Format
```csv
# KeyName,KeyHex
FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200
FF813F7D062AC0BC,AA0B5C77F088CCC2D39049BD267F066D
```

### WoW.txt Format
```
# keyname key description
FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200 BfA Season 4
FF813F7D062AC0BC AA0B5C77F088CCC2D39049BD267F066D Shadowlands Beta
```

### Simple Format
```
FA505078126ACB3E=BDC51862ABED79B2DE48C8E7E66C6200
FF813F7D062AC0BC=AA0B5C77F088CCC2D39049BD267F066D
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_hardcoded_keys() {
        let service = KeyService::with_default_keys();
        
        // Should have at least the hardcoded keys
        assert!(service.key_count() >= 13);
        
        // Check specific key exists
        assert!(service.get_key(0xFA505078126ACB3E).is_some());
    }
    
    #[test]
    fn test_key_file_loading() {
        let temp_dir = TempDir::new().unwrap();
        let key_file = temp_dir.path().join("keys.txt");
        
        std::fs::write(&key_file, 
            "DEADBEEF12345678,0102030405060708090A0B0C0D0E0F10\n\
             # Comment line\n\
             ABCDEF0123456789 11121314151617181 91A1B1C1D1E1F20"
        ).unwrap();
        
        let mut service = KeyService::new();
        let loaded = service.load_key_file(&key_file).unwrap();
        
        assert_eq!(loaded, 2);
        assert!(service.get_key(0xDEADBEEF12345678).is_some());
        assert!(service.get_key(0xABCDEF0123456789).is_some());
    }
    
    #[test]
    fn test_hex_parsing() {
        let result = hex_to_array("0102030405060708090A0B0C0D0E0F10");
        assert!(result.is_ok());
        
        let expected = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10
        ];
        assert_eq!(result.unwrap(), expected);
    }
}
```

## Security Considerations

1. **Key Storage**: Keys are stored in memory only
2. **No Logging**: Never log key values
3. **File Permissions**: Ensure key files have restricted permissions
4. **Clear on Drop**: Consider clearing keys from memory when dropped

```rust
impl Drop for KeyService {
    fn drop(&mut self) {
        // Overwrite keys in memory
        for (_, key) in self.keys.iter_mut() {
            key.fill(0);
        }
    }
}
```

## Integration Points

### With BLTE Decryption

```rust
pub fn decrypt_blte_with_service(
    data: &[u8],
    key_service: &KeyService,
) -> Result<Vec<u8>> {
    // Parse encrypted block to get key_name
    let key_name = parse_key_name(data)?;
    
    // Get key from service
    let key = key_service.get_key(key_name)
        .ok_or_else(|| anyhow!("Key {:016X} not found", key_name))?;
    
    // Decrypt using key
    decrypt_with_key(data, key)
}
```

### With CLI

```rust
// In CLI main
let key_service = if let Some(key_file) = args.key_file {
    KeyServiceBuilder::new()
        .add_key_file(key_file)
        .build()
} else {
    KeyService::with_default_keys()
};
```

## Performance Notes

1. **HashMap Lookup**: O(1) key retrieval
2. **Lazy Loading**: Load keys only when needed
3. **Caching**: Keys remain in memory for session

## Common Issues

1. **Missing Keys**: Not all keys are publicly known
2. **Format Variations**: Different tools use different formats
3. **Path Issues**: Ensure search directories exist
4. **Permissions**: Key files must be readable

## Next Steps

1. Implement key service in project
2. Add more known keys as discovered
3. Create key update mechanism
4. Add key export functionality