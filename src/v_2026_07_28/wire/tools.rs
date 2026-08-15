//! Modern `tools/*` wire types for MCP revision `2026-07-28`.
//!
//! Surface today:
//! - `tools/list` request + result, with the SEP-2549
//!   `CacheableResult` envelope (required `resultType` + `ttlMs` +
//!   `cacheScope`) at the result level plus an optional per-entry
//!   `cacheScope` hint.
//! - `tools/call` request + result for the **complete** path
//!   (the call ran to completion and returned content + optional
//!   structured payload).
//!
//! Not yet here:
//! - **MRTR `InputRequiredResult`** — the modern shape of a
//!   suspended tools/call. Lives in
//!   [`v_2026_07_28::wire::mrtr`](super::mrtr) and is selected via a
//!   top-level result-type discriminator the handler unions in.
//! - **Per-tool task augmentation** — the `task` opt-in on
//!   `tools/call` was removed by SEP-2663 in the modern revision
//!   (the server now decides per-call whether to surface a task).
//!
//! ## Cache surface (SEP-2549)
//!
//! The modern `tools/list` result is a `CacheableResult`: it carries
//! a required `resultType` (always `"complete"` on this path), a
//! required `ttlMs`, and a required `cacheScope`. The client caches
//! the result for up to `ttlMs` milliseconds bucketed by
//! `cacheScope`.
//!
//! `CacheScope` is exposed as a typed enum; unknown strings fail
//! deserialization so a misspelled scope never silently degrades
//! to "no cache".

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::caching::default_result_type_complete;
use crate::shared::content::{Icon, ToolContent};

// ---------------------------------------------------------------------------
// Method-name constants.
// ---------------------------------------------------------------------------

/// `tools/list` JSON-RPC method name.
pub const METHOD_TOOLS_LIST: &str = "tools/list";

/// `tools/call` JSON-RPC method name.
pub const METHOD_TOOLS_CALL: &str = "tools/call";

// ---------------------------------------------------------------------------
// Cache scope enum.
// ---------------------------------------------------------------------------

/// SEP-2549 cache scope. Determines how a client should bucket
/// a cached list result.
///
/// Unknown strings fail deserialization (rather than masquerading
/// as `Global` or `None`) so a misspelled `cacheScope` in a
/// hand-tuned manifest doesn't silently change cache semantics.
// SEP-2549 cache scope is a version-agnostic caching primitive; it
// lives in `protocol::shared::caching` so the legacy wire can emit
// the same vocabulary for forward-compatible clients. Re-exported
// here so existing `wire::tools::CacheScope` imports stay valid.
pub use crate::shared::caching::CacheScope;

// ---------------------------------------------------------------------------
// `tools/list` shapes.
// ---------------------------------------------------------------------------

/// Parameters for `tools/list`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListParams {
    /// Pagination cursor returned from a prior `tools/list`. Opaque
    /// to the client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    /// `_meta` — typically carries
    /// `io.modelcontextprotocol/cacheToken` on revalidating
    /// requests.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result of `tools/list`. A `CacheableResult` (SEP-2549): the
/// `resultType` + `ttlMs` + `cacheScope` fields are required on the
/// modern wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsListResult {
    /// SEP-2322 result-type discriminator. Always `"complete"` on
    /// the `tools/list` complete path.
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    /// The tool catalog page.
    #[serde(default)]
    pub tools: Vec<ToolDescriptor>,
    /// Cursor for the next page. `None` ⇒ no more pages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// SEP-2549 cache lifetime in milliseconds.
    pub ttl_ms: u64,
    /// SEP-2549 cache bucket.
    pub cache_scope: CacheScope,
    /// `_meta` for forward-compat (`io.modelcontextprotocol/*`).
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl Default for ToolsListResult {
    fn default() -> Self {
        Self {
            result_type: default_result_type_complete(),
            tools: Vec::new(),
            next_cursor: None,
            ttl_ms: crate::shared::caching::DEFAULT_LIST_TTL_MS,
            cache_scope: CacheScope::Public,
            meta: None,
        }
    }
}

