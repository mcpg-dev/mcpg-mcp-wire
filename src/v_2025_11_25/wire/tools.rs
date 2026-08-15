//! Tools wire types for MCP revision `2025-11-25`.
//!
//! - [`ToolCallParams`] — body of a `tools/call` request.
//! - [`ToolsListResult`] — body of a `tools/list` result.
//! - [`ToolCallResult`] — body of a `tools/call` result (the
//!   structured shape any `tools/call` resolves to once the backend
//!   has produced its content).
//!
//! Operation-enum variants live in [`operations`](super::operations); the
//! method-name → operation router lives in [`routing`](super::routing).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::TaskAugmentParams;
use crate::descriptors::ToolDescriptor;
use crate::shared::content::ToolContent;

/// Parameters for `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    /// Task-augment opt-in (2025-11-25 only). The DRAFT-2026-v1
    /// tasks extension removes per-request opt-in; servers decide.
    #[serde(default)]
    pub task: Option<TaskAugmentParams>,
}

/// Result body for `tools/list`.
#[derive(Debug, Clone, Serialize)]
pub struct ToolsListResult {
    pub tools: Vec<ToolDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "nextCursor")]
    pub next_cursor: Option<String>,
    /// SEP-2549 caching hints. Emitted on the legacy wire as
    /// forward-compatible optional fields (ignorable by 2025-11-25
    /// clients) so caching-aware clients — including the conformance
    /// suite, which speaks 2025-11-25 over the SDK — get the same
    /// hints the modern wire surfaces.
    #[serde(skip_serializing_if = "Option::is_none", rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheScope")]
    pub cache_scope: Option<crate::shared::caching::CacheScope>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheToken")]
    pub cache_token: Option<String>,
}

/// Result body for `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_params_deserializes_meta() {
        let json = serde_json::json!({
            "name": "my_tool",
            "arguments": { "key": "val" },
            "_meta": { "org.paymentauth/credential": { "challenge": "abc" } }
        });
        let params: ToolCallParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.name, "my_tool");
        let meta = params.meta.expect("meta");
        assert!(meta.get("org.paymentauth/credential").is_some());
    }

    #[test]
    fn tool_call_params_without_meta_has_none() {
        let json = serde_json::json!({
            "name": "my_tool",
            "arguments": { "key": "val" }
        });
        let params: ToolCallParams = serde_json::from_value(json).unwrap();
        assert!(params.meta.is_none());
    }

    #[test]
    fn tool_call_result_serializes_meta() {
        let result = ToolCallResult {
            content: vec![ToolContent::text("ok")],
            structured_content: None,
            is_error: false,
            meta: Some(serde_json::json!({
                "org.paymentauth/receipt": { "status": "success" }
            })),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("_meta").is_some());
        assert_eq!(
            json["_meta"]["org.paymentauth/receipt"]["status"],
            "success"
        );
    }

    #[test]
    fn tool_call_result_without_meta_omits_field() {
        let result = ToolCallResult {
            content: vec![ToolContent::text("ok")],
            structured_content: None,
            is_error: false,
            meta: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("_meta").is_none());
    }

    #[test]
    fn tools_list_result_serializes_next_cursor() {
        let result = ToolsListResult {
            tools: vec![],
            next_cursor: Some("cursor_abc".to_owned()),
            ttl_ms: None,
            cache_scope: None,
            cache_token: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["nextCursor"], "cursor_abc");
    }

    #[test]
    fn tools_list_result_omits_null_cursor() {
        let result = ToolsListResult {
            tools: vec![],
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
            cache_token: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("nextCursor").is_none());
    }
}
