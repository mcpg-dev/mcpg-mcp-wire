//! [`ProtocolError`] — the parser / dispatcher error type — plus the
//! error code constants: the five JSON-RPC 2.0 baseline codes, the
//! MCP-reserved (`-32020..-32099`) codes, and the application-level
//! (`-33xxx`) payment codes. Extension-specific codes live with their
//! extension (e.g. the MCP Events codes).

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

/// MCP-reserved JSON-RPC error code: the request names no protocol
/// version, or one this server does not serve.
pub const UNSUPPORTED_PROTOCOL_VERSION_CODE: i32 = -32022;

/// MCP-reserved JSON-RPC error code: a required client capability was
/// not declared on a request that needs it.
pub const MISSING_REQUIRED_CLIENT_CAPABILITY_CODE: i32 = -32021;

/// MCP-reserved JSON-RPC error code: an HTTP routing header
/// (`Mcp-Protocol-Version` cross-check, `Mcp-Method`, `Mcp-Name`)
/// disagrees with the request body (SEP-2243/SEP-2575).
pub const HEADER_MISMATCH_CODE: i32 = -32020;

/// MPP-defined JSON-RPC error code for payment required.
///
/// Application-level range (`-33xxx`), so it never collides with an
/// MCP-reserved code.
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

    /// Build an error the server's own application logic chose, as it is:
    /// its code, message and data.
    pub fn application(
        id: Option<Value>,
        code: i32,
        message: impl Into<String>,
        data: Option<Value>,
    ) -> Self {
        Self {
            id,
            code,
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
    /// The wire returns this when a request needs a client
    /// capability the caller did not declare in its per-request
    /// `_meta.io.modelcontextprotocol/clientCapabilities`. It is the
    /// single mint point for that error.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code sits at the value the JSON-RPC / MCP / MPP contracts
    /// require, and the application-range codes stay outside the spec
    /// range.
    #[test]
    #[allow(
        clippy::assertions_on_constants,
        reason = "pins the wire values of the error codes"
    )]
    fn error_code_constants_are_stable() {
        assert_eq!(PARSE_ERROR_CODE, -32700);
        assert_eq!(INVALID_REQUEST_CODE, -32600);
        assert_eq!(METHOD_NOT_FOUND_CODE, -32601);
        assert_eq!(INVALID_PARAMS_CODE, -32602);
        assert_eq!(INTERNAL_ERROR_CODE, -32603);
        assert_eq!(HEADER_MISMATCH_CODE, -32020);
        assert_eq!(MISSING_REQUIRED_CLIENT_CAPABILITY_CODE, -32021);
        assert_eq!(UNSUPPORTED_PROTOCOL_VERSION_CODE, -32022);
        assert!(PAYMENT_REQUIRED_CODE < -33000);
        assert!(PAYMENT_VERIFICATION_FAILED_CODE < -33000);
    }

    #[test]
    fn method_not_found_renders_into_the_jsonrpc_envelope() {
        let err = ProtocolError::method_not_found(Some(serde_json::json!(3)), "x/y");
        assert_eq!(err.code(), METHOD_NOT_FOUND_CODE);
        let rendered = serde_json::to_value(err.into_jsonrpc_error()).unwrap();
        assert_eq!(rendered["id"], 3);
        assert_eq!(rendered["error"]["message"], "Method not found: x/y");
    }
}
