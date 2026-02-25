//! TVFS path table structures and parsing
//!
//! The path table is a serialized prefix tree (trie) encoding file paths.
//! Format follows CascLib's `CapturePathEntry` / `ParsePathFileTable`:
//!
//! - `0x00` byte = path separator (before or after a name fragment)
//! - Length byte + N name bytes = name fragment
//! - `0xFF` + 4-byte BE NodeValue = node value marker
//!   - Bit 31 set: folder node, lower 31 bits = folder data length (includes the 4-byte NodeValue)
//!   - Bit 31 clear: file node, value = byte offset into VFS table

use crate::tvfs::error::{TvfsError, TvfsResult};

/// Folder flag in NodeValue (bit 31).
const TVFS_FOLDER_NODE: u32 = 0x8000_0000;
/// Mask for folder data length (lower 31 bits).
const TVFS_FOLDER_SIZE_MASK: u32 = 0x7FFF_FFFF;
/// Node value marker byte.
const NODE_VALUE_MARKER: u8 = 0xFF;
/// Path separator byte.
const PATH_SEPARATOR: u8 = 0x00;

/// Path table storing the recursive prefix tree and resolved file entries.
#[derive(Debug, Clone)]
pub struct PathTable {
    /// Raw table bytes for round-trip fidelity.
    pub data: Vec<u8>,
    /// Flattened list of resolved file entries (path → VFS offset).
    pub files: Vec<PathFileEntry>,
    /// Tree structure for enumeration and path resolution.
    pub root: PathTreeNode,
}

/// A resolved file entry from the path table.
#[derive(Debug, Clone)]
pub struct PathFileEntry {
    /// Full path (components joined by `/`)
    pub path: String,
    /// Byte offset into the VFS table
    pub vfs_offset: u32,
}

/// A node in the reconstructed path tree (for enumeration).
#[derive(Debug, Clone)]
pub struct PathTreeNode {
    /// Name fragment for this node
    pub name: String,
    /// Children (subdirectories and files)
    pub children: Vec<PathTreeNode>,
    /// VFS offset if this is a file node
    pub vfs_offset: Option<u32>,
}

impl PathTable {
    /// Parse the path table from raw bytes.
    pub fn parse(data: &[u8]) -> TvfsResult<Self> {
        let mut files = Vec::new();
        let mut root = PathTreeNode {
            name: String::new(),
            children: Vec::new(),
            vfs_offset: None,
        };

        // The path table typically starts with a root folder: 0xFF + NodeValue(bit31 set)
        // But we parse generically by calling parse_directory on the full range.
        parse_directory(
            data,
            0,
            data.len(),
            &mut String::new(),
            &mut files,
            &mut root,
        )?;

        Ok(Self {
            data: data.to_vec(),
            files,
            root,
        })
    }

    /// Get the number of file entries (for compatibility with tests checking `nodes.len()`).
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Resolve a path to a VFS offset.
    pub fn resolve_path(&self, path: &str) -> Option<u32> {
        self.files
            .iter()
            .find(|f| f.path == path)
            .map(|f| f.vfs_offset)
    }

    /// Build path table bytes from a tree structure.
    ///
    /// This produces the binary prefix tree format that CascLib expects.
    pub fn build(root: &PathTreeNode) -> Vec<u8> {
        let mut data = Vec::new();
        build_directory(&mut data, root);
        data
    }
}

