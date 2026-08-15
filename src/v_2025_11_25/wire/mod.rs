//! Wire-format types for MCP revision `2025-11-25`.
//!
//! ## Layout
//!
//! Each sub-module owns one cohesive feature area of the MCP wire surface.
//! Operation discriminators live in [`operations`], the
//! `map_client_message_to_operation` router in [`routing`], and transport-level
//! constants alongside the feature that defines them.
//!
//! - [`lifecycle`] — `initialize` / capabilities negotiation
//!   (`InitializeParams`, `ClientCapabilities`, `ServerCapabilities`,
//!   plus task / sampling / elicitation / roots capability sub-types).
//!
//! - [`tools`], [`prompts`], [`resources`], [`completion`], [`logging`],
//!   [`tasks`], [`elicitation`], [`sampling`] — one MCP feature surface each.
//! - [`common`] — types shared by more than one of the above.
//! - [`operations`], [`routing`] — the operation enum and its router.

pub mod common;
pub mod completion;
pub mod elicitation;
pub mod lifecycle;
pub mod logging;
pub mod operations;
pub mod prompts;
pub mod resources;
pub mod routing;
pub mod sampling;
pub mod tasks;
pub mod tools;

// ---------------------------------------------------------------------------
// Version-string and transport-header constants. These are wire-stable for
// the 2025-11-25 revision; subsequent revisions own their own constants
// under their own version folder.
// ---------------------------------------------------------------------------

/// The negotiated wire-string identifier for this revision.
///
/// Matches `crate::version::ProtocolVersion::V_2025_11_25.as_str()`
/// and the value embedded in `InitializeResult.protocolVersion` on the
/// wire.
pub const SUPPORTED_PROTOCOL_VERSION: &str = "2025-11-25";

/// MCP Streamable HTTP spec: an absent `Mcp-Protocol-Version` header
/// on post-initialize requests is interpreted as `2025-03-26`. The
/// list below is the additional legacy revisions we accept when
/// explicitly requested.
pub const LEGACY_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26"];

/// Header-absent fallback per the MCP Streamable HTTP spec.
pub const LEGACY_DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

pub use crate::shared::PROTOCOL_VERSION_HEADER;

/// HTTP header carrying the server-minted session id (legacy
/// revisions only — `DRAFT-2026-v1` drops sessions).
pub const SESSION_ID_HEADER: &str = "mcp-session-id";

/// MCP spec marks `sampling/createMessage.maxTokens` as REQUIRED.
/// Substituted when an operator omits it from a pipeline sampling
/// step.
pub const DEFAULT_SAMPLING_MAX_TOKENS: u64 = 4096;
