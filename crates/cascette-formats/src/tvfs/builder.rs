//! TVFS builder for creating TVFS files
//!
//! Builds a TVFS manifest from a list of files. Produces the binary prefix
//! tree path table, span-based VFS table, and fixed-stride CFT that CascLib
//! and Agent.exe expect.

use crate::tvfs::{
    TvfsFile,
    container_table::{ContainerEntry, ContainerFileTable},
    error::TvfsResult,
    est_table::EstTable,
    header::{TVFS_FLAG_ENCODING_SPEC, TVFS_FLAG_INCLUDE_CKEY, TvfsHeader},
    path_table::{PathTable, PathTreeNode},
    vfs_table::{VfsEntry, VfsSpan, VfsTable},
};

/// Builder for creating TVFS files.
#[derive(Debug)]
pub struct TvfsBuilder {
    /// Files to include: (path, ekey, encoded_size, content_key)
    files: Vec<FileRecord>,
    /// Format flags
    flags: u32,
    /// Encoding spec strings (if TVFS_FLAG_ENCODING_SPEC is set)
    est_specs: Vec<String>,
}

#[derive(Debug, Clone)]
struct FileRecord {
    path: String,
    ekey: Vec<u8>,
    encoded_size: u32,
    content_size: u32,
    content_key: Option<Vec<u8>>,
    est_index: Option<u32>,
}

impl TvfsBuilder {
    /// Create a new TVFS builder with default flags (INCLUDE_CKEY only).
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            flags: TVFS_FLAG_INCLUDE_CKEY,
            est_specs: Vec::new(),
        }
    }

    /// Create a new TVFS builder with custom flags.
    pub fn with_flags(flags: u32) -> Self {
        Self {
            files: Vec::new(),
            flags,
            est_specs: Vec::new(),
        }
    }

    /// Add an encoding spec string (only used when TVFS_FLAG_ENCODING_SPEC is set).
    pub fn add_est_spec(&mut self, spec: String) {
        self.est_specs.push(spec);
    }

    /// Add a file to the TVFS.
    pub fn add_file(
        &mut self,
        path: String,
        ekey: [u8; 9],
        encoded_size: u32,
        content_size: u32,
        content_key: Option<[u8; 16]>,
    ) {
        self.files.push(FileRecord {
            path,
            ekey: ekey.to_vec(),
            encoded_size,
            content_size,
            content_key: content_key.map(|k| k.to_vec()),
            est_index: None,
        });
    }

    /// Add a file with an encoding spec index.
    pub fn add_file_with_est(
        &mut self,
        path: String,
        ekey: [u8; 9],
        encoded_size: u32,
        content_size: u32,
        content_key: Option<[u8; 16]>,
        est_index: u32,
    ) {
        self.files.push(FileRecord {
            path,
            ekey: ekey.to_vec(),
            encoded_size,
            content_size,
            content_key: content_key.map(|k| k.to_vec()),
            est_index: Some(est_index),
        });
    }

    /// Build the TVFS file and return serialized bytes.
    pub fn build(&mut self) -> TvfsResult<Vec<u8>> {
        // Sort files for deterministic output
        self.files.sort_by(|a, b| a.path.cmp(&b.path));

        // Create a temporary header to compute field sizes.
        // We need to know cft_table_size for CftOffsSize, but cft_table_size
        // depends on CftOffsSize (circular). Break the cycle by computing
        // the entry count first — entry_size only depends on flags and the
        // *range* of cft_table_size (1/2/3/4 byte threshold).
        let mut header = TvfsHeader::new(self.flags);

        // Build CFT entries and data
        let cft_entries: Vec<ContainerEntry> = self
            .files
            .iter()
            .map(|rec| ContainerEntry {
                offset: 0, // will be set below
                ekey: rec.ekey.clone(),
                encoded_size: rec.encoded_size,
                content_key: rec.content_key.clone(),
                est_index: rec.est_index,
                patch_offset: None,
            })
            .collect();

        // First pass: estimate CFT size with minimum offs sizes
        let est_entry_size_estimate = header.cft_entry_size();
        let cft_size_estimate = (cft_entries.len() * est_entry_size_estimate) as u32;
        header.cft_table_size = cft_size_estimate;

        // Now recompute with correct offs sizes
        let entry_size = header.cft_entry_size();
        let cft_size = (cft_entries.len() * entry_size) as u32;
        header.cft_table_size = cft_size;

        // Assign offsets to CFT entries
        let cft_entries: Vec<ContainerEntry> = cft_entries
            .into_iter()
            .enumerate()
            .map(|(i, mut e)| {
                e.offset = (i * entry_size) as u32;
                e
            })
            .collect();

        // Build VFS entries (one per file, single span each)
        let mut vfs_entries: Vec<VfsEntry> = Vec::with_capacity(self.files.len());
        let cft_offs_size = header.cft_offs_size() as usize;
        let span_wire_size = 1 + 4 + 4 + cft_offs_size; // span_count(1) + file_offset(4) + span_length(4) + cft_offset

        let mut vfs_offset = 0u32;
        for (i, rec) in self.files.iter().enumerate() {
            vfs_entries.push(VfsEntry {
                offset: vfs_offset,
                spans: vec![VfsSpan {
                    file_offset: 0,
                    span_length: rec.content_size,
                    cft_offset: cft_entries[i].offset,
                }],
            });
            vfs_offset += span_wire_size as u32;
        }

        // Build path tree
        let root = build_path_tree(&self.files, &vfs_entries);

        // Serialize path table
        let path_data = PathTable::build(&root);

        // Serialize VFS table
        let vfs_data = VfsTable::build(&vfs_entries, &header);

        // Serialize CFT
        let container_table = ContainerFileTable {
            data: Vec::new(), // will be built
            entries: cft_entries,
        };
        let cft_data = container_table.build(&header);

        // EST
        let est_table = if (self.flags & TVFS_FLAG_ENCODING_SPEC) != 0 && !self.est_specs.is_empty()
        {
            let mut est = EstTable::new();
            for spec in &self.est_specs {
                est.add_spec(spec.clone());
            }
            Some(est)
        } else {
            None
        };

        let est_data = est_table.as_ref().map(|est| {
            let mut buf = Vec::new();
            for spec in &est.specs {
                buf.extend_from_slice(spec.as_bytes());
                buf.push(0);
            }
            buf
        });

        // Compute table layout: header → path → est(opt) → cft → vfs
        let header_size = header.header_size as u32;
        let path_offset = header_size;
        let path_size = path_data.len() as u32;

        let (est_offset, est_size, cft_start);
        if let Some(ref est) = est_data {
            est_offset = path_offset + path_size;
            est_size = est.len() as u32;
            cft_start = est_offset + est_size;
            header.est_table_offset = Some(est_offset);
            header.est_table_size = Some(est_size);
        } else {
            est_offset = 0;
            est_size = 0;
            let _ = (est_offset, est_size);
            cft_start = path_offset + path_size;
        }

        let cft_actual_size = cft_data.len() as u32;
        let vfs_off = cft_start + cft_actual_size;
        let vfs_size = vfs_data.len() as u32;

        // Calculate max depth
        let max_depth = calculate_max_depth(&root, 0);

        header.path_table_offset = path_offset;
        header.path_table_size = path_size;
        header.vfs_table_offset = vfs_off;
        header.vfs_table_size = vfs_size;
        header.cft_table_offset = cft_start;
        header.cft_table_size = cft_actual_size;
        header.max_depth = max_depth;

        // Assemble the TvfsFile for serialization
        let tvfs = TvfsFile {
            header,
            path_table: PathTable {
                data: path_data.clone(),
                files: Vec::new(), // not needed for build output
                root,
            },
            vfs_table: VfsTable {
                data: vfs_data.clone(),
                entries: vfs_entries,
            },
            container_table: ContainerFileTable {
                data: cft_data.clone(),
                entries: container_table.entries,
            },
            est_table,
        };

        tvfs.build()
    }
}

