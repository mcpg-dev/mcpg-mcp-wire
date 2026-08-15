//! Resources wire types for MCP revision `2025-11-25`.
//!
//! Covers every type exchanged on the `resources/*` surface:
//!
//! - Request params: [`ResourceReadParams`], [`ResourceSubscribeParams`]
//!   (also used by `resources/unsubscribe`).
//! - Result bodies: [`ResourcesListResult`], [`ResourceReadResult`],
//!   [`ResourceTemplatesListResult`].
//! - Content variants: [`ResourceContents`] (untagged union of text
//!   vs. blob), [`ResourceTextContents`], [`BlobResourceContents`].
//! - URI templates: [`ResourceTemplate`].
//! - Server-pushed notifications: [`ResourceUpdatedNotification`]
//!   (+ params).
//!
//! The shared `ListChangedNotification` envelope used by tools /
//! prompts / resources alike lives in `wire::common`.
//!
//! Operation-enum variants live in [`operations`](super::operations); the
//! method-name → operation router lives in [`routing`](super::routing).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::descriptors::ResourceDescriptor;
use crate::shared::content::{ContentAnnotations, Icon};

// ---------------------------------------------------------------------------
// Request parameters.
// ---------------------------------------------------------------------------

/// Parameters for `resources/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReadParams {
    pub uri: String,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Parameters for `resources/subscribe` and `resources/unsubscribe`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSubscribeParams {
    pub uri: String,
}

// ---------------------------------------------------------------------------
// URI template (`resources/templates/list`).
// ---------------------------------------------------------------------------

/// Resource template descriptor per MCP 2025-11-25.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplate {
    pub uri_template: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "mimeType")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ContentAnnotations>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result for `resources/templates/list`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTemplatesListResult {
    pub resource_templates: Vec<ResourceTemplate>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "nextCursor")]
    pub next_cursor: Option<String>,
    /// SEP-2549 caching hints — see [`super::tools::ToolsListResult`].
    #[serde(skip_serializing_if = "Option::is_none", rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheScope")]
    pub cache_scope: Option<crate::shared::caching::CacheScope>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheToken")]
    pub cache_token: Option<String>,
}

// ---------------------------------------------------------------------------
// `notifications/resources/updated` server push.
// ---------------------------------------------------------------------------

/// JSON-RPC notification for resource updates, sent via SSE on
/// `GET /mcp` (or stdio's shared bidirectional channel).
#[derive(Debug, Clone, Serialize)]
pub struct ResourceUpdatedNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: ResourceUpdatedParams,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceUpdatedParams {
    pub uri: String,
}

// ---------------------------------------------------------------------------
// `resources/list` and `resources/read` result bodies.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ResourcesListResult {
    pub resources: Vec<ResourceDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "nextCursor")]
    pub next_cursor: Option<String>,
    /// SEP-2549 caching hints — see [`super::tools::ToolsListResult`].
    #[serde(skip_serializing_if = "Option::is_none", rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheScope")]
    pub cache_scope: Option<crate::shared::caching::CacheScope>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheToken")]
    pub cache_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceReadResult {
    pub contents: Vec<ResourceContents>,
    /// SEP-2549 caching hints — see [`super::tools::ToolsListResult`].
    #[serde(skip_serializing_if = "Option::is_none", rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheScope")]
    pub cache_scope: Option<crate::shared::caching::CacheScope>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheToken")]
    pub cache_token: Option<String>,
}

