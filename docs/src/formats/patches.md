# NGDP/TACT Patch System

The NGDP patch system enables incremental updates between game versions using
differential patches.

## Patch System Architecture

The patch system uses a multi-tier structure:

1. **Patch Manifests** (PA files in /patch/): Index files listing patches

   between builds

2. **Patch Archives** (ZBSDIFF files in /patch/): Actual differential patch data
3. **Intermediate Results** (in /data/): Results of applying patches in a chain

## Patch File Locations

According to wowdev.wiki, the directories are:

- `/config/`: Build configs, CDN configs, and Patch configs

- `/data/`: Archives, indexes, and unarchived files (binaries, media, root,

  install, download)

- `/patch/`: Patch manifests, patch files, patch archives, patch indexes

Specifically:

- **Patch Manifests**: `https://cdn.host/tpr/wow/patch/{hash[:2]}/{hash[2:4]}/{hash}`
  - PA (Patch Archive) format files containing patch entry indices
  - Referenced by `patch` field in build configs
- **Patch Archives**: `https://cdn.host/tpr/wow/patch/{hash[:2]}/{hash[2:4]}/{hash}`
  - ZBSDIFF1 format differential patch files stored in archives
  - Found in patch-entry lines (the patch_hash values)
  - Stored in archives just like regular data files
- **Patch Archive Indices**: `https://cdn.host/tpr/wow/patch/{hash[:2]}/{hash[2:4]}/{hash}.index`
  - Index files for patch archives using the same format as data archive indices
  - Map content hashes to locations within patch archives
  - Referenced by `patch-archives-index` field in CDN configs
  - Use IndexType::Patch (offset_bytes = 0) in the footer
- **Patch Results**: `https://cdn.host/tpr/wow/data/{hash[:2]}/{hash[2:4]}/{hash}`
  - Intermediate or final results of applying patches
  - BLTE-encoded files with DL/EN/IN signatures for manifest types
- **Patch Configurations**: `https://cdn.host/tpr/wow/config/{hash[:2]}/{hash[2:4]}/{hash}`
  - Text configs with patch-entry lines describing patch chains
  - Referenced by `patch-config` field in build configs

## Patch Manifest Format

Patch manifests use the PA (Patch Archive) format. All numeric fields are
big-endian throughout (header, block table, and block data).

### Header Structure (10 bytes)

```c
struct PatchArchiveHeader {  // 10 bytes, big-endian
    uint8_t  magic[2];         // "PA" (0x5041)
    uint8_t  version;          // Format version (1-2)
    uint8_t  file_key_size;    // Target file CKey size (1-16, typically 16)
    uint8_t  old_key_size;     // Source file EKey size (1-16, typically 16)
    uint8_t  patch_key_size;   // Patch EKey size (1-16, typically 16)
    uint8_t  block_size_bits;  // Block size as power of 2 (range [12, 24])
    uint16_t block_count;      // Number of blocks (big-endian)
    uint8_t  flags;            // Format flags (see below)
};
```

**Flags**:

- **Bit 0 (0x01)**: Plain data mode (informational, Agent.exe logs but
  does not reject)
- **Bit 1 (0x02)**: Extended header present with encoding info. All
  known CDN patch manifests have this flag set.

### Extended Header (when flags & 0x02)

Present immediately after the 10-byte header. Contains encoding file
metadata for the patch manifest:

```c
struct PatchArchiveEncodingInfo {
    uint8_t  encoding_ckey[file_key_size];  // Encoding file CKey
    uint8_t  encoding_ekey[file_key_size];  // Encoding file EKey
    uint32_t decoded_size;                  // Decoded size (big-endian)
    uint32_t encoded_size;                  // Encoded size (big-endian)
    uint8_t  espec_length;                  // Length of ESpec string
    uint8_t  espec[espec_length];           // ESpec (length-prefixed, NOT null-terminated)
};
```

### Block Table

Follows the header (or extended header if present). Each entry has a
fixed size of `file_key_size + 20` bytes:

```c
struct BlockTableEntry {  // file_key_size + 20 bytes per entry
    uint8_t  last_file_ckey[file_key_size];  // Last (highest) CKey in this block
    uint8_t  block_md5[16];                  // MD5 hash of block data
    uint32_t block_offset;                   // Absolute byte offset (big-endian)
};
```

The block table is sorted by `last_file_ckey`. Agent.exe validates sort
order using `_memcmp` during parsing.

### Block Data

At each `block_offset`, file entries are stored as variable-length records
terminated by a 0x00 sentinel byte:

