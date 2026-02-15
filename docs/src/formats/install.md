# Install Manifest Format

The Install manifest tracks which game files should be installed on disk and
manages file tags for selective installation based on system requirements and
user preferences.

## Overview

The Install manifest provides:

- Mapping of files to installation paths

- Tag-based file categorization

- Selective installation support

- Platform-specific file filtering

- Size tracking for installation planning

## File Structure

The Install manifest is BLTE-encoded and contains:

```text
[BLTE Container]
  [Header]
  [Tag Section]
  [File Entries]
```

## Binary Format

### Header

```c
struct InstallHeader {
    uint16_t magic;              // 'IN' (0x494E)
    uint8_t  version;            // Version (1 or 2)
    uint8_t  ckey_length;        // Content key length in bytes (16)
    uint16_t tag_count;          // Number of tags (big-endian)
    uint32_t entry_count;        // Number of file entries (big-endian)

    // Version 2+ fields (6 additional bytes, total 16 bytes)
    uint8_t  loose_file_type;    // File type for loose files
    uint32_t extra_entry_count;  // Additional entry count (big-endian)
    uint8_t  entry_size;         // Per-entry byte size
};
```

For version 1, the entry size is derived as `ckey_length + 4` (content key +
4-byte file size). Version 2 specifies entry_size explicitly.

### Tag Section

Tags categorize files for selective installation. Each tag consists of:

```c
struct InstallTag {
    char     name[];             // Null-terminated tag name
    uint16_t type;               // Tag type (big-endian)
    uint8_t  bit_mask[];         // Bit mask ((entry_count + 7) / 8 bytes)
};
```

**Important**: The bit mask uses **little-endian bit ordering** within each
byte:

- Bit 0 (LSB) corresponds to file index `byte_index * 8 + 0`

- Bit 7 (MSB) corresponds to file index `byte_index * 8 + 7`

- No bit reversal is needed (contrary to some community documentation)

### File Entry

File entries follow the tag section:

```c
struct InstallFileEntry {
    char     path[];             // Null-terminated file path
    uint8_t  content_key[16];    // MD5 content key
    uint32_t file_size;          // File size (big-endian)
};
```

Tag associations are determined by bit positions in each tag's bit mask.

## Tag System

### Tag Types

| Type | Value | Description | Examples |
|------|-------|-------------|----------|
| Platform | 0x0001 | Operating system tags | Windows, OSX, Android, IOS |
| Architecture | 0x0002 | CPU architecture tags | x86_32, x86_64, arm64 |
| Locale | 0x0003 | Language/region tags | enUS, deDE, frFR |
| Category | 0x0004 | Content category tags | speech, text |
| Alternate | 0x4000 | Alternate content | Alternate, HighRes |

### Common Tags

```text
Platform Tags:

- Windows, OSX, Android, IOS, Web

Architecture Tags:

- x86_32, x86_64, arm64

Locale Tags:

- enUS, enGB, deDE, frFR, esES, esMX, itIT,
  ruRU, koKR, zhTW, zhCN, ptBR, ptPT

Category Tags:

- speech, text

Alternate Tags:

- Alternate, HighRes
```

### Tag Mask Usage

Tags use bit masks to indicate which files they apply to:

```rust
fn should_install(
    file_index: usize,
    tag: &InstallTag,
    selected: bool
) -> bool {
    let byte_index = file_index / 8;
    let bit_offset = file_index % 8;

    if byte_index >= tag.bit_mask.len() {
        return false;
    }

    // Little-endian bit ordering: bit 0 = LSB
    let has_tag = (tag.bit_mask[byte_index] & (1 << bit_offset)) != 0;
    has_tag && selected
}
```

## Installation Planning

### Size Calculation

Calculate installation size for selected tags:

```rust
fn calculate_install_size(
    entries: &[InstallFileEntry],
    selected_tags: u16
) -> u64 {
    entries.iter()
        .filter(|e| should_install(e, selected_tags))
        .map(|e| e.file_size as u64)
        .sum()
}
```

