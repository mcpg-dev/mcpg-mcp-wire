//! [`ProtocolError`] — the parser / dispatcher error type — plus the
//! standard JSON-RPC 2.0 error code constants.
//!
//! Per-version error codes (`-32042 URL_ELICITATION_REQUIRED`,
//! `-32100 ELICITATION_NOT_SUPPORTED`, guardrail / content-too-large
//! codes that drift between revisions, etc.) live in the version's
//! own wire module under `protocol/v_<date>/wire/`. This file owns
//! only the codes that are stable across every MCP revision MCPG
//! supports — the five JSON-RPC 2.0 baseline codes and the
//! application-level (`-33xxx`) payment codes.

use serde_json::Value;

use crate::shared::jsonrpc::{JSONRPC_VERSION, JsonRpcError, JsonRpcErrorBody};

/// JSON-RPC 2.0: Parse error.
pub const PARSE_ERROR_CODE: i32 = -32700;
/// JSON-RPC 2.0: Invalid Request.
pub const INVALID_REQUEST_CODE: i32 = -32600;
/// JSON-RPC 2.0: Method not found.
pub const METHOD_NOT_FOUND_CODE: i32 = -32601;
/// JSON-RPC 2.0: Invalid params.
pub const INVALID_PARAMS_CODE: i32 = -32602;
/// JSON-RPC 2.0: Internal error.
pub const INTERNAL_ERROR_CODE: i32 = -32603;

/// MCP-reserved JSON-RPC error code: the negotiated protocol version is
/// not supported by this server. The `2026-07-28` revision moved this
/// from the impl-defined `-32004` into the MCP-reserved band
/// (`-32020..-32099`).
pub const UNSUPPORTED_PROTOCOL_VERSION_CODE: i32 = -32022;

/// MCP-reserved JSON-RPC error code: a required client capability was
/// not declared on a request that needs it (`2026-07-28`).
pub const MISSING_REQUIRED_CLIENT_CAPABILITY_CODE: i32 = -32021;

/// MCP-reserved JSON-RPC error code: an HTTP routing header
/// (`Mcp-Protocol-Version` cross-check, `Mcp-Method`, `Mcp-Name`)
/// disagrees with the request body (`2026-07-28`, SEP-2243/SEP-2575).
pub const HEADER_MISMATCH_CODE: i32 = -32020;

/// MPP-defined JSON-RPC error code for payment required.
///
/// Application-level range (`-33xxx`) to avoid collision with MCP
/// spec codes — see commit history for the migration off `-32042`,
/// which the spec reserved for URL-mode elicitation.
pub const PAYMENT_REQUIRED_CODE: i32 = -33042;

/// MPP-defined JSON-RPC error code for payment verification failed.
pub const PAYMENT_VERIFICATION_FAILED_CODE: i32 = -33043;

/// Parser / dispatcher error.
///
/// Constructed by `parse_client_message`, `map_client_message_to_operation`,
/// and dispatch handlers; consumed by the transport layer which renders
/// it into the wire `JsonRpcError` envelope.
#[derive(Debug, Clone)]
pub struct ProtocolError {
    id: Option<Value>,
    code: i32,
    message: String,
    data: Option<Value>,
}

impl ProtocolError {
    /// Build an Invalid Request error (`-32600`).
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self {
            id: None,
            code: INVALID_REQUEST_CODE,
            message: message.into(),
            data: None,
        }
    }

    /// Build an Invalid Params error (`-32602`) optionally carrying
    /// the originating JSON-RPC id and structured `data`.
    pub fn invalid_params(
        id: Option<Value>,
        message: impl Into<String>,
        data: Option<Value>,
    ) -> Self {
        Self {
            id,
            code: INVALID_PARAMS_CODE,
            message: message.into(),
            data,
        }
    }

    /// Build a Method-Not-Found error (`-32601`) including the
    /// unknown method name in the message.
    pub fn method_not_found(id: Option<Value>, method: impl Into<String>) -> Self {
        Self {
            id,
            code: METHOD_NOT_FOUND_CODE,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }

    /// Build a Missing-Required-Client-Capability error (`-32021`).
    ///
    /// The `2026-07-28` wire returns this when a request needs a client
    /// capability the caller did not declare in its per-request
    /// `_meta.io.modelcontextprotocol/clientCapabilities`. No producer
    /// is wired on the modern dispatch path yet — the per-request
    /// capability-enforcement gate is a later phase; this constructor
    /// exists so that gate has a single canonical mint point.
    pub fn missing_required_client_capability(
        id: Option<Value>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            id,
            code: MISSING_REQUIRED_CLIENT_CAPABILITY_CODE,
            message: message.into(),
            data: None,
        }
    }

    /// Build a Parse Error (`-32700`).
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self {
            id: None,
            code: PARSE_ERROR_CODE,
            message: message.into(),
            data: None,
        }
    }

    /// Render into the wire `JsonRpcError` envelope.
    pub fn into_jsonrpc_error(self) -> JsonRpcError {
        JsonRpcError {
            jsonrpc: JSONRPC_VERSION,
            id: self.id,
            error: JsonRpcErrorBody {
                code: self.code,
                message: self.message,
                data: self.data,
            },
        }
    }

    /// Inspect the JSON-RPC error code (for tests and diagnostics).
    pub fn code(&self) -> i32 {
        self.code
    }

    /// Inspect the diagnostic message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)
    }
}

impl std::error::Error for ProtocolError {}
