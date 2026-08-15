//! Modern `prompts/*` wire types for MCP revision `2026-07-28`.
//!
//! Surface today:
//! - `prompts/list` request + result, with the SEP-2549
//!   `CacheableResult` envelope (required `resultType` + `ttlMs` +
//!   `cacheScope`) at the result level and an optional per-entry
//!   `cacheScope` override on each [`PromptDescriptor`].
//! - `prompts/get` request + result.
//!
//! ## Differences from 2025-11-25
//!
//! - Result-level + per-entry cache fields (SEP-2549).
//! - `PromptDescriptor` is the per-version copy of the legacy
//!   shape — the field set matches today but keeping a separate
//!   module lets the modern prompt surface evolve (e.g., for
//!   MRTR-aware multi-message prompts) without disturbing the
//!   legacy contract.
//! - `PromptMessage.content` is the full shared [`ContentBlock`]
//!   ([`ToolContent`]) per the final schema (`PromptMessage.content`
//!   is a `ContentBlock`), so a prompt message can carry the same
//!   `name` / `annotations` / `size` / `icons` a tool-result content
//!   block carries — no per-version subset that silently drops those
//!   fields.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::caching::default_result_type_complete;
use crate::shared::content::{Icon, ToolContent};
use crate::v_2026_07_28::wire::tools::CacheScope;

// ---------------------------------------------------------------------------
// Method-name constants.
// ---------------------------------------------------------------------------

pub const METHOD_PROMPTS_LIST: &str = "prompts/list";
pub const METHOD_PROMPTS_GET: &str = "prompts/get";

// ---------------------------------------------------------------------------
// `prompts/list` shapes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptsListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result of `prompts/list`. A `CacheableResult` (SEP-2549):
/// `resultType` + `ttlMs` + `cacheScope` are required on the modern
/// wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptsListResult {
    /// SEP-2322 result-type discriminator. Always `"complete"`.
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    #[serde(default)]
    pub prompts: Vec<PromptDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// SEP-2549 cache lifetime (ms).
    pub ttl_ms: u64,
    /// SEP-2549 cache bucket.
    pub cache_scope: CacheScope,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl Default for PromptsListResult {
    fn default() -> Self {
        Self {
            result_type: default_result_type_complete(),
            prompts: Vec::new(),
            next_cursor: None,
            ttl_ms: crate::shared::caching::DEFAULT_LIST_TTL_MS,
            cache_scope: CacheScope::Public,
            meta: None,
        }
    }
}

/// Single prompt entry in the catalog page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptDescriptor {
    /// Machine-readable identifier used by `prompts/get#params.name`.
    pub name: String,
    /// Human-readable display title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One-paragraph operator-facing description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Ordered argument list. `None` ⇒ the prompt takes no
    /// arguments and `prompts/get` can be issued with empty
    /// `arguments`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
    /// Per-prompt branding icons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    /// Per-prompt cache scope override (SEP-2549). When present,
    /// MUST match the cacheability of this specific prompt; the
    /// page-level `cache_scope` covers everything else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<CacheScope>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// A single argument the prompt expects when `prompts/get` is
/// called.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptArgument {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// `Some(true)` ⇒ `prompts/get` MUST include this argument;
    /// `Some(false)` or `None` ⇒ optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

// ---------------------------------------------------------------------------
// `prompts/get` shapes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptGetParams {
    /// Prompt identifier (matches `PromptDescriptor.name`).
    pub name: String,
    /// String-valued arguments. Modern revision keeps the legacy
    /// `Map<String, String>` shape (not JSON Schema 2020-12 —
    /// that's tools-only per SEP-2106) so prompt authors don't
    /// re-spec.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Map<String, Value>>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
    /// SEP-2322 MRTR resumption — a prompt whose backend pipeline
    /// suspends for elicitation / sampling / roots returns an
    /// `InputRequiredResult`; the client re-issues `prompts/get`
    /// echoing the `requestState` blob alongside `inputResponses`.
    /// Top-level params fields per the spec, mirroring
    /// [`super::tools::ToolCallParams`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_state: Option<String>,
    /// Companion to [`Self::request_state`]. Keys match the
    /// `inputRequests` map the server emitted; values are the
    /// client's typed answers (or explicit `{error: ...}` envelopes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_responses: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptGetResult {
    /// Human-readable description of what the rendered prompt
    /// represents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Conversation messages the prompt resolves into.
    pub messages: Vec<PromptMessage>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// One conversation turn inside a rendered prompt.
