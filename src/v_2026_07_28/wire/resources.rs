//! Modern `resources/*` wire types for MCP revision `2026-07-28`.
//!
//! Surface today:
//! - `resources/list` request + result (with SEP-2549 cache triple).
//! - `resources/templates/list` request + result (with SEP-2549
//!   cache triple).
//! - `resources/read` request + result.
//!
//! ## Differences from 2025-11-25
//!
//! - **No subscribe / unsubscribe.** The legacy
//!   `resources/subscribe` + `resources/unsubscribe` methods and
//!   the matching `subscribe` sub-flag on `ResourcesCapability` are
//!   removed. The modern revision routes live update streams
//!   through `subscriptions/listen` — a single long-lived
//!   POST-SSE response per session that multiplexes every update
//!   type instead of one server-initiated request per resource.
//! - **No `notifications/resources/updated`.** Same reason —
//!   updates ride on the `subscriptions/listen` stream.
//! - **Cache fields on the list + read endpoints.** Required
//!   result-level `result_type` + `ttl_ms` + `cache_scope`
//!   (`CacheableResult`, SEP-2549/2322); per-entry `cache_scope`
//!   override on `ResourceDescriptor` for catalogs that mix
//!   per-tenant and global resources.
//!
//! ## Content shape reuse
//!
//! `ResourceContents` (text / blob variants) is duplicated from
//! the legacy module for per-version isolation, but the bytes are
//! identical today — `EmbeddedResource` and friends in
//! [`protocol::shared::content`](crate::shared::content)
//! continue to be the cross-version primitives.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::caching::default_result_type_complete;
use crate::shared::content::{ContentAnnotations, Icon};
use crate::v_2026_07_28::wire::tools::CacheScope;

// ---------------------------------------------------------------------------
// Method-name constants.
// ---------------------------------------------------------------------------

pub const METHOD_RESOURCES_LIST: &str = "resources/list";
pub const METHOD_RESOURCES_READ: &str = "resources/read";
pub const METHOD_RESOURCES_TEMPLATES_LIST: &str = "resources/templates/list";

// ---------------------------------------------------------------------------
// `resources/list` shapes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result of `resources/list`. A `CacheableResult` (SEP-2549):
/// `resultType` + `ttlMs` + `cacheScope` are required on the modern
/// wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesListResult {
    /// SEP-2322 result-type discriminator. Always `"complete"`.
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    #[serde(default)]
    pub resources: Vec<ResourceDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// SEP-2549 cache lifetime (ms).
    pub ttl_ms: u64,
    /// SEP-2549 cache bucket.
    pub cache_scope: CacheScope,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl Default for ResourcesListResult {
    fn default() -> Self {
        Self {
            result_type: default_result_type_complete(),
            resources: Vec::new(),
            next_cursor: None,
            ttl_ms: crate::shared::caching::DEFAULT_LIST_TTL_MS,
            cache_scope: CacheScope::Public,
            meta: None,
        }
    }
}

/// Single resource entry in the catalog page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDescriptor {
    /// Absolute URI the client uses on `resources/read#params.uri`.
    pub uri: String,
    /// Machine-readable identifier.
    pub name: String,
    /// Human-readable display title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// One-paragraph operator-facing description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// IANA media type the resource exposes when read. `None` ⇒
    /// the resource is heterogeneous or self-describing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Byte size of the resource body, if known. `None` ⇒ unknown
    /// or computed on demand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// Per-resource branding icons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    /// Spec resource annotations (`audience` / `priority` /
    /// `lastModified`). Carried through from the backend descriptor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ContentAnnotations>,
    /// Per-resource cache scope override (SEP-2549).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<CacheScope>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