```c
// Repeat until num_patches == 0:
struct FileEntry {
    uint8_t  num_patches;                    // 0 = end of block
    uint8_t  target_ckey[file_key_size];     // Target file CKey
    uint8_t  decoded_size[5];                // uint40, big-endian
    // Followed by num_patches patch records:
    struct {
        uint8_t  source_ekey[old_key_size];  // Source file EKey
        uint8_t  source_decoded_size[5];     // uint40, big-endian
        uint8_t  patch_ekey[patch_key_size]; // Patch data EKey
        uint32_t patch_size;                 // Patch data size (big-endian)
        uint8_t  patch_index;                // Ordering hint
    } patches[num_patches];
};
uint8_t end_marker = 0;  // Sentinel byte
```

Decoded sizes use uint40 (5-byte big-endian) to support files up to ~1 TB.

## Compression Info Format

The compression info string describes byte ranges and their compression:

- Format: `{offset=method,offset=method,...,*=default}`

- Methods: `n` (none), `z` (zlib)

- Example: `{22=n,10044521=z,734880=n,*=z}`

## Build Config References

Build configurations reference patches through:

- `patch`: Main patch manifest hash

- `patch-size`: Size of patch manifest

- `patch-index`: Patch index files

- `patch-config`: Patch configuration hash

## Patch Configuration

Patch configs contain `patch-entry` lines describing patch chains between file
versions.

### Patch Entry Format

```text
patch-entry = type old_hash old_size new_hash new_size compression_info
[result_hash result_size patch_hash patch_size]+
```

Components:

- `type`: Manifest type (download, encoding, install, size, vfs:, etc.)

- `old_hash`: MD5 of original file content

- `old_size`: Size of original file

- `new_hash`: MD5 of final patched content

- `new_size`: Size of final file

- `compression_info`: Compression specification (e.g.,
`b:{11=n,8183230=n,1255589=z}`)

- Followed by repeating groups of:
  - `result_hash`: MD5 of intermediate/final result (stored in /data/)
  - `result_size`: Size of result file
  - `patch_hash`: MD5 of ZBSDIFF patch file (stored in /patch/)
  - `patch_size`: Size of patch file

### Patch Chain Example

```text
patch-entry = download 6afd6862... 9438830 d29e5263... 8190785 b:{...} \
  557b46d1... 15384969 08c046c8... 1623773 \
  4ebf89a1... 15384925 e960d26b... 1623636
```

This describes a chain:

1. Apply patch `08c046c8` to original `6afd6862` → result `557b46d1`
2. Apply patch `e960d26b` to result `557b46d1` → result `4ebf89a1`
3. Continue until reaching final `d29e5263`

## ZBSDIFF1 Format (Zlib-compressed Binary Differential)

ZBSDIFF1 is the binary differential patch format used by NGDP/TACT for
efficient file updates:

### Header (32 bytes, little-endian)

```c
struct ZbsdiffHeader {
    uint8_t  signature[8];       // "ZBSDIFF1"
    int64_t  control_size;       // Size of compressed control block (little-endian)
    int64_t  diff_size;          // Size of compressed diff block (little-endian)
    int64_t  output_size;        // Size of final output file (little-endian)
};
```

### Three-Block Structure

1. **Control Block** (zlib-compressed):
   - Triple sequences: (diff_size, extra_size, seek_offset)
   - Instructions for applying differences and inserting new data
   - All values are signed 64-bit integers

2. **Diff Block** (zlib-compressed):
   - Byte differences to apply to old data
   - Applied by XOR operation: new[i] = old[i] + diff[i]

3. **Extra Block** (zlib-compressed):
   - New data to insert at specified positions
   - Copied directly to output

### Streaming Application

ZBSDIFF1 supports streaming application without loading entire files:

```rust
// Streaming patch application
let mut old_pos = 0;
let mut new_pos = 0;
let mut control_entries = decompress_control_block(&patch.control_data)?;

while let Some((diff_size, extra_size, seek_offset)) = control_entries.next()? {
    // Copy diff_size bytes with differences
    copy_with_diff(&old_data[old_pos..], &diff_data, &mut new_data[new_pos..], diff_size);
    old_pos += diff_size;
    new_pos += diff_size;

    // Copy extra_size bytes of new data
    copy_extra(&extra_data, &mut new_data[new_pos..], extra_size);
    new_pos += extra_size;

    // Seek in old data
    old_pos += seek_offset;
}
```

### Format Characteristics