### Path Resolution

Convert relative paths to absolute:

```rust
fn resolve_install_path(
    base_dir: &Path,
    entry: &InstallFileEntry
) -> PathBuf {
    let relative_path = std::str::from_utf8(&entry.path).unwrap();
    base_dir.join(relative_path)
}
```

## File Categories

### Essential Files

Files with tag mask `0x0000` or `0xFFFF`:

- Core executables

- Essential libraries

- Base configuration

- Critical game data

### Optional Content

Files with specific tag requirements:

- High-resolution textures (HighResTextures tag)

- Cinematics (Cinematics tag)

- Additional languages (locale tags)

- Developer tools (DevTools tag)

## Implementation Example

```rust
struct InstallFile {
    header: InstallHeader,
    tags: Vec<InstallTag>,
    entries: Vec<InstallFileEntry>,
}

impl InstallFile {
    pub fn get_install_list(&self, tags: &[String]) -> Vec<InstallItem> {
        let tag_mask = self.build_tag_mask(tags);

        self.entries.iter()
            .filter(|e| should_install(e, tag_mask))
            .map(|e| InstallItem {
                content_key: e.content_key,
                install_path: String::from_utf8_lossy(&e.path).to_string(),
                file_size: e.file_size,
            })
            .collect()
    }

    fn build_tag_mask(&self, tag_names: &[String]) -> u16 {
        let mut mask = 0u16;

        for name in tag_names {
            if let Some(tag) = self.tags.iter().find(|t| t.name == name) {
                mask |= 1 << tag.id;
            }
        }

        mask
    }
}
```

## Selective Installation

### Platform-Specific

Install only files for current platform:

```rust
fn get_platform_tags() -> Vec<String> {
    let mut tags = vec!["Base".to_string()];

    #[cfg(target_os = "windows")]
    tags.push("Windows".to_string());

    #[cfg(target_arch = "x86_64")]
    tags.push("x64".to_string());

    tags
}
```

### Language Selection

Install specific language assets:

```rust
fn get_locale_tags(selected_locale: &str) -> Vec<String> {
    vec![
        "Base".to_string(),
        selected_locale.to_string(),
    ]
}
```

## Optimization Strategies

### Parallel Installation

Install multiple files concurrently:

```rust
use rayon::prelude::*;

fn install_files(items: Vec<InstallItem>) {
    items.par_iter()
        .for_each(|item| {
            download_and_install(item);
        });
}
```

### Incremental Updates

Track installed files for patching:

```rust
struct InstalledFiles {
    entries: HashMap<PathBuf, InstalledFileInfo>,
}

struct InstalledFileInfo {
    content_key: [u8; 16],
    file_size: u32,
    modified_time: SystemTime,
}
```

## Validation

### Post-Installation Verification

```rust
fn verify_installation(
    install_dir: &Path,
    install_file: &InstallFile,
    selected_tags: u16
) -> Result<()> {
    for entry in &install_file.entries {
        if !should_install(entry, selected_tags) {
            continue;
        }

        let path = install_dir.join(&entry.path);

        // Verify file exists
        if !path.exists() {
            return Err("Missing file");
        }

        // Verify file size
        let metadata = fs::metadata(&path)?;
        if metadata.len() != entry.file_size as u64 {
            return Err("Size mismatch");
        }
    }

    Ok(())
}
```

## Repair Process

Detect and repair corrupted installations:

```rust
fn repair_installation(
    install_file: &InstallFile,
    install_dir: &Path
) -> Vec<RepairAction> {
    let mut actions = Vec::new();

    for entry in &install_file.entries {
        let path = install_dir.join(&entry.path);

        if !path.exists() {
            actions.push(RepairAction::Download(entry.content_key));
        } else if !verify_file(&path, entry) {
            actions.push(RepairAction::Redownload(entry.content_key));
        }
    }

    actions
}
```

