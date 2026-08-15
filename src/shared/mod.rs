//! Version-agnostic protocol primitives.
//!
//! Types in this module are shared by every `ProtocolHandler` impl:
//!
//! - [`jsonrpc`] — JSON-RPC 2.0 envelope (`ClientMessage`,
//!   `JsonRpcRequest/Notification/Response`, `ProtocolHttpResponse`,
//!   `parse_client_message`).
//! - [`error`] — `ProtocolError` plus standard JSON-RPC error code
//!   constants.
//! - [`id_validation`] — JSON-RPC `id` rules.
//! - [`meta`] — `_meta` reserved-prefix policing.
//! - [`apps`] — SEP-1865 MCP Apps core: `_meta.ui` constants, the
//!   `ui://` resourceUri rewrite, and the tighten-only CSP/permission
//!   policy engine (version-agnostic).
//! - [`caching`] — SEP-2549 caching hints (`ttlMs` + `cacheScope`
//!   response-envelope hints for cacheable results).
//! - [`completion`] — version-agnostic `completion/complete` wire
//!   types (prompt-argument / resource-template autocomplete).
//! - [`content`] — content blocks (text / image / audio / embedded
//!   resource / resource link) used by both protocol revisions.
//! - [`deprecation`] — SEP-2596 feature-lifecycle deprecation
//!   advertisements + usage metering.
//! - [`messages`] — version-erased intermediates that cross the
//!   `ProtocolHandler` boundary (`ProtocolMessage`,
//!   `PipelineSuspension`, etc.).
//! - [`routing`] — params extraction shared by both wires' method routers.
//!
//! The `ProtocolHandler` trait itself stays in the gateway
//! (`mcpg::protocol::shared::traits`): it is the seam between these
//! wire types and the gateway's runtime services, and it is the one
//! piece of `shared` that is not wire.

pub mod apps;
pub mod caching;
pub mod completion;
pub mod content;
pub mod deprecation;
pub mod error;
pub mod id_validation;
pub mod jsonrpc;
pub mod messages;
pub mod meta;
pub mod routing;

/// HTTP header carrying the negotiated MCP protocol version on every
/// Streamable HTTP request. The name is stable across every revision
/// (the SEP-2243 standardisation did not rename it), so it is defined
/// once here and re-exported by the per-version wire modules and the
/// registry.
pub const PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";