///
/// Per the final schema `PromptMessage.content` is a full
/// `ContentBlock`, so the body is the shared [`ToolContent`] union —
/// the same content primitive tool results use, carrying
/// `name` / `annotations` / `size` / `icons` rather than a reduced
/// per-version subset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    /// `"user"` or `"assistant"` per MCP spec.
    pub role: String,
    pub content: ToolContent,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn method_constants_match_spec() {
        assert_eq!(METHOD_PROMPTS_LIST, "prompts/list");
        assert_eq!(METHOD_PROMPTS_GET, "prompts/get");
    }

    #[test]
    fn prompts_list_result_round_trip_with_cache_fields() {
        let r = PromptsListResult {
            result_type: default_result_type_complete(),
            prompts: vec![PromptDescriptor {
                name: "code_review".to_owned(),
                title: Some("Code Review".to_owned()),
                description: Some("Review a pull request".to_owned()),
                arguments: Some(vec![PromptArgument {
                    name: "pr_url".to_owned(),
                    title: None,
                    description: Some("URL of the PR to review".to_owned()),
                    required: Some(true),
                    meta: None,
                }]),
                icons: None,
                cache_scope: Some(CacheScope::Private),
                meta: None,
            }],
            next_cursor: None,
            ttl_ms: 300_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["prompts"][0]["name"], "code_review");
        assert_eq!(v["prompts"][0]["arguments"][0]["name"], "pr_url");
        assert_eq!(v["prompts"][0]["arguments"][0]["required"], true);
        assert_eq!(v["prompts"][0]["cacheScope"], "private");
        assert_eq!(v["ttlMs"], 300_000);
        assert_eq!(v["cacheScope"], "public");
        assert!(v.get("cacheToken").is_none());
        let back: PromptsListResult = serde_json::from_value(v).unwrap();
        assert_eq!(back.cache_scope, CacheScope::Public);
        assert_eq!(back.prompts[0].cache_scope, Some(CacheScope::Private));
    }

    #[test]
    fn prompts_list_result_revalidation_returns_empty_prompts() {
        let r = PromptsListResult {
            result_type: default_result_type_complete(),
            prompts: vec![],
            next_cursor: None,
            ttl_ms: 300_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["prompts"].as_array().unwrap().len(), 0);
        assert_eq!(v["cacheScope"], "public");
    }

    #[test]
    fn prompts_list_result_default_stamps_required_envelope() {
        let r = PromptsListResult::default();
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert!(v["prompts"].as_array().unwrap().is_empty());
        assert!(v.get("nextCursor").is_none());
        assert!(v["ttlMs"].is_u64());
        assert!(v.get("cacheScope").is_some());
        assert!(v.get("cacheToken").is_none());
    }

    #[test]
    fn prompt_descriptor_minimal_omits_optionals() {
        let p = PromptDescriptor {
            name: "x".to_owned(),
            title: None,
            description: None,
            arguments: None,
            icons: None,
            cache_scope: None,
            meta: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["name"], "x");
        assert!(v.get("title").is_none());
        assert!(v.get("arguments").is_none());
        assert!(v.get("cacheScope").is_none());
    }

    #[test]
    fn prompt_argument_optional_required_omitted() {
        let a = PromptArgument {
            name: "x".to_owned(),
            title: None,
            description: None,
            required: None,
            meta: None,
        };
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["name"], "x");
        assert!(v.get("required").is_none());
    }

    #[test]
    fn prompts_get_params_round_trip() {
        let p = PromptGetParams {
            name: "code_review".to_owned(),
            arguments: Some(
                [("pr_url".to_owned(), json!("https://example.com/pr/1"))]
                    .into_iter()
                    .collect(),
            ),
            meta: Some(json!({ "io.modelcontextprotocol/traceparent": "00-x-y-01" })),
            request_state: None,
            input_responses: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["name"], "code_review");
        assert_eq!(v["arguments"]["pr_url"], "https://example.com/pr/1");
        assert_eq!(
            v["_meta"]["io.modelcontextprotocol/traceparent"],
            "00-x-y-01"
        );
    }

    #[test]
    fn prompts_get_result_with_text_message() {
        let r = PromptGetResult {
            description: Some("Review this PR".to_owned()),
            messages: vec![PromptMessage {
                role: "user".to_owned(),
                content: ToolContent::Text {
                    text: "Please review the diff at <URL>.".to_owned(),
                    annotations: None,
                },
                meta: None,
            }],
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["description"], "Review this PR");
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["content"]["type"], "text");
        assert_eq!(
            v["messages"][0]["content"]["text"],
            "Please review the diff at <URL>."
        );
    }

    #[test]
    fn prompt_message_content_image_uses_camel_case_mime_type() {
        let c = ToolContent::Image {
            data: "AA==".to_owned(),
            mime_type: "image/png".to_owned(),
            annotations: None,
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["type"], "image");
        assert_eq!(v["mimeType"], "image/png");
    }

    #[test]
    fn prompt_message_content_is_full_content_block() {
        // CT-06: the prompt-message content is the full ContentBlock,
        // so a resource_link carries the tool-content field set
        // (`name` required, plus `size` / `icons`), not a reduced
        // per-version subset that drops them.
        let c = ToolContent::ResourceLink {
            uri: "mcpg://prompts/x".to_owned(),
            name: "x".to_owned(),
            title: None,
            description: None,
            mime_type: Some("application/json".to_owned()),
            annotations: None,
            size: Some(42),
            icons: Some(vec![Icon {
                src: "https://example.com/i.png".to_owned(),
                mime_type: None,
                sizes: None,
                theme: None,
            }]),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["type"], "resource_link");
        assert_eq!(v["uri"], "mcpg://prompts/x");
        assert_eq!(v["name"], "x");
        assert_eq!(v["size"], 42);
        assert_eq!(v["icons"][0]["src"], "https://example.com/i.png");
        let back: ToolContent = serde_json::from_value(v).unwrap();
        assert!(matches!(back, ToolContent::ResourceLink { .. }));
    }

    #[test]
    fn prompt_get_result_default_has_no_description() {
        let r = PromptGetResult::default();
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("description").is_none());
        assert!(v["messages"].as_array().unwrap().is_empty());
    }
}