/// Parse a directory range within the path table.
///
/// `start..end` is the byte range of directory contents (after the folder NodeValue).
fn parse_directory(
    data: &[u8],
    start: usize,
    end: usize,
    current_path: &mut String,
    files: &mut Vec<PathFileEntry>,
    tree_node: &mut PathTreeNode,
) -> TvfsResult<()> {
    let mut pos = start;

    while pos < end {
        // Parse one path entry
        let mut name_parts: Vec<Vec<u8>> = Vec::new();

        // Skip leading path separator
        if pos < end && data[pos] == PATH_SEPARATOR {
            pos += 1;
        }

        // Parse name fragments until we hit a NodeValue marker
        loop {
            if pos >= end {
                return Err(TvfsError::PathTableTruncated(pos));
            }

            if data[pos] == NODE_VALUE_MARKER {
                // NodeValue follows
                break;
            }

            // Read name fragment: length byte + name bytes
            let name_len = data[pos] as usize;
            pos += 1;

            if pos + name_len > end {
                return Err(TvfsError::PathTableTruncated(pos));
            }

            name_parts.push(data[pos..pos + name_len].to_vec());
            pos += name_len;

            // Check what follows the name fragment
            if pos >= end {
                return Err(TvfsError::PathTableTruncated(pos));
            }

            if data[pos] == NODE_VALUE_MARKER {
                // NodeValue next
                break;
            }

            if data[pos] == PATH_SEPARATOR {
                // Explicit path separator after name, before next fragment or NodeValue
                pos += 1;

                if pos < end && data[pos] == NODE_VALUE_MARKER {
                    // Separator then NodeValue
                    break;
                }
                // Otherwise continue reading next name fragment
            }
            // If next byte is non-zero and not 0xFF, it's the start of a new name fragment
            // (implicit separator per CascLib)
        }

        // Now read the NodeValue
        if pos >= end || data[pos] != NODE_VALUE_MARKER {
            return Err(TvfsError::InvalidPathNode(
                pos,
                "expected 0xFF node value marker".to_string(),
            ));
        }
        pos += 1; // skip 0xFF

        if pos + 4 > end {
            return Err(TvfsError::PathTableTruncated(pos));
        }
        let node_value =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;

        // Build the name from fragments
        let name = build_name_from_parts(&name_parts);

        // Build the full path
        let full_path = if current_path.is_empty() {
            name.clone()
        } else if name.is_empty() {
            current_path.clone()
        } else {
            format!("{}/{}", current_path, name)
        };

        if (node_value & TVFS_FOLDER_NODE) != 0 {
            // Folder node: children are inline
            let folder_data_len = (node_value & TVFS_FOLDER_SIZE_MASK) as usize;

            // folder_data_len includes the 4-byte NodeValue itself
            if folder_data_len < 4 {
                return Err(TvfsError::InvalidPathNode(
                    pos - 4,
                    format!("folder data length too small: {folder_data_len}"),
                ));
            }
            let children_len = folder_data_len - 4;
            let children_start = pos;
            let children_end = pos + children_len;

            if children_end > end {
                return Err(TvfsError::PathTableTruncated(pos));
            }

            let mut child_tree = PathTreeNode {
                name: name.clone(),
                children: Vec::new(),
                vfs_offset: None,
            };

            parse_directory(
                data,
                children_start,
                children_end,
                &mut full_path.clone(),
                files,
                &mut child_tree,
            )?;

            tree_node.children.push(child_tree);
            pos = children_end;
        } else {
            // File node: NodeValue is VFS offset
            files.push(PathFileEntry {
                path: full_path.clone(),
                vfs_offset: node_value,
            });

            tree_node.children.push(PathTreeNode {
                name,
                children: Vec::new(),
                vfs_offset: Some(node_value),
            });
        }
    }

    Ok(())
}

/// Build a name string from name fragments. Fragments within one entry
/// are concatenated (they form one path component split for trie sharing).
fn build_name_from_parts(parts: &[Vec<u8>]) -> String {
    let total_len: usize = parts.iter().map(|p| p.len()).sum();
    let mut bytes = Vec::with_capacity(total_len);
    for part in parts {
        bytes.extend_from_slice(part);
    }
    // Path components may not be valid UTF-8 (e.g. Asian locale filenames).
    // Use lossy conversion for display purposes.
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Build a directory's binary representation into the output buffer.
fn build_directory(out: &mut Vec<u8>, node: &PathTreeNode) {
    for child in &node.children {
        build_entry(out, child);
    }
}

/// Build a single path entry (file or folder) into the output buffer.
fn build_entry(out: &mut Vec<u8>, node: &PathTreeNode) {
    let name_bytes = node.name.as_bytes();

    // Write path separator before name if this is a top-level entry
    // (We always write the separator for consistency)
    if !name_bytes.is_empty() {
        out.push(PATH_SEPARATOR);

        // Write name fragment: length + bytes
        // For names longer than 255 bytes, we'd need to split into fragments.
        // In practice TVFS names are short.
        if name_bytes.len() <= 255 {
            out.push(name_bytes.len() as u8);
            out.extend_from_slice(name_bytes);
        } else {
            // Split into 255-byte chunks
            for chunk in name_bytes.chunks(255) {
                out.push(chunk.len() as u8);
                out.extend_from_slice(chunk);
            }
        }
    }

    // Write NodeValue
    out.push(NODE_VALUE_MARKER);

    if node.vfs_offset.is_some() {
        // File node
        let vfs_offset = node.vfs_offset.unwrap_or(0);
        out.extend_from_slice(&vfs_offset.to_be_bytes());
    } else {
        // Folder node — we need to compute the children data first to know the length
        let mut children_data = Vec::new();
        build_directory(&mut children_data, node);

        // folder_data_len includes the 4-byte NodeValue
        let folder_data_len = (children_data.len() + 4) as u32;
        let node_value = TVFS_FOLDER_NODE | (folder_data_len & TVFS_FOLDER_SIZE_MASK);
        out.extend_from_slice(&node_value.to_be_bytes());
        out.extend_from_slice(&children_data);
    }
}
