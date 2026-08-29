//! Generated protobuf types for the Blizzard TACT/CASC protocol.
//!
//! Types are generated from `.proto` schema files that define the wire
//! format for `.product.db` and `product.db` files used by the Blizzard
//! Agent system.

/// Product database message types (`proto_database` package).
///
/// The top-level `Database` message is serialized as raw protobuf bytes
/// to `.product.db` (per-install) and `product.db` (main agent database).
#[allow(clippy::derive_partial_eq_without_eq)]
pub mod proto_database {
    include!(concat!(env!("OUT_DIR"), "/proto_database.rs"));
}
