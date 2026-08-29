//! HTTP server: Axum router and request handlers matching real Blizzard agent endpoints.

pub mod handlers;
pub mod router;

pub use router::create_router;
