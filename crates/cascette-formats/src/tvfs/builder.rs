//! TVFS builder for creating TVFS files

use std::collections::HashMap;

use crate::tvfs::{
    TvfsFile,
    container_table::{ContainerEntry, ContainerFileTable},
    error::TvfsResult,
    header::{TVFS_FLAG_INCLUDE_CKEY, TvfsHeader},
    path_table::{PathNode, PathTable},
    vfs_table::{VfsEntry, VfsTable},
};

/// Builder for creating TVFS files
#[derive(Debug)]
pub struct TvfsBuilder {
    /// Files to include in the TVFS
    files: Vec<(String, ContainerEntry)>,
    /// Format flags
    flags: u32,
    /// Maximum depth tracking
    max_depth: u16,
}

impl TvfsBuilder {
    /// Create a new TVFS builder
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            flags: TVFS_FLAG_INCLUDE_CKEY,
            max_depth: 0,
        }
    }

    /// Create a new TVFS builder with custom flags
    pub fn with_flags(flags: u32) -> Self {
        Self {
            files: Vec::new(),
            flags,
            max_depth: 0,
        }
    }

    /// Add a file to the TVFS
    pub fn add_file(
        &mut self,
        path: String,
        ekey: [u8; 9],
        file_size: u32,
        content_key: Option<[u8; 16]>,
    ) {
        let compressed_size = if (self.flags & TVFS_FLAG_INCLUDE_CKEY) != 0 {
            Some(file_size) // Default to same size
        } else {
            None
        };

        let entry = ContainerEntry::new(ekey, file_size, compressed_size, content_key);
        // Update max depth before moving path
        let depth = path.split('/').filter(|s| !s.is_empty()).count() as u16;

        self.files.push((path, entry));
        self.max_depth = self.max_depth.max(depth);
    }

    /// Build the TVFS file
    pub fn build(&mut self) -> TvfsResult<Vec<u8>> {
        // Sort files for optimal path table generation
        self.files.sort_by(|a, b| a.0.cmp(&b.0));

        // Build path table from file list
        let path_table = self.build_path_table()?;

        // Build VFS table
        let vfs_table = self.build_vfs_table()?;

        // Build container table
        let container_table = self.build_container_table();

        // Create complete TVFS structure
        let tvfs = TvfsFile {
            header: self.create_header(&path_table, &vfs_table, &container_table),
            path_table,
            vfs_table,
            container_table,
        };

        // Serialize to bytes
        tvfs.build()
    }

    /// Build path table from file list
    fn build_path_table(&self) -> TvfsResult<PathTable> {
        let root = PathNode::root();
        let mut nodes = vec![root.clone()];
        let mut node_index_map = HashMap::new();
        node_index_map.insert(String::new(), 0u32);

        let mut next_file_id = 0u32;

        // Process each file
        for (file_index, (path, _)) in self.files.iter().enumerate() {
            self.insert_path_into_table(
                path,
                &mut nodes,
                &mut node_index_map,
                file_index as u32,
                &mut next_file_id,
            )?;
        }

        // Root node is already updated in nodes[0] during insertion

        // Create path table
        let mut path_table = PathTable {
            root_node: nodes[0].clone(),
            nodes,
            node_index: HashMap::new(),
        };

        // Build node index mapping
        for (i, _node) in path_table.nodes.iter().enumerate() {
            path_table.node_index.insert(i as u32, i);
        }

        Ok(path_table)
    }

    /// Insert a path into the path table
    fn insert_path_into_table(
        &self,
        path: &str,
        nodes: &mut Vec<PathNode>,
        node_index_map: &mut HashMap<String, u32>,
        _file_index: u32,
        next_file_id: &mut u32,
    ) -> TvfsResult<()> {
        let components: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if components.is_empty() {
            return Ok(());
        }

        let mut current_path = String::new();
        let mut _current_node_index = 0u32; // Root node

        // Navigate/create path components
        for (i, component) in components.iter().enumerate() {
            let parent_path = current_path.clone();
            current_path = if current_path.is_empty() {
                (*component).to_string()
            } else {
                format!("{}/{}", current_path, component)
            };

            let is_file = i == components.len() - 1;

            // Check if node already exists
            if let Some(&existing_index) = node_index_map.get(&current_path) {
                _current_node_index = existing_index;

                // If it's a file, set the file ID
                if is_file {
                    if let Some(node) = nodes.get_mut(existing_index as usize) {
                        node.file_id = Some(*next_file_id);
                        *next_file_id += 1;
                    }
                }
            } else {
                // Create new node
                let mut new_node = PathNode::new((*component).to_string(), !is_file);
                new_node.node_index = nodes.len() as u32;

                if is_file {
                    new_node.file_id = Some(*next_file_id);
                    *next_file_id += 1;
                }

                let new_index = nodes.len() as u32;
                nodes.push(new_node);
                node_index_map.insert(current_path.clone(), new_index);

                // Add child reference to parent
                if let Some(parent_index) = node_index_map.get(&parent_path) {
                    if let Some(parent_node) = nodes.get_mut(*parent_index as usize) {
                        parent_node.add_child(new_index);
                    }
                }

                _current_node_index = new_index;
            }
        }

        Ok(())
    }

    /// Build VFS table
    fn build_vfs_table(&self) -> TvfsResult<VfsTable> {
        let mut vfs_table = VfsTable::new();

        for (file_index, (_path, _entry)) in self.files.iter().enumerate() {
            let vfs_entry = VfsEntry::new(
                file_index as u32, // file_id
                file_index as u32, // container_index
                0,                 // file_offset (not used in TVFS)
                _entry.file_size,  // file_size
                false,             // is_directory
                false,             // is_compressed
            );

            vfs_table.add_entry(vfs_entry);
        }

        Ok(vfs_table)
    }

    /// Build container table
    fn build_container_table(&self) -> ContainerFileTable {
        let mut container_table = ContainerFileTable::new();

        for (_path, entry) in &self.files {
            container_table.add_entry(entry.clone());
        }

        container_table
    }

    /// Create header with proper offsets and sizes
    fn create_header(
        &self,
        path_table: &PathTable,
        vfs_table: &VfsTable,
        container_table: &ContainerFileTable,
    ) -> TvfsHeader {
        let mut header = TvfsHeader::new(self.flags);

        // Calculate table sizes
        let path_table_size = self.calculate_path_table_size(path_table);
        let vfs_table_size = vfs_table.table_size();
        let container_table_size = container_table.calculate_size(header.includes_content_keys());

        // Calculate offsets
        let header_size = u32::from(header.header_size);
        let path_table_offset = header_size;
        let vfs_table_offset = path_table_offset + path_table_size;
        let cft_table_offset = vfs_table_offset + vfs_table_size;

        header.update_table_info(
            path_table_offset,
            path_table_size,
            vfs_table_offset,
            vfs_table_size,
            cft_table_offset,
            container_table_size,
            self.max_depth,
        );

        header
    }

    /// Calculate path table size (approximation)
    fn calculate_path_table_size(&self, path_table: &PathTable) -> u32 {
        let mut size = 0usize;

        for node in &path_table.nodes {
            size += 1; // flags
            size += 1; // path length
            size += node.path_part.len(); // path data

            if !node.children.is_empty() {
                size += 1; // child count
                size += node.children.len() * 5; // Approximate varint size
            }

            if node.file_id.is_some() {
                size += 5; // Approximate varint size
            }
        }

        size as u32
    }
}

impl Default for TvfsBuilder {
    fn default() -> Self {
        Self::new()
    }
}
