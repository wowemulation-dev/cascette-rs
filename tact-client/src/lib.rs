//! TACT protocol client for Cascette

pub mod batch;
pub mod error;
pub mod http;
pub mod pool;
pub mod region;
pub mod response_types;
pub mod resumable;

pub use batch::{BatchConfig, BatchRequest, BatchResponse, BatchStats, RequestBatcher};
pub use error::{Error, Result};
pub use http::{HttpClient, ProtocolVersion};
pub use pool::{PoolConfig, create_pooled_client, get_global_pool, init_global_pool};
pub use region::Region;
pub use response_types::{CdnEntry, VersionEntry, parse_cdns, parse_versions};
pub use resumable::{DownloadProgress, ResumableDownload};
