//! Index file parsing and management for CASC storage

mod group_index;
mod idx_parser;
mod index_file;

pub use group_index::GroupIndex;
pub use idx_parser::IdxParser;
pub use index_file::{IndexFile, IndexVersion};