/// Resource content item — either text or blob. Per MCP 2025-11-25
/// spec the `contents` array may interleave both types; the untagged
/// serde representation matches the wire by inspecting whether `text`
/// or `blob` is present in the JSON object.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ResourceContents {
    Text(ResourceTextContents),
    Blob(BlobResourceContents),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceTextContents {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub text: String,
    /// Per-content `_meta` — notably SEP-1865 `_meta.ui` (the CSP /
    /// permissions / domain envelope a host folds into the iframe for a
    /// `ui://` resource). Carried through `resources/read` unchanged;
    /// the operator CSP/permission policy clamps it on egress.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Binary resource content (base64-encoded).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlobResourceContents {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub blob: String,
    /// Per-content `_meta` — see [`ResourceTextContents::meta`].
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_read_params_deserializes_meta() {
        let json = serde_json::json!({ "uri": "file:///x", "_meta": { "ctx": 1 } });
        let params: ResourceReadParams = serde_json::from_value(json).unwrap();
        assert!(params.meta.is_some());
    }

    #[test]
    fn resource_subscribe_params_round_trip() {
        let json = serde_json::json!({ "uri": "file:///project/config.json" });
        let params: ResourceSubscribeParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.uri, "file:///project/config.json");
        let back = serde_json::to_value(&params).unwrap();
        assert_eq!(back["uri"], "file:///project/config.json");
    }

    #[test]
    fn resource_template_emits_camel_case_field_names() {
        let template = ResourceTemplate {
            uri_template: "file:///{path}".to_owned(),
            name: "files".to_owned(),
            title: Some("File browser".to_owned()),
            description: None,
            mime_type: Some("text/plain".to_owned()),
            annotations: None,
            icons: None,
            meta: None,
        };
        let json = serde_json::to_value(&template).unwrap();
        assert_eq!(json["uriTemplate"], "file:///{path}");
        assert_eq!(json["mimeType"], "text/plain");
        assert_eq!(json["title"], "File browser");
        assert!(json.get("description").is_none(), "Option::None omitted");
    }

    #[test]
    fn resource_templates_list_omits_null_cursor() {
        let result = ResourceTemplatesListResult {
            resource_templates: vec![],
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
            cache_token: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("nextCursor").is_none());
    }

    #[test]
    fn resources_list_result_emits_next_cursor() {
        let result = ResourcesListResult {
            resources: vec![],
            next_cursor: Some("page2".to_owned()),
            ttl_ms: None,
            cache_scope: None,
            cache_token: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["nextCursor"], "page2");
    }

    #[test]
    fn resource_text_contents_serializes_with_camel_case() {
        let c = ResourceTextContents {
            uri: "file:///x".to_owned(),
            mime_type: Some("text/plain".to_owned()),
            text: "hi".to_owned(),
            meta: None,
        };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["uri"], "file:///x");
        assert_eq!(json["mimeType"], "text/plain");
        assert_eq!(json["text"], "hi");
    }

    #[test]
    fn blob_resource_contents_serializes_with_camel_case() {
        let c = BlobResourceContents {
            uri: "data://x".to_owned(),
            mime_type: Some("image/png".to_owned()),
            blob: "QUJD".to_owned(),
            meta: None,
        };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(json["mimeType"], "image/png");
        assert_eq!(json["blob"], "QUJD");
        assert!(json.get("text").is_none(), "text-side field absent on blob");
    }

    #[test]
    fn resource_contents_untagged_serializes_text_variant() {
        // The untagged representation means the JSON has no
        // discriminator key — text vs blob is distinguished by which
        // payload field appears in the object.
        let c = ResourceContents::Text(ResourceTextContents {
            uri: "file:///x".to_owned(),
            mime_type: None,
            text: "hi".to_owned(),
            meta: None,
        });
        let json = serde_json::to_value(&c).unwrap();
        assert!(json.get("text").is_some());
        assert!(json.get("blob").is_none());
        // No "type" tag at the outer level for an untagged union.
        assert!(json.get("type").is_none());
    }

    #[test]
    fn resource_contents_untagged_serializes_blob_variant() {
        let c = ResourceContents::Blob(BlobResourceContents {
            uri: "data://x".to_owned(),
            mime_type: Some("application/octet-stream".to_owned()),
            blob: "QUJD".to_owned(),
            meta: None,
        });
        let json = serde_json::to_value(&c).unwrap();
        assert!(json.get("blob").is_some());
        assert!(json.get("text").is_none());
        assert!(json.get("type").is_none());
    }

    #[test]
    fn resource_updated_notification_carries_uri() {
        let notif = ResourceUpdatedNotification {
            jsonrpc: "2.0",
            method: "notifications/resources/updated",
            params: ResourceUpdatedParams {
                uri: "file:///project/config.json".to_owned(),
            },
        };
        let json = serde_json::to_value(&notif).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "notifications/resources/updated");
        assert_eq!(json["params"]["uri"], "file:///project/config.json");
    }
}
