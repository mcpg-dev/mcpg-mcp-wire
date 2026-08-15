//! Cross-cutting envelopes and constants used across the 2025-11-25
//! wire surface.
//!
//! Types in this module are not specific to any one feature
//! (tools / prompts / resources / completion / logging / tasks /
//! elicitation / sampling). They are reused by multiple operation
//! arms and by transport-layer plumbing:
//!
//! - [`ListParams`] — pagination shape for any `*/list` request.
//! - [`CancelledNotificationParams`] — body of
//!   `notifications/cancelled`.
//! - [`ListChangedNotification`] — generic notification envelope
//!   shared by `notifications/{tools,prompts,resources}/list_changed`.
//! - [`ProgressNotification`] + [`ProgressParams`] —
//!   `notifications/progress` shape.
//! - [`ServerJsonRpcRequest`] — outbound request envelope used when
//!   the server emits `elicitation/create`, `sampling/createMessage`,
//!   or `roots/list` (server-initiated, 2025-11-25 only — replaced by
//!   MRTR `inputRequests` in DRAFT-2026-v1).
//! - [`EmptyResult`] — empty success body for methods like `ping`.
//! - Error codes: [`CONTENT_TOO_LARGE_CODE`] (-32002),
//!   [`GUARDRAIL_DENIED_CODE`] (-32044),
//!   [`GUARDRAIL_SERVICE_ERROR_CODE`] (-32046).

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Cross-cutting wire error code constants.
// ---------------------------------------------------------------------------

/// JSON-RPC error code: resource content too large to return inline.
/// Per MCP 2025-11-25 spec uses `-32002` (the legacy "Resource Not
/// Found" slot). `DRAFT-2026-v1` re-uses `-32602 InvalidParams` for
/// resource-not-found per SEP-2164; the content-too-large semantics
/// move to a different code in the modern wire.
pub const CONTENT_TOO_LARGE_CODE: i32 = -32002;

/// JSON-RPC error code for guardrail denial. Plugin-defined; used by
/// the `mcpg-plugin-security-guardrails` chain.
pub const GUARDRAIL_DENIED_CODE: i32 = -32044;

/// JSON-RPC error code for guardrail service error (when an
/// operator-configured guardrail callback fails and the guardrail's
/// `on_error = deny`).
pub const GUARDRAIL_SERVICE_ERROR_CODE: i32 = -32046;

// ---------------------------------------------------------------------------
// Pagination + cancellation + progress + server-initiated request.
// ---------------------------------------------------------------------------

/// Pagination parameters for any `*/list` operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Parameters for `notifications/cancelled`. The client identifies
/// the in-flight request by its JSON-RPC `id`; the optional
/// `reason` is a free-form human string surfaced to logs / audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledNotificationParams {
    pub request_id: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// JSON-RPC notification envelope for any `*/list_changed` event.
/// Identical wire shape across tools / prompts / resources — only
/// the `method` string differs at emission time.
#[derive(Debug, Clone, Serialize)]
pub struct ListChangedNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
}

/// JSON-RPC notification for progress updates (`notifications/progress`).
#[derive(Debug, Clone, Serialize)]
pub struct ProgressNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: ProgressParams,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressParams {
    pub progress_token: Value,
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A JSON-RPC request from the server to the client, sent via SSE.
/// Used in 2025-11-25 to mint `elicitation/create`,
/// `sampling/createMessage`, and `roots/list` server-initiated
/// requests. `DRAFT-2026-v1` replaces this entire mechanism with
/// MRTR `inputRequests` (SEP-2322).
#[derive(Debug, Clone, Serialize)]
pub struct ServerJsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub method: String,
    pub params: Value,
}

/// Empty `{}` success body. Used by methods like `ping` and
/// `notifications/initialized` ack.
#[derive(Debug, Clone, Serialize)]
pub struct EmptyResult {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::error::{PAYMENT_REQUIRED_CODE, PAYMENT_VERIFICATION_FAILED_CODE};
    use crate::v_2025_11_25::wire::elicitation::{
        ELICITATION_NOT_SUPPORTED_CODE, URL_ELICITATION_REQUIRED_CODE,
    };

