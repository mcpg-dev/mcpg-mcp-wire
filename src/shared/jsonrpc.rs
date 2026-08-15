//! JSON-RPC 2.0 envelope types shared across MCP protocol revisions.
//!
//! The JSON-RPC layer is the same in every MCP version: requests carry
//! `id` + `method` + `params`; notifications drop the `id`; responses
//! drop the `method`. This module owns the envelope types plus the
//! single entry point that turns a raw JSON body into a typed
//! [`ClientMessage`].

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::error::ProtocolError;
use crate::shared::id_validation::validate_jsonrpc_id;
use crate::shared::meta::validate_request_meta;

/// JSON-RPC protocol version string. Every message carries
/// `"jsonrpc": "2.0"`.
pub const JSONRPC_VERSION: &str = "2.0";

/// Inbound JSON-RPC message from the client. Uses serde `untagged`
/// deserialization: tries Request first (has `id` + `method`), then
/// Notification (`method` only), then Response (`id` +
/// `result`/`error`, no `method`). Order matters because `untagged`
/// tries each variant top-to-bottom and picks the first that succeeds.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ClientMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
}

/// A client-initiated JSON-RPC request (has both `id` and `method`).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// A client-initiated JSON-RPC notification (has `method` but no `id`).
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// A JSON-RPC response from the client to a server-initiated request
/// (legacy versions only). Has `id` and either `result` or `error`,
/// but no `method` field.
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<JsonRpcErrorBody>,
}

/// Wire-format response: either a JSON-RPC success, a JSON-RPC error,
/// or a notification acknowledgement (HTTP 202 with no body).
#[derive(Debug, Clone, Serialize)]
pub enum ProtocolResponse {
    JsonRpcSuccess(JsonRpcSuccess),
    JsonRpcError(JsonRpcError),
    NotificationAccepted,
}