- **Little-Endian Header**: All header fields use little-endian byte order
  (verified against Agent.exe `tact::BsPatch::ParseHeader` at 0x6fbd1c)

- **Signed Integers**: Control block uses signed 64-bit little-endian integers
  for sizes and offsets

- **Zlib Compression**: All data blocks compressed independently

- **Memory Efficient**: Can process large files with minimal RAM usage

- **Error Detection**: Header validation and decompression errors detected

## Patch Archive Storage

Patch data is stored in archives just like regular game data:

1. **Patch Archives**: Large files containing multiple patch data blobs
   - Located in `/patch/` directory on CDN
   - Contain BLTE-encoded ZBSDIFF1 patches
   - Named with content hashes like regular archives

2. **Patch Archive Indices**: Map patch hashes to archive locations
   - Use the same `.index` format as data archives
   - Footer uses IndexType::Patch (offset_bytes = 0)
   - Allow CDN to locate specific patches within archives

3. **Patch Archive Groups**: Client-side optimization structures
   - Use the same Archive Group format as data archives
   - Group related patches for efficient client caching
   - Located in client's local CASC storage (not on CDN)
   - Referenced in `.idx` files with grouped archive information

4. **CDN Config References**:
   - `patch-archives`: List of patch archive hashes
   - `patch-archives-index`: Corresponding index file hashes
   - `patch-archives-index-size`: Size of each index file

This completely mirrors the structure used for data archives:

- `archives` → `patch-archives`

- `archives-index` → `patch-archives-index`

- Archive Groups → Patch Archive Groups

- Same formats, just in `/patch/` directory instead of `/data/`

## Patch Chain Building and Validation

### Patch Chain Construction

Patches can form chains from one content version to another with cycle
detection:

```rust
pub fn build_patch_chain(
    &self,
    start_key: &[u8; 16],
    end_key: &[u8; 16]
) -> Option<PatchChain> {
    let mut chain = Vec::new();
    let mut current_key = *start_key;
    let mut visited = HashSet::new();

    while current_key != *end_key {
        // Cycle detection
        if visited.contains(&current_key) {
            return None; // Cycle detected
        }
        visited.insert(current_key);

        let patch_entry = self.find_patch_for_content(&current_key)?;
        current_key = patch_entry.new_content_key;
        chain.push(patch_entry.clone());

        // Safety limit: prevent infinite chains
        if chain.len() > 10 {
            return None; // Chain too long
        }
    }

    Some(PatchChain { steps: chain, start_key: *start_key, end_key: *end_key })
}
```

### Safety Validations

- **Cycle Detection**: Prevents infinite loops in patch chains

- **Chain Length Limits**: Maximum 10 steps to prevent excessive processing

- **Size Validation**: Output size must match header specification

- **Checksum Verification**: Content keys validated after patch application

- **Stream Bounds Checking**: Prevents buffer overflows during streaming

### Size Limits and Memory Management

```rust
// ZBSDIFF1 size limits for safety
const MAX_PATCH_SIZE: usize = 100 * 1024 * 1024; // 100MB max patch
const MAX_OUTPUT_SIZE: usize = 1024 * 1024 * 1024; // 1GB max output
const MAX_CONTROL_ENTRIES: usize = 1_000_000; // Prevent memory exhaustion

impl ZbsdiffHeader {
    pub fn validate(&self) -> Result<(), ZbsdiffError> {
        if self.output_size > MAX_OUTPUT_SIZE as u64 {
            return Err(ZbsdiffError::OutputTooLarge(self.output_size));
        }

        if self.control_size + self.diff_size > MAX_PATCH_SIZE as u64 {
            return Err(ZbsdiffError::PatchTooLarge);
        }

        Ok(())
    }
}
```

## Patch Application Process

1. Fetch patch manifest from CDN using patch hash from build config
2. Parse manifest to find patch entry for target file
3. **Validate patch chain**: Check for cycles and reasonable length
4. Look up patch in patch archive index to find archive and offset
5. Download patch data from archive using index information
6. **Validate patch size limits** before processing
7. Decode BLTE wrapper and extract ZBSDIFF1 patch
8. Apply patch using streaming algorithm with bounds checking
9. **Verify result size and hash** match expectations

## Implementation Notes

- Patches are not BLTE-encoded at the manifest level

- Individual patch data files may be BLTE-encoded

- Block size is typically 64KB (2^16 bytes)

- Version 2 is the current patch format version

- Patches enable efficient updates without re-downloading entire files