/// A single tool in the `tools/list` catalog page.
///
/// `name`, `inputSchema`, `outputSchema` follow JSON Schema 2020-12
/// per SEP-2106 (modern revision tightened the constraint).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDescriptor {
    /// Machine-readable identifier. MUST match the
    /// `[a-zA-Z0-9_-]+` charset and is the value used in
    /// `tools/call#params.name`.
    pub name: String,
    /// Human-readable display title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One-paragraph operator-facing description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema 2020-12 for the tool's arguments.
    pub input_schema: Value,
    /// JSON Schema 2020-12 for the structured result body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    /// Per-tool branding icons (same `Icon` shape as 2025-11-25).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    /// Per-tool cache scope override. When present, the server is
    /// signaling that this tool's cacheability differs from the
    /// page-level `cache_scope`. Useful for catalogs that mix
    /// global and tenant-scoped tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<CacheScope>,
    /// Server-emitted hints about the tool's behavior (read-only,
    /// destructive, idempotent, etc.). The exact key set is
    /// extensible; MCPG forwards them verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
    /// `_meta` for forward-compat per-entry signals.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

// ---------------------------------------------------------------------------
// `tools/call` shapes.
// ---------------------------------------------------------------------------

/// Parameters for `tools/call`. The modern revision dropped the
/// per-call `task` opt-in (the server decides per-call whether to
/// surface a task via the tasks extension).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallParams {
    /// Tool identifier (must match a `ToolDescriptor.name` from
    /// `tools/list`).
    pub name: String,
    /// Arguments object. Validated against
    /// `ToolDescriptor.input_schema` before dispatch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
    /// `_meta` — carries `progressToken`, `traceparent`, etc.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    /// SEP-2322 MRTR resumption — client echoes back the
    /// `requestState` blob from the prior `InputRequiredResult`
    /// alongside `inputResponses` keyed by the same correlation
    /// tokens the server emitted. Top-level params field per the
    /// spec, not under `_meta`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_state: Option<String>,
    /// Companion to [`Self::request_state`]. Keys match the
    /// `inputRequests` map; values are the client's typed
    /// answers (or explicit `{error: ...}` envelopes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_responses: Option<Value>,
}

/// Complete-path result of `tools/call`. When the call suspends
/// for input the handler returns the MRTR `InputRequiredResult`
/// shape instead — that result lives in a sibling `mrtr.rs` file
/// because its shape differs.
///
/// ## `resultType` discriminator (SEP-2663)
///
/// The modern `tools/call` result is one of three shapes
/// discriminated by `resultType`:
/// - `"complete"` — this struct. The call ran to completion and
///   the body carries `content` + optional structured data.
/// - `"input_required"` — MRTR suspension. Wire shape lives in
///   [`v_2026_07_28::wire::mrtr::InputRequiredResult`].
/// - `"task"` — the server elected async execution; the result is a
///   [`CreateTaskResult`](crate::v_2026_07_28::extensions::tasks::wire::CreateTaskResult)
///   (`Result & Task`, flat) instead of content. Task creation is
///   server-directed (SEP-2663): there is no client `createTask`; the
///   server decides per-request whether a `tools/call` materializes a
///   task, and MUST do so only for a client that declared the
///   `io.modelcontextprotocol/tasks` extension. The discriminator is
///   defined here so the wire vocabulary lives in one place. The live
///   background materialization is staged separately from this wire
///   seam (see the tasks extension module).
///
/// `result_type` is `Option<String>` so MCPG's complete-path
/// responses can omit it; clients that consult it MUST default to
/// `"complete"` on absence per SEP-2663.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    /// SEP-2663 result-type discriminator. `None` ⇒ omit on the
    /// wire; clients default to `"complete"`. See module doc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_type: Option<String>,
    /// Ordered content blocks (text / image / audio / resource /
    /// resource_link). Same `ToolContent` enum as 2025-11-25
    /// because content shapes are version-stable.
    #[serde(default)]
    pub content: Vec<ToolContent>,
    /// Structured payload validated against
    /// `ToolDescriptor.output_schema`. `None` ⇒ no structured
    /// data; `content` is the only result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    /// `true` ⇒ the tool ran to completion but reported an
    /// application-level error. The client should surface the
    /// `content` payload as the error message. (Spec-level errors
    /// — bad arguments, schema mismatch, denied policy — flow
    /// through the JSON-RPC `error` envelope, not this flag.)
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
    /// `_meta` for forward-compat.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Discriminator constants for the modern `tools/call` result.