/// Protocol response enriched with HTTP-level metadata (status code,
/// session header). The `session_id_header` field is populated only
/// by legacy versions (modern stateless versions leave it `None`).
#[derive(Debug, Clone, Serialize)]
pub struct ProtocolHttpResponse {
    pub http_status: u16,
    #[serde(skip_serializing)]
    pub session_id_header: Option<String>,
    pub response: ProtocolResponse,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcSuccess {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub result: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcError {
    pub jsonrpc: &'static str,
    pub id: Option<Value>,
    pub error: JsonRpcErrorBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorBody {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Parse a single JSON-RPC message body.
///
/// MCP 2025-11-25 requires each POST body to contain exactly one
/// JSON-RPC request, notification, or response. Batch arrays were
/// removed from the spec in 2025-06-18 and are not accepted by MCPG;
/// the transport layer rejects them before this call.
///
/// Validates `_meta` keys per MCP reserved-prefix rules (`mcp.` /
/// `modelcontextprotocol.` namespaces fail closed), then checks the
/// JSON-RPC `id` field (must be string or non-empty number, never
/// null / structured).
pub fn parse_client_message(body: Value) -> Result<ClientMessage, ProtocolError> {
    validate_request_meta(&body)?;

    let Some(obj) = body.as_object() else {
        return Err(ProtocolError::invalid_request(
            "JSON-RPC message must be a JSON object",
        ));
    };
    let has_id = obj.contains_key("id");
    let has_method = obj.contains_key("method");
    let has_result = obj.contains_key("result");
    let has_error = obj.contains_key("error");

    // Classify STRICTLY before the untagged deserialize, so a malformed
    // request can't fall through to the Response variant and be mistaken
    // for a client answer to a server-initiated request (which would
    // inject a bogus step result into a suspended pipeline). A message
    // carrying `method` is a Request/Notification; one without is a
    // Response and must satisfy JSON-RPC 2.0's exactly-one(result|error).
    if has_method {
        // `method` MUST be a string. Without this check a non-string
        // `method` fails the Request/Notification variants and falls
        // through to Response, decoding as a null result.
        if !obj.get("method").is_some_and(Value::is_string) {
            return Err(ProtocolError::invalid_request(
                "JSON-RPC `method` must be a string",
            ));
        }
        // A request/notification must not also carry response fields.
        if has_result || has_error {
            return Err(ProtocolError::invalid_request(
                "JSON-RPC message carries both `method` and `result`/`error`",
            ));
        }
        if has_id {
            validate_jsonrpc_id(&obj["id"])?;
            return serde_json::from_value(body)
                .map(ClientMessage::Request)
                .map_err(|error| {
                    ProtocolError::invalid_request(format!("invalid JSON-RPC request: {error}"))
                });
        }
        return serde_json::from_value(body)
            .map(ClientMessage::Notification)
            .map_err(|error| {
                ProtocolError::invalid_request(format!("invalid JSON-RPC notification: {error}"))
            });
    }

    // No `method` → it must be a Response to a server-initiated request:
    // a validated non-null `id` plus EXACTLY ONE of `result`/`error`.
    if !has_id {
        return Err(ProtocolError::invalid_request(
            "JSON-RPC message must carry a `method` (request/notification) or an `id` (response)",
        ));
    }
    validate_jsonrpc_id(&obj["id"])?;
    if has_result == has_error {
        return Err(ProtocolError::invalid_request(
            "JSON-RPC response must carry exactly one of `result` or `error`",
        ));
    }
    serde_json::from_value(body)
        .map(ClientMessage::Response)
        .map_err(|error| {
            ProtocolError::invalid_request(format!("invalid JSON-RPC response: {error}"))
        })
}

/// A `-32603` internal error as an HTTP 500 protocol response.
///
/// The shape both wires answer with when the gateway itself failed rather than
/// the request — a dropped runtime handle, a serialization that cannot fail but
/// did. Kept beside [`ProtocolHttpResponse`] so the two wires cannot drift on
/// the status/code pairing.
pub fn handler_internal_error(jsonrpc_id: Option<Value>, message: &str) -> ProtocolHttpResponse {
    ProtocolHttpResponse {
        http_status: 500,
        session_id_header: None,
        response: ProtocolResponse::JsonRpcError(JsonRpcError {
            jsonrpc: JSONRPC_VERSION,
            id: jsonrpc_id,
            error: JsonRpcErrorBody {
                code: crate::shared::error::INTERNAL_ERROR_CODE,
                message: message.to_owned(),
                data: None,
            },
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_single_request() {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
        let msg = parse_client_message(body).unwrap();
        assert!(matches!(msg, ClientMessage::Request(_)));
    }

    #[test]
    fn rejects_array_body() {
        // MCP 2025-11-25 forbids batch arrays. The transport rejects
        // arrays before calling parse_client_message; this confirms the
        // parser itself will not silently coerce an array into a
        // single message.
        let body = json!([{ "jsonrpc": "2.0", "id": 1, "method": "ping" }]);
        assert!(parse_client_message(body).is_err());
    }

    #[test]
    fn rejects_null_id() {
        let body = json!({ "jsonrpc": "2.0", "id": null, "method": "tools/list" });
        let err = parse_client_message(body).unwrap_err();
        assert!(err.message().to_lowercase().contains("null"));
    }

    #[test]
    fn rejects_bool_id() {
        let body = json!({ "jsonrpc": "2.0", "id": true, "method": "tools/list" });
        assert!(parse_client_message(body).is_err());
    }

    #[test]
    fn rejects_object_id() {
        let body = json!({ "jsonrpc": "2.0", "id": { "x": 1 }, "method": "tools/list" });
        assert!(parse_client_message(body).is_err());
    }

    #[test]
    fn rejects_empty_string_id() {
        let body = json!({ "jsonrpc": "2.0", "id": "", "method": "tools/list" });
        assert!(parse_client_message(body).is_err());
    }

    #[test]
    fn accepts_string_and_number_id() {
        parse_client_message(json!({ "jsonrpc": "2.0", "id": "r-1", "method": "tools/list" }))
            .unwrap();
        parse_client_message(json!({ "jsonrpc": "2.0", "id": 42, "method": "tools/list" }))
            .unwrap();
    }

    #[test]
    fn rejects_non_string_method() {
        // `{"id":5,"method":123}` must not fall through to a Response with
        // a null result (which would advance a suspended pipeline).
        let err =
            parse_client_message(json!({ "jsonrpc": "2.0", "id": 5, "method": 123 })).unwrap_err();
        assert!(
            err.message().to_lowercase().contains("method"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn rejects_no_method_no_result_or_error() {
        // `{"id":"srv-1"}` is neither a request nor a valid response — it
        // must not become Response{result:None,error:None}.
        let err = parse_client_message(json!({ "jsonrpc": "2.0", "id": "srv-1" })).unwrap_err();
        assert!(
            err.message().to_lowercase().contains("exactly one"),
            "{}",
            err.message()
        );
    }

    #[test]
    fn rejects_response_with_both_result_and_error() {
        let err = parse_client_message(json!({
            "jsonrpc": "2.0", "id": "srv-1", "result": {}, "error": { "code": -1, "message": "x" }
        }))
        .unwrap_err();
        assert!(err.message().to_lowercase().contains("exactly one"));
    }

    #[test]
    fn rejects_message_with_method_and_result() {
        let err = parse_client_message(json!({
            "jsonrpc": "2.0", "id": 1, "method": "ping", "result": {}
        }))
        .unwrap_err();
        assert!(err.message().to_lowercase().contains("both"));
    }

    #[test]
    fn accepts_valid_response_with_result() {
        let msg = parse_client_message(json!({
            "jsonrpc": "2.0", "id": "srv-1", "result": { "ok": true }
        }))
        .unwrap();
        assert!(matches!(msg, ClientMessage::Response(_)));
    }

    #[test]
    fn accepts_valid_response_with_error() {
        let msg = parse_client_message(json!({
            "jsonrpc": "2.0", "id": "srv-1", "error": { "code": -1, "message": "nope" }
        }))
        .unwrap();
        assert!(matches!(msg, ClientMessage::Response(_)));
    }

    #[test]
    fn accepts_notification_without_id() {
        let msg = parse_client_message(json!({
            "jsonrpc": "2.0", "method": "notifications/cancelled", "params": { "requestId": 1 }
        }))
        .unwrap();
        assert!(matches!(msg, ClientMessage::Notification(_)));
    }

    #[test]
    fn rejects_response_with_null_id() {
        // A response must carry a validated non-null id.
        let err = parse_client_message(json!({ "jsonrpc": "2.0", "id": null, "result": {} }))
            .unwrap_err();
        assert!(err.message().to_lowercase().contains("null"));
    }
}