// ---------------------------------------------------------------------------
// `resources/templates/list` shapes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplatesListParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result of `resources/templates/list`. A `CacheableResult`
/// (SEP-2549) — required `resultType` + `ttlMs` + `cacheScope`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplatesListResult {
    /// SEP-2322 result-type discriminator. Always `"complete"`.
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    #[serde(default)]
    pub resource_templates: Vec<ResourceTemplate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    pub ttl_ms: u64,
    pub cache_scope: CacheScope,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl Default for ResourceTemplatesListResult {
    fn default() -> Self {
        Self {
            result_type: default_result_type_complete(),
            resource_templates: Vec::new(),
            next_cursor: None,
            ttl_ms: crate::shared::caching::DEFAULT_LIST_TTL_MS,
            cache_scope: CacheScope::Public,
            meta: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    /// RFC-6570 URI template (e.g., `"db://users/{user_id}"`).
    pub uri_template: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    /// Spec resource annotations (`audience` / `priority` /
    /// `lastModified`). Carried through from the backend template.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ContentAnnotations>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

// ---------------------------------------------------------------------------
// `resources/read` shapes.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceReadParams {
    /// Absolute URI (matches a `ResourceDescriptor.uri` or expands
    /// a `ResourceTemplate.uri_template`).
    pub uri: String,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result of `resources/read`. A `CacheableResult` (SEP-2549/2322) —
/// required `resultType` + `ttlMs` + `cacheScope`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceReadResult {
    /// SEP-2322 result-type discriminator. Always `"complete"`.
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    /// One or more content fragments. A single-resource read MAY
    /// return multiple fragments when the resource is structurally
    /// composite (e.g., a folder).
    #[serde(default)]
    pub contents: Vec<ResourceContents>,
    /// SEP-2549 cache lifetime (ms).
    pub ttl_ms: u64,
    /// SEP-2549 cache bucket.
    pub cache_scope: CacheScope,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl Default for ResourceReadResult {
    fn default() -> Self {
        Self {
            result_type: default_result_type_complete(),
            contents: Vec::new(),
            ttl_ms: crate::shared::caching::DEFAULT_READ_TTL_MS,
            cache_scope: CacheScope::Public,
            meta: None,
        }
    }
}

/// Tagged union: a resource fragment is either text or binary
/// (base64). Discriminated by whether `text` or `blob` is set, not
/// by an explicit tag — this matches the spec's "exactly one of
/// `text` / `blob`" shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceContents {
    Text(ResourceTextContents),
    Blob(BlobResourceContents),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTextContents {
    pub uri: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContents {
    pub uri: String,
    /// Base64-encoded bytes.
    pub blob: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn method_constants_match_spec() {
        assert_eq!(METHOD_RESOURCES_LIST, "resources/list");
        assert_eq!(METHOD_RESOURCES_READ, "resources/read");
        assert_eq!(METHOD_RESOURCES_TEMPLATES_LIST, "resources/templates/list");
    }

    #[test]
    fn resources_list_result_round_trip_with_cache_fields() {
        let r = ResourcesListResult {
            result_type: default_result_type_complete(),
            resources: vec![ResourceDescriptor {
                uri: "mcpg://runtime/overview".to_owned(),
                name: "runtime_overview".to_owned(),
                title: Some("Runtime Overview".to_owned()),
                description: Some("Current runtime snapshot".to_owned()),
                mime_type: Some("application/json".to_owned()),
                size: Some(2048),
                icons: None,
                annotations: Some(ContentAnnotations {
                    audience: Some(vec!["assistant".to_owned()]),
                    priority: Some(0.9),
                    last_modified: None,
                }),
                cache_scope: Some(CacheScope::Private),
                meta: None,
            }],
            next_cursor: Some("page-2".to_owned()),
            ttl_ms: 30_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["resources"][0]["uri"], "mcpg://runtime/overview");
        assert_eq!(v["resources"][0]["mimeType"], "application/json");
        assert_eq!(v["resources"][0]["size"], 2048);
        assert_eq!(v["resources"][0]["cacheScope"], "private");
        // RES-08: annotations are carried on the descriptor.
        assert_eq!(v["resources"][0]["annotations"]["priority"], 0.9);
        assert_eq!(v["resources"][0]["annotations"]["audience"][0], "assistant");
        assert_eq!(v["ttlMs"], 30_000);
        assert_eq!(v["cacheScope"], "public");
        assert!(v.get("cacheToken").is_none());
        let back: ResourcesListResult = serde_json::from_value(v).unwrap();
        assert_eq!(back.cache_scope, CacheScope::Public);
        assert_eq!(back.resources[0].size, Some(2048));
        assert_eq!(
            back.resources[0].annotations.as_ref().unwrap().priority,
            Some(0.9)
        );
    }

    #[test]
    fn resources_list_result_revalidation_emits_empty() {
        let r = ResourcesListResult {
            result_type: default_result_type_complete(),
            resources: vec![],
            next_cursor: None,
            ttl_ms: 30_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resources"].as_array().unwrap().len(), 0);
        assert_eq!(v["cacheScope"], "public");
    }

    #[test]
    fn resources_list_result_default_stamps_required_envelope() {
        let r = ResourcesListResult::default();
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert!(v["resources"].as_array().unwrap().is_empty());
        assert!(v.get("nextCursor").is_none());
        assert!(v["ttlMs"].is_u64());
        assert!(v.get("cacheScope").is_some());
        assert!(v.get("cacheToken").is_none());
    }

    #[test]
    fn resource_descriptor_minimal_omits_optionals() {
        let d = ResourceDescriptor {
            uri: "u".to_owned(),
            name: "n".to_owned(),
            title: None,
            description: None,
            mime_type: None,
            size: None,
            icons: None,
            annotations: None,
            cache_scope: None,
            meta: None,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["uri"], "u");
        assert_eq!(v["name"], "n");
        assert!(v.get("title").is_none());
        assert!(v.get("mimeType").is_none());
        assert!(v.get("size").is_none());
        assert!(v.get("cacheScope").is_none());
    }

    #[test]
    fn resource_templates_list_result_round_trip() {
        let r = ResourceTemplatesListResult {
            result_type: default_result_type_complete(),
            resource_templates: vec![ResourceTemplate {
                uri_template: "db://users/{user_id}".to_owned(),
                name: "user".to_owned(),
                title: Some("User Record".to_owned()),
                description: None,
                mime_type: Some("application/json".to_owned()),
                icons: None,
                annotations: None,
                meta: None,
            }],
            next_cursor: None,
            ttl_ms: 60_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert_eq!(
            v["resourceTemplates"][0]["uriTemplate"],
            "db://users/{user_id}"
        );
        assert_eq!(v["resourceTemplates"][0]["mimeType"], "application/json");
        assert_eq!(v["ttlMs"], 60_000);
        assert!(v.get("cacheToken").is_none());
        let back: ResourceTemplatesListResult = serde_json::from_value(v).unwrap();
        assert_eq!(back.resource_templates[0].name, "user");
    }

    #[test]
    fn resource_read_params_round_trip() {
        let p = ResourceReadParams {
            uri: "mcpg://x".to_owned(),
            meta: Some(json!({ "io.modelcontextprotocol/traceparent": "00-a-b-01" })),
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["uri"], "mcpg://x");
        assert_eq!(
            v["_meta"]["io.modelcontextprotocol/traceparent"],
            "00-a-b-01"
        );
    }

    #[test]
    fn resource_read_result_with_text_contents() {
        let r = ResourceReadResult {
            result_type: default_result_type_complete(),
            contents: vec![ResourceContents::Text(ResourceTextContents {
                uri: "mcpg://x".to_owned(),
                text: "hello".to_owned(),
                mime_type: Some("text/plain".to_owned()),
                meta: None,
            })],
            ttl_ms: 30_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["ttlMs"], 30_000);
        assert_eq!(v["cacheScope"], "public");
        assert_eq!(v["contents"][0]["uri"], "mcpg://x");
        assert_eq!(v["contents"][0]["text"], "hello");
        assert_eq!(v["contents"][0]["mimeType"], "text/plain");
        // No `blob` field on the text variant.
        assert!(v["contents"][0].get("blob").is_none());
    }

    #[test]
    fn resource_read_result_with_blob_contents() {
        let r = ResourceReadResult {
            result_type: default_result_type_complete(),
            contents: vec![ResourceContents::Blob(BlobResourceContents {
                uri: "mcpg://img".to_owned(),
                blob: "AA==".to_owned(),
                mime_type: Some("image/png".to_owned()),
                meta: None,
            })],
            ttl_ms: 30_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["contents"][0]["uri"], "mcpg://img");
        assert_eq!(v["contents"][0]["blob"], "AA==");
        assert_eq!(v["contents"][0]["mimeType"], "image/png");
        assert!(v["contents"][0].get("text").is_none());
    }

    #[test]
    fn resource_contents_untagged_round_trip_distinguishes_variants() {
        // Untagged enum: serde picks the variant by which fields
        // are present. `text` field ⇒ Text; `blob` field ⇒ Blob.
        let text_value = json!({ "uri": "u", "text": "hi", "mimeType": "text/plain" });
        let parsed: ResourceContents = serde_json::from_value(text_value).unwrap();
        assert!(matches!(parsed, ResourceContents::Text(_)));

        let blob_value = json!({ "uri": "u", "blob": "AA==", "mimeType": "image/png" });
        let parsed: ResourceContents = serde_json::from_value(blob_value).unwrap();
        assert!(matches!(parsed, ResourceContents::Blob(_)));
    }

    #[test]
    fn resource_read_result_default_stamps_required_envelope() {
        let r = ResourceReadResult::default();
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert!(v["contents"].as_array().unwrap().is_empty());
        assert!(v["ttlMs"].is_u64());
        assert!(v.get("cacheScope").is_some());
        assert!(v.get("_meta").is_none());
    }
}