impl Default for TvfsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a path tree from sorted file records and their VFS entries.
fn build_path_tree(files: &[FileRecord], vfs_entries: &[VfsEntry]) -> PathTreeNode {
    let mut root = PathTreeNode {
        name: String::new(),
        children: Vec::new(),
        vfs_offset: None,
    };

    for (i, rec) in files.iter().enumerate() {
        let components: Vec<&str> = rec.path.split('/').filter(|s| !s.is_empty()).collect();
        insert_path(&mut root, &components, 0, vfs_entries[i].offset);
    }

    root
}

/// Recursively insert a path into the tree.
fn insert_path(node: &mut PathTreeNode, components: &[&str], depth: usize, vfs_offset: u32) {
    if depth >= components.len() {
        return;
    }

    let component = components[depth];
    let is_leaf = depth == components.len() - 1;

    // Find existing child
    let child_pos = node.children.iter().position(|c| c.name == component);

    if let Some(pos) = child_pos {
        if is_leaf {
            node.children[pos].vfs_offset = Some(vfs_offset);
        } else {
            insert_path(&mut node.children[pos], components, depth + 1, vfs_offset);
        }
    } else {
        let mut child = PathTreeNode {
            name: component.to_string(),
            children: Vec::new(),
            vfs_offset: if is_leaf { Some(vfs_offset) } else { None },
        };

        if !is_leaf {
            insert_path(&mut child, components, depth + 1, vfs_offset);
        }

        node.children.push(child);
    }
}

/// Calculate maximum depth of the tree.
fn calculate_max_depth(node: &PathTreeNode, depth: u16) -> u16 {
    let mut max = depth;
    for child in &node.children {
        max = max.max(calculate_max_depth(child, depth + 1));
    }
    max
}
