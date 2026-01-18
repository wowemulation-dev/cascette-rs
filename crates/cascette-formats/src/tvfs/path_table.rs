//! TVFS path table structures and parsing

use binrw::io::{Read, Seek, Write};
use binrw::{BinRead, BinResult, BinWrite};
use std::collections::HashMap;

use crate::tvfs::error::{TvfsError, TvfsResult};
use crate::tvfs::utils::{read_varint, write_varint};

/// Path table with hierarchical prefix tree structure
#[derive(Debug, Clone)]
pub struct PathTable {
    /// Root node of the path tree
    pub root_node: PathNode,
    /// Flattened nodes for fast access by index
    pub nodes: Vec<PathNode>,
    /// Node index mapping for quick lookups
    pub node_index: HashMap<u32, usize>,
}

/// Individual path node in the prefix tree
#[derive(Debug, Clone)]
pub struct PathNode {
    /// Path component (file/directory name)
    pub path_part: String,
    /// Whether this node represents a directory
    pub is_directory: bool,
    /// Child node indices
    pub children: Vec<u32>,
    /// File ID reference for files (index into VFS table)
    pub file_id: Option<u32>,
    /// Node index for internal reference
    pub node_index: u32,
}

// Path node flags for binary format
const PATH_FLAG_DIRECTORY: u8 = 0x80;
const PATH_FLAG_HAS_CHILDREN: u8 = 0x40;
const PATH_FLAG_HAS_FILE_ID: u8 = 0x20;

impl BinRead for PathTable {
    type Args<'a> = (u32,); // table_size

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let table_size = args.0 as usize;
        let mut table_data = vec![0u8; table_size];
        reader.read_exact(&mut table_data)?;

        // Parse prefix tree from binary data
        let mut nodes = Vec::new();
        let mut node_index = HashMap::new();
        let mut offset = 0;

        // Read all nodes
        let mut current_index = 0u32;
        while offset < table_data.len() {
            let mut node =
                parse_path_node(&table_data, &mut offset, current_index).map_err(|e| {
                    binrw::Error::Custom {
                        pos: offset as u64,
                        err: Box::new(e),
                    }
                })?;

            node.node_index = current_index;
            node_index.insert(current_index, nodes.len());
            nodes.push(node);
            current_index += 1;
        }

        // The first node should be the root
        let root_node = nodes
            .first()
            .ok_or_else(|| binrw::Error::Custom {
                pos: 0,
                err: Box::new(TvfsError::EmptyPathTable),
            })?
            .clone();

        Ok(PathTable {
            root_node,
            nodes,
            node_index,
        })
    }
}

impl BinWrite for PathTable {
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> BinResult<()> {
        let mut data = Vec::new();

        // Write all nodes
        for node in &self.nodes {
            write_path_node(node, &mut data);
        }

        writer.write_all(&data)?;
        Ok(())
    }
}

impl PathTable {
    /// Get node by index
    pub fn get_node(&self, index: u32) -> Option<&PathNode> {
        self.node_index.get(&index).and_then(|&i| self.nodes.get(i))
    }

    /// Get mutable node by index
    pub fn get_node_mut(&mut self, index: u32) -> Option<&mut PathNode> {
        if let Some(&i) = self.node_index.get(&index) {
            self.nodes.get_mut(i)
        } else {
            None
        }
    }

    /// Add a new node to the table
    pub fn add_node(&mut self, node: PathNode) -> u32 {
        let index = self.nodes.len() as u32;
        self.node_index.insert(index, self.nodes.len());
        self.nodes.push(node);
        index
    }

    /// Calculate maximum depth of the path tree
    pub fn calculate_max_depth(&self) -> u16 {
        self.calculate_node_depth(&self.root_node, 0)
    }

    fn calculate_node_depth(&self, node: &PathNode, current_depth: u16) -> u16 {
        let mut max_depth = current_depth;

        for &child_id in &node.children {
            if let Some(child) = self.get_node(child_id) {
                let child_depth = self.calculate_node_depth(child, current_depth + 1);
                max_depth = max_depth.max(child_depth);
            }
        }

        max_depth
    }
}

impl PathNode {
    /// Create a new path node
    pub fn new(path_part: String, is_directory: bool) -> Self {
        Self {
            path_part,
            is_directory,
            children: Vec::new(),
            file_id: None,
            node_index: 0,
        }
    }

    /// Create a root node
    pub fn root() -> Self {
        Self {
            path_part: String::new(),
            is_directory: true,
            children: Vec::new(),
            file_id: None,
            node_index: 0,
        }
    }

    /// Add a child node index
    pub fn add_child(&mut self, child_index: u32) {
        if !self.children.contains(&child_index) {
            self.children.push(child_index);
        }
    }
}

/// Parse a single path node from binary data
fn parse_path_node(data: &[u8], offset: &mut usize, _node_index: u32) -> TvfsResult<PathNode> {
    if *offset >= data.len() {
        return Err(TvfsError::PathTableTruncated(*offset));
    }

    let flags = data[*offset];
    *offset += 1;

    let is_directory = (flags & PATH_FLAG_DIRECTORY) != 0;
    let has_children = (flags & PATH_FLAG_HAS_CHILDREN) != 0;
    let has_file_id = (flags & PATH_FLAG_HAS_FILE_ID) != 0;

    // Read path component length
    if *offset >= data.len() {
        return Err(TvfsError::PathTableTruncated(*offset));
    }

    let path_len = data[*offset] as usize;
    *offset += 1;

    // Read path component
    if *offset + path_len > data.len() {
        return Err(TvfsError::PathTableTruncated(*offset));
    }

    let path_bytes = &data[*offset..*offset + path_len];
    let path_part = String::from_utf8(path_bytes.to_vec())
        .map_err(|e| TvfsError::InvalidPathNode(*offset, format!("Invalid UTF-8: {}", e)))?;
    *offset += path_len;

    // Read children if present
    let mut children = Vec::new();
    if has_children {
        if *offset >= data.len() {
            return Err(TvfsError::PathTableTruncated(*offset));
        }

        let child_count = data[*offset] as usize;
        *offset += 1;

        for _ in 0..child_count {
            let child_id = read_varint(data, offset)?;
            children.push(child_id);
        }
    }

    // Read file ID if present
    let file_id = if has_file_id {
        Some(read_varint(data, offset)?)
    } else {
        None
    };

    Ok(PathNode {
        path_part,
        is_directory,
        children,
        file_id,
        node_index: 0, // Will be set by caller
    })
}

/// Write a path node to binary data
fn write_path_node(node: &PathNode, data: &mut Vec<u8>) {
    let mut flags = 0u8;

    if node.is_directory {
        flags |= PATH_FLAG_DIRECTORY;
    }
    if !node.children.is_empty() {
        flags |= PATH_FLAG_HAS_CHILDREN;
    }
    if node.file_id.is_some() {
        flags |= PATH_FLAG_HAS_FILE_ID;
    }

    data.push(flags);

    // Write path component
    let path_bytes = node.path_part.as_bytes();
    data.push(path_bytes.len() as u8);
    data.extend_from_slice(path_bytes);

    // Write children
    if !node.children.is_empty() {
        data.push(node.children.len() as u8);
        for &child_id in &node.children {
            write_varint(child_id, data);
        }
    }

    // Write file ID
    if let Some(file_id) = node.file_id {
        write_varint(file_id, data);
    }
}
