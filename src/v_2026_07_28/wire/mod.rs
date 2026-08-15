//! Wire types for MCP revision `2026-07-28`.
//!
//! Re-exports stay at the version root (`v_2026_07_28::mod.rs`) so
//! `crate::v_2026_07_28::wire::*` is the single canonical
//! import path for downstream code.

pub mod completion;
pub mod headers;
pub mod lifecycle;
pub mod meta;
pub mod mrtr;
pub mod operations;
pub mod prompts;
pub mod request_meta;
pub mod resources;
pub mod routing;
pub mod subscriptions;
pub mod tools;

// ---------------------------------------------------------------------------
// Wire-string identifiers + transport-header names.
// ---------------------------------------------------------------------------

/// Wire-string identifier for this revision — the final published
/// MCP revision date. `ProtocolVersion::parse()` also accepts the
/// pre-final `"DRAFT-2026-v1"` label as a transitional inbound alias.
pub const SUPPORTED_PROTOCOL_VERSION: &str = "2026-07-28";

pub use crate::shared::PROTOCOL_VERSION_HEADER;

pub use headers::{
    METHOD_HEADER, NAME_HEADER, PARAM_HEADER_PREFIX, X_MCP_HEADER_KEYWORD, decode_header_value,
    encode_header_value, header_mismatch, name_source_field, promote_param_headers,
    validate_name_header, validate_param_headers,
};
pub use request_meta::enforce_request_meta_triple;