/// See [`ToolCallResult`] for the SEP-2663 rationale.
pub const RESULT_TYPE_COMPLETE: &str = "complete";
/// Discriminator value for MRTR suspensions. Defined alongside
/// [`RESULT_TYPE_COMPLETE`] here even though
/// [`InputRequiredResult`](super::mrtr::InputRequiredResult) owns
/// the actual struct — so the full discriminator vocabulary lives
/// in one place for spec audits.
pub const RESULT_TYPE_INPUT_REQUIRED: &str = "input_required";
/// Discriminator value for the server-directed task-materialization
/// shape (`CreateTaskResult`). See [`ToolCallResult`] for the model.
pub const RESULT_TYPE_TASK: &str = "task";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::content::ToolContent;
    use serde_json::json;

    #[test]
    fn method_constants_match_spec() {
        assert_eq!(METHOD_TOOLS_LIST, "tools/list");
        assert_eq!(METHOD_TOOLS_CALL, "tools/call");
    }

    #[test]
    fn cache_scope_serializes_lowercase() {
        for (scope, expected) in [
            (CacheScope::Public, "public"),
            (CacheScope::Private, "private"),
        ] {
            let json = serde_json::to_value(scope).unwrap();
            assert_eq!(json, expected);
            let back: CacheScope = serde_json::from_value(json).unwrap();
            assert_eq!(back, scope);
        }
    }

    #[test]
    fn cache_scope_rejects_unknown_value() {
        let result: Result<CacheScope, _> = serde_json::from_value(json!("forever"));
        assert!(
            result.is_err(),
            "unknown cache scope must fail deserialization, not silently coerce"
        );
    }

    #[test]
    fn tools_list_params_default_is_empty() {
        let params = ToolsListParams::default();
        let v = serde_json::to_value(&params).unwrap();
        assert!(v.as_object().unwrap().is_empty(), "got: {v}");
    }

    #[test]
    fn tools_list_params_with_cursor_and_cache_token_meta() {
        let params = ToolsListParams {
            cursor: Some("page-2".to_owned()),
            meta: Some(json!({
                "io.modelcontextprotocol/cacheToken": "etag-xyz"
            })),
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["cursor"], "page-2");
        assert_eq!(v["_meta"]["io.modelcontextprotocol/cacheToken"], "etag-xyz");
    }

    #[test]
    fn tools_list_result_full_round_trip() {
        let r = ToolsListResult {
            result_type: default_result_type_complete(),
            tools: vec![ToolDescriptor {
                name: "search".to_owned(),
                title: Some("Search".to_owned()),
                description: Some("Full-text search".to_owned()),
                input_schema: json!({ "type": "object" }),
                output_schema: Some(json!({ "type": "object" })),
                icons: None,
                cache_scope: Some(CacheScope::Private),
                annotations: Some(json!({ "readOnlyHint": true })),
                meta: None,
            }],
            next_cursor: Some("page-3".to_owned()),
            ttl_ms: 60_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["tools"][0]["name"], "search");
        assert_eq!(v["tools"][0]["cacheScope"], "private");
        assert_eq!(v["tools"][0]["annotations"]["readOnlyHint"], true);
        assert_eq!(v["nextCursor"], "page-3");
        assert_eq!(v["ttlMs"], 60_000);
        assert_eq!(v["cacheScope"], "public");
        // The non-spec `cacheToken` field is gone (VN-3).
        assert!(v.get("cacheToken").is_none());
        // Round-trip back.
        let back: ToolsListResult = serde_json::from_value(v).unwrap();
        assert_eq!(back.cache_scope, CacheScope::Public);
        assert_eq!(back.tools[0].cache_scope, Some(CacheScope::Private));
    }

    #[test]
    fn tools_list_result_revalidation_emits_empty_tools() {
        // Cache-revalidation hit: server MAY return an empty tools
        // page; the client keeps its cached copy.
        let r = ToolsListResult {
            result_type: default_result_type_complete(),
            tools: vec![],
            next_cursor: None,
            ttl_ms: 60_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["tools"].as_array().unwrap().len(), 0);
        assert_eq!(v["cacheScope"], "public");
    }

    #[test]
    fn tools_list_result_default_stamps_required_envelope() {
        let r = ToolsListResult::default();
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert!(v["tools"].as_array().unwrap().is_empty());
        assert!(v.get("nextCursor").is_none());
        // ttlMs + cacheScope are required (CacheableResult) — always
        // serialized.
        assert!(v["ttlMs"].is_u64());
        assert!(v.get("cacheScope").is_some());
        assert!(v.get("cacheToken").is_none());
        assert!(v.get("_meta").is_none());
    }

    #[test]
    fn tool_descriptor_minimal_omits_optionals() {
        let t = ToolDescriptor {
            name: "x".to_owned(),
            title: None,
            description: None,
            input_schema: json!({}),
            output_schema: None,
            icons: None,
            cache_scope: None,
            annotations: None,
            meta: None,
        };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["name"], "x");
        assert!(v.get("title").is_none());
        assert!(v.get("description").is_none());
        assert!(v.get("outputSchema").is_none());
        assert!(v.get("cacheScope").is_none());
    }

    #[test]
    fn tools_call_params_round_trip() {
        let p = ToolCallParams {
            name: "search".to_owned(),
            arguments: Some(json!({ "q": "hello" })),
            meta: Some(json!({ "io.modelcontextprotocol/progressToken": "p1" })),
            request_state: None,
            input_responses: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["name"], "search");
        assert_eq!(v["arguments"]["q"], "hello");
        assert_eq!(v["_meta"]["io.modelcontextprotocol/progressToken"], "p1");
        let back: ToolCallParams = serde_json::from_value(v).unwrap();
        assert_eq!(back.name, "search");
    }

    #[test]
    fn tools_call_result_complete_path() {
        let r = ToolCallResult {
            result_type: None,
            content: vec![ToolContent::text("ok")],
            structured_content: Some(json!({ "answer": 42 })),
            is_error: false,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["content"][0]["type"], "text");
        assert_eq!(v["content"][0]["text"], "ok");
        assert_eq!(v["structuredContent"]["answer"], 42);
        // is_error: false omitted by `std::ops::Not::not` skip.
        assert!(v.get("isError").is_none());
        // resultType omitted on the wire when None — clients
        // default to "complete" per SEP-2663.
        assert!(v.get("resultType").is_none());
    }

    #[test]
    fn tools_call_result_error_path_emits_is_error_true() {
        let r = ToolCallResult {
            result_type: None,
            content: vec![ToolContent::text("backend timed out")],
            structured_content: None,
            is_error: true,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["isError"], true);
        assert_eq!(v["content"][0]["text"], "backend timed out");
    }

    #[test]
    fn tools_call_result_emits_result_type_when_explicitly_set() {
        let r = ToolCallResult {
            result_type: Some(RESULT_TYPE_COMPLETE.to_owned()),
            content: vec![],
            structured_content: None,
            is_error: false,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
    }

    #[test]
    fn result_type_constants_match_spec_strings() {
        assert_eq!(RESULT_TYPE_COMPLETE, "complete");
        assert_eq!(RESULT_TYPE_INPUT_REQUIRED, "input_required");
        assert_eq!(RESULT_TYPE_TASK, "task");
    }
}
