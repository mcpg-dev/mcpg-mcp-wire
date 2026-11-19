//! `resources/read` params and result, and the resource template
//! descriptor, as the gateway runtime handles them.
//!
//! - [`ResourceReadParams`] / [`ResourceReadResult`].
//! - [`ResourceContents`] — untagged union of [`ResourceTextContents`]
//!   and [`BlobResourceContents`].
//! - [`ResourceTemplate`] — one `resources/templates/list` entry.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::content::{ContentAnnotations, Icon};

/// Parameters for `resources/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceReadParams {
    pub uri: String,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Resource template descriptor (`resources/templates/list`).
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

/// Result body for `resources/read`.
#[derive(Debug, Clone, Serialize)]
pub struct ResourceReadResult {
    pub contents: Vec<ResourceContents>,
    /// SEP-2549 cache lifetime (ms).
    #[serde(skip_serializing_if = "Option::is_none", rename = "ttlMs")]
    pub ttl_ms: Option<u64>,
    /// SEP-2549 cache bucket.
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheScope")]
    pub cache_scope: Option<crate::shared::caching::CacheScope>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "cacheToken")]
    pub cache_token: Option<String>,
}

/// Resource content item — either text or blob. The `contents` array
/// may interleave both; the untagged representation tells them apart by
/// whether `text` or `blob` is present.
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
    use serde_json::json;

    #[test]
    fn resource_read_params_deserializes_meta() {
        let params: ResourceReadParams =
            serde_json::from_value(json!({ "uri": "file:///x", "_meta": { "ctx": 1 } })).unwrap();
        assert_eq!(params.uri, "file:///x");
        assert!(params.meta.is_some());
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
    fn resource_text_contents_serializes_with_camel_case() {
        let c = ResourceTextContents {
            uri: "file:///x".to_owned(),
            mime_type: Some("text/plain".to_owned()),
            text: "hi".to_owned(),
            meta: None,
        };
        let json = serde_json::to_value(&c).unwrap();
        assert_eq!(
            json,
            json!({ "uri": "file:///x", "mimeType": "text/plain", "text": "hi" })
        );
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
    fn resource_contents_untagged_serializes_both_variants() {
        let text = ResourceContents::Text(ResourceTextContents {
            uri: "file:///x".to_owned(),
            mime_type: None,
            text: "hi".to_owned(),
            meta: None,
        });
        let json = serde_json::to_value(&text).unwrap();
        assert!(json.get("text").is_some());
        assert!(json.get("blob").is_none());
        assert!(
            json.get("type").is_none(),
            "no outer tag on an untagged union"
        );

        let blob = ResourceContents::Blob(BlobResourceContents {
            uri: "data://x".to_owned(),
            mime_type: Some("application/octet-stream".to_owned()),
            blob: "QUJD".to_owned(),
            meta: None,
        });
        let json = serde_json::to_value(&blob).unwrap();
        assert!(json.get("blob").is_some());
        assert!(json.get("text").is_none());
        assert!(json.get("type").is_none());
    }

    #[test]
    fn resource_read_result_carries_cache_hints_only_when_set() {
        let bare = ResourceReadResult {
            contents: vec![],
            ttl_ms: None,
            cache_scope: None,
            cache_token: None,
        };
        assert_eq!(
            serde_json::to_value(&bare).unwrap(),
            json!({ "contents": [] })
        );
        let cached = ResourceReadResult {
            contents: vec![],
            ttl_ms: Some(30_000),
            cache_scope: Some(crate::shared::caching::CacheScope::Private),
            cache_token: Some("t".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(&cached).unwrap(),
            json!({ "contents": [], "ttlMs": 30_000, "cacheScope": "private", "cacheToken": "t" })
        );
    }
}
