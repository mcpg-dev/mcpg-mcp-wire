//! Protocol primitives the wire modules and the gateway runtime share.
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
//!   policy engine.
//! - [`caching`] — SEP-2549 caching hints (`ttlMs` + `cacheScope`
//!   response-envelope hints for cacheable results).
//! - [`canonical`] — RFC 8785 JSON canonicalization, for comparing and
//!   hashing JSON by value.
//! - [`client_capabilities`] — the client capability declarations a
//!   request carries in `_meta`.
//! - [`completion`] — `completion/complete` wire types
//!   (prompt-argument / resource-template autocomplete).
//! - [`content`] — content blocks (text / image / audio / embedded
//!   resource / resource link).
//! - [`deprecation`] — SEP-2596 feature-lifecycle deprecation
//!   advertisements + usage metering.
//! - [`messages`] — the parsed [`messages::ProtocolMessage`] and the
//!   [`messages::TransportRejection`] that cross from the protocol
//!   handler to the runtime.
//! - [`notifications`] — cancellation, progress, list-changed and
//!   resource-updated notifications.
//! - [`prompts`], [`resources`], [`tools`] — the `prompts/get`,
//!   `resources/read` and `tools/call` shapes the runtime builds.
//! - [`routing`] — params extraction for the method router.
//! - [`server_requests`] — the elicitation / sampling / roots requests
//!   a suspended pipeline puts to the client.
//! - [`skills`] — SEP-2640 Skills extension: `skills/list`, `skills/get`
//!   and `resources/directory/read` wire types.
//! - [`tasks`] — the stored task record.

pub mod apps;
pub mod caching;
pub mod canonical;
pub mod client_capabilities;
pub mod completion;
pub mod content;
pub mod deprecation;
pub mod error;
pub mod id_validation;
pub mod jsonrpc;
pub mod messages;
pub mod meta;
pub mod notifications;
pub mod prompts;
pub mod resources;
pub mod routing;
pub mod server_requests;
pub mod skills;
pub mod tasks;
pub mod tools;

/// HTTP header carrying the MCP protocol version on every Streamable
/// HTTP request and response.
pub const PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";