## Common Issues

1. **Tag conflicts**: Multiple tags may include same file
2. **Path separators**: Handle platform-specific separators
3. **Case sensitivity**: File systems vary in case handling
4. **Symlink support**: Some platforms don't support symlinks
5. **Permission issues**: Installation may require elevation

## Special Considerations

### Shared Files

Files used by multiple products:

```rust
struct SharedFile {
    content_key: [u8; 16],
    products: Vec<String>,
    ref_count: u32,
}
```

### Uninstall Tracking

Track files for clean uninstall:

```rust
struct UninstallManifest {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    registry_keys: Vec<String>,  // Windows only
}
```

## Parser Implementation Status

### Python Parser (cascette-py)

**Status**: Complete

**Capabilities**:

- Version 1 header parsing with IN magic detection

- Tag extraction with proper little-endian bit ordering

- Platform/architecture/locale tag type classification

- File entry parsing with path, content key, and size

- Tag-to-file association via bitmask resolution

- BLTE decompression for compressed manifests

**Verified Against**:

- WoW 11.0.5.57689 (242 entries, 28 tags)

- Multiple WoW Classic builds

- Cross-platform tag validation (Windows, OSX, mobile)

**Known Issues**: None

See <https://github.com/wowemulation-dev/cascette-py> for the Python
implementation.

## Version History

The Install manifest format has two versions:

### Version 1

- **Header Size**: 10 bytes
- **Magic**: "IN" (0x494E)
- **Entry Size**: Derived as `ckey_length + 4`
- **Features**:
  - File path to content key mapping
  - Tag-based selective installation
  - Platform/architecture/locale filtering
  - Bit mask system for tag associations
  - Little-endian bit ordering in tag masks
  - Tag type classification (Platform, Architecture, Locale, Category, Alternate)

### Version 2

- **Header Size**: 16 bytes (10 base + 6 additional)
- **Added Fields**: `loose_file_type` (1 byte), `extra_entry_count` (4 bytes
  BE), `entry_size` (1 byte)
- **Features**: All version 1 features plus explicit entry size and support
  for loose file types

### Version Detection

The version field is at offset 2 in the header. The agent accepts versions
1 and 2 (validates non-zero and <= 2).

### Implementation Status

- **cascette-formats**: Full support for version 1 with validation
- **cascette-py**: Complete parsing for version 1 with tag extraction

## References

- See [Root File](root.md) for file catalog

- See [Download Manifest](download.md) for download prioritization

- See [Encoding Documentation](encoding.md) for content resolution

- See [Format Transitions](format-transitions.md) for format evolution tracking

## Binary Verification (Agent.exe, TACT 3.13.3)

Verified against Agent.exe (WoW Classic Era) using Binary Ninja on
2026-02-15. Install manifest parser source:
`d:\package_cache\tact\3.13.3\src\install_manifest\install_manifest_binary_reader.cpp`.

### Confirmed Correct

| Claim | Agent Evidence |
|-------|---------------|
| Magic: "IN" (0x494E) | Confirmed at `sub_6cf44d` (0x6cf491: cmp 0x4e49 LE) |
| Minimum header size 10 bytes | Confirmed (0x6cf460: size check >= 0xa) |
| Version at offset 2 | Confirmed |
| Hash size at offset 3 | Confirmed |
| Tag count BE16 at offset 4-5 | Confirmed (shift+OR pattern at 0x6cf4e1) |
| Entry count BE32 at offset 6-9 | Confirmed via `sub_6a2976` call |

### Changes Applied

1. Updated version field to accept 1 or 2
2. Added version 2 header fields (loose_file_type, extra_entry_count,
   entry_size)
3. Documented version 1 entry size derivation (ckey_length + 4)
4. Updated Version History to include version 2

### Source File

Agent source path:
`d:\package_cache\tact\3.13.3\src\install_manifest\install_manifest_binary_reader.cpp`