    use serde_json::json;

    #[test]
    fn list_params_deserializes_with_cursor() {
        let v = json!({ "cursor": "abc123" });
        let params: ListParams = serde_json::from_value(v).unwrap();
        assert_eq!(params.cursor.as_deref(), Some("abc123"));
    }

    #[test]
    fn list_params_defaults_without_cursor() {
        let params: ListParams = serde_json::from_value(json!({})).unwrap();
        assert!(params.cursor.is_none());
    }

    #[test]
    fn list_params_default_constructor_has_neither_field() {
        let params: ListParams = ListParams::default();
        assert!(params.cursor.is_none());
        assert!(params.meta.is_none());
    }

    #[test]
    fn list_params_carries_meta_when_present() {
        let v = json!({ "cursor": "p2", "_meta": { "trace": "x" } });
        let params: ListParams = serde_json::from_value(v).unwrap();
        assert_eq!(params.cursor.as_deref(), Some("p2"));
        assert!(params.meta.is_some());
    }

    #[test]
    fn cancelled_notification_params_round_trip() {
        let v = json!({ "requestId": 42, "reason": "user abort" });
        let p: CancelledNotificationParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.request_id, json!(42));
        assert_eq!(p.reason.as_deref(), Some("user abort"));
    }

    #[test]
    fn server_jsonrpc_request_serializes() {
        let request = ServerJsonRpcRequest {
            jsonrpc: "2.0",
            id: Value::String("srv-req-1".to_owned()),
            method: "elicitation/create".to_owned(),
            params: json!({ "message": "Do you confirm?" }),
        };
        let v = serde_json::to_value(&request).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["method"], "elicitation/create");
        assert_eq!(v["id"], "srv-req-1");
        assert_eq!(v["params"]["message"], "Do you confirm?");
    }

    #[test]
    fn progress_notification_serializes_camel_case() {
        let notif = ProgressNotification {
            jsonrpc: "2.0",
            method: "notifications/progress",
            params: ProgressParams {
                progress_token: json!("p1"),
                progress: 0.5,
                total: Some(1.0),
                message: Some("halfway".to_owned()),
            },
        };
        let v = serde_json::to_value(&notif).unwrap();
        assert_eq!(v["params"]["progressToken"], "p1");
        assert_eq!(v["params"]["progress"], 0.5);
        assert_eq!(v["params"]["total"], 1.0);
        assert_eq!(v["params"]["message"], "halfway");
    }

    #[test]
    fn list_changed_notification_serializes() {
        let notif = ListChangedNotification {
            jsonrpc: "2.0",
            method: "notifications/tools/list_changed",
        };
        let v = serde_json::to_value(&notif).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["method"], "notifications/tools/list_changed");
    }

    #[test]
    fn empty_result_serializes_to_empty_object() {
        let v = serde_json::to_value(&EmptyResult {}).unwrap();
        assert_eq!(v, json!({}));
    }

    /// Compile-time contract: every error code is at the value
    /// the MCP spec / MPP / guardrails contract requires, and none
    /// of the application-range codes collide with the spec range.
    #[test]
    #[allow(
        clippy::assertions_on_constants,
        reason = "compile-time contract check — regression guard if a future \
                  constant drifts outside the plugin error range per spec.md §10.1"
    )]
    fn error_code_constants_are_stable() {
        assert_eq!(CONTENT_TOO_LARGE_CODE, -32002);
        assert_eq!(GUARDRAIL_DENIED_CODE, -32044);
        assert_eq!(GUARDRAIL_SERVICE_ERROR_CODE, -32046);
        assert_eq!(URL_ELICITATION_REQUIRED_CODE, -32042);
        assert_eq!(ELICITATION_NOT_SUPPORTED_CODE, -32100);
        // Payment codes live outside the MCP spec range (-33xxx).
        assert!(PAYMENT_REQUIRED_CODE < -33000);
        assert!(PAYMENT_VERIFICATION_FAILED_CODE < -33000);
    }
}
