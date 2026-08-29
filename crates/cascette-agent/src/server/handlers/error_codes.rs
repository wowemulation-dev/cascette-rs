//! Numeric error codes for the Blizzard Agent HTTP API.
//!
//! The agent returns `{"error": <integer>}` in JSON error responses.

/// General bad request (2312). Missing required fields or malformed body.
pub const AGENT_ERROR_INVALID_REQUEST: u32 = 2312;

/// Protocol or path validation failed (2310). The `instructions_product`
/// field does not match `"NGDP"` or the patch URL is invalid.
pub const AGENT_ERROR_INVALID_PROTOCOL: u32 = 2310;

/// Extended config validation failed (2311). Build/CDN config values
/// are present but invalid.
pub const AGENT_ERROR_INVALID_CONFIG: u32 = 2311;

/// Duplicate product UID (2410). A product with the same UID is already
/// registered at a different install path.
pub const AGENT_ERROR_DUPLICATE_UID: u32 = 2410;

/// Install directory conflict (800). Another product already occupies
/// the requested install directory.
pub const AGENT_ERROR_DIRECTORY_CONFLICT: u32 = 800;
