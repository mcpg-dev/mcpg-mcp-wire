//! Elicitation wire types for MCP revision `2025-11-25`.
//!
//! Elicitation is the server-initiated path where the server asks
//! the client to collect additional input from the user. Two modes:
//!
//! - **Form mode**: the server provides a JSON Schema describing the
//!   input it needs, and the client renders a form. The client's
//!   reply ships inline as a JSON-RPC response.
//! - **URL mode**: the server provides an out-of-band URL the client
//!   navigates to (typically for OAuth-style flows). The client
//!   correlates the outcome back via the
//!   `notifications/elicitation/complete` channel using the
//!   server-minted `elicitationId`.
//!
//! ## Modern counterpart
//!
//! On `2026-07-28`, `elicitation/create` is an `inputRequests` map entry inside
//! MRTR (`InputRequiredResult`, SEP-2322) rather than a standalone
//! server-initiated request, and the URL completion channel
//! (`notifications/elicitation/complete`) is replaced by ordinary client
//! retries carrying `inputResponses`. Those shapes live in
//! [`v_2026_07_28::wire::mrtr`](crate::v_2026_07_28::wire::mrtr); the
//! ones here are frozen for the `2025-11-25` compatibility window.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC error code: URL elicitation required (-32042 per MCP
/// 2025-11-25 spec). Servers emit this when they cannot proceed
/// without a URL-mode elicitation result and the client has not yet
/// fulfilled one.
pub const URL_ELICITATION_REQUIRED_CODE: i32 = -32042;

/// JSON-RPC error code for elicitation not supported or failed
/// (-32100 per MCP 2025-11-25 spec).
pub const ELICITATION_NOT_SUPPORTED_CODE: i32 = -32100;

/// Server-to-client elicitation request parameters.
///
/// Supports both form-based and URL-based elicitation per MCP
/// 2025-11-25. The `mode` field determines which sibling fields are
/// required:
///
/// - `mode: "form"` → `requestedSchema` (required)
/// - `mode: "url"` → `elicitationId` (required), `url` (required)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationCreateParams {
    /// Elicitation mode: `"form"` or `"url"`.
    pub mode: String,
    pub message: String,
    /// Form mode: a JSON Schema describing the requested input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_schema: Option<Value>,
    /// URL mode: unique identifier for correlating the completion
    /// notification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation_id: Option<String>,
    /// URL mode: URL the client should navigate the user to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Optional task metadata for task-augmented elicitation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
    /// Hint for how to present the elicitation: `"inline"`,
    /// `"popup"`, or `"newWindow"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Elicitation result action from the client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ElicitationAction {
    Accept,
    Decline,
    Cancel,
}

/// JSON-RPC notification: `notifications/elicitation/complete`. Sent
/// by the server after URL-based elicitation completes, or by the
/// client to indicate the terminal action against a previously-issued
/// `elicitation/create` with `mode: "url"`.
#[derive(Debug, Clone, Serialize)]
pub struct ElicitationCompleteNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: ElicitationCompleteParams,
}

/// Parameters for `notifications/elicitation/complete`.
///
/// Carries both directions: the server emits this when URL-mode
/// elicitation completes, and the client sends it to indicate the
/// terminal action taken against a previously-issued
/// `elicitation/create` with `mode: "url"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationCompleteParams {
    pub elicitation_id: Value,
    pub action: ElicitationAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn elicitation_create_params_serialize() {
        let params = ElicitationCreateParams {
            mode: "form".to_owned(),
            message: "Confirm deployment?".to_owned(),
            requested_schema: Some(json!({
                "type": "object",
                "properties": { "confirm": { "type": "boolean" } },
            })),
            elicitation_id: None,
            url: None,
            task: None,
            presentation_hint: None,
            meta: None,
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["message"], "Confirm deployment?");
        assert!(v["requestedSchema"].is_object());
    }

    #[test]
    fn elicitation_params_form_mode_serializes() {
        let params = ElicitationCreateParams {
            mode: "form".to_owned(),
            message: "Confirm?".to_owned(),
            requested_schema: Some(json!({ "type": "object" })),
            elicitation_id: None,
            url: None,
            task: None,
            presentation_hint: None,
            meta: None,
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["mode"], "form");
        assert!(v.get("url").is_none());
        assert!(v.get("elicitationId").is_none());
    }

    #[test]
    fn elicitation_params_url_mode_serializes() {
        let params = ElicitationCreateParams {
            mode: "url".to_owned(),
            message: "Complete auth".to_owned(),
            requested_schema: None,
            elicitation_id: Some("elic-123".to_owned()),
            url: Some("https://auth.example.com/flow".to_owned()),
            task: None,
            presentation_hint: Some("popup".to_owned()),
            meta: None,
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["mode"], "url");
        assert_eq!(v["elicitationId"], "elic-123");
        assert_eq!(v["url"], "https://auth.example.com/flow");
        assert_eq!(v["presentationHint"], "popup");
        assert!(v.get("requestedSchema").is_none());
    }

    #[test]
    fn elicitation_action_serializes() {
        assert_eq!(
            serde_json::to_value(ElicitationAction::Accept).unwrap(),
            "accept"
        );
        assert_eq!(
            serde_json::to_value(ElicitationAction::Decline).unwrap(),
            "decline"
        );
        assert_eq!(
            serde_json::to_value(ElicitationAction::Cancel).unwrap(),
            "cancel"
        );
    }

    #[test]
    fn elicitation_complete_params_round_trip() {
        let v = json!({
            "elicitationId": "elic-9",
            "action": "accept",
            "content": { "answer": 42 }
        });
        let p: ElicitationCompleteParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.action, ElicitationAction::Accept);
        assert_eq!(p.elicitation_id, json!("elic-9"));
        assert!(p.content.is_some());
    }

    #[test]
    fn error_code_constants_match_spec() {
        assert_eq!(URL_ELICITATION_REQUIRED_CODE, -32042);
        assert_eq!(ELICITATION_NOT_SUPPORTED_CODE, -32100);
    }
}
