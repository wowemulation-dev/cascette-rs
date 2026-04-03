//! SQLite-backed state persistence for products and operations.

pub mod db;
pub mod queue;
pub mod registry;
pub mod size_cache;

pub use db::Database;
pub use queue::OperationQueue;
pub use registry::ProductRegistry;
pub use size_cache::SizeEstimateCache;
