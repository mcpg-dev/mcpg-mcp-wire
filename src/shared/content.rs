//! Content block types shared across MCP protocol revisions.
//!
//! [`ToolContent`], [`ContentAnnotations`], [`Icon`], and
//! [`EmbeddedResource`] are part of the wire format both `2025-11-25`
//! and `DRAFT-2026-v1` versions use verbatim. Putting them here lets
//! the per-version wire modules import from
//! `crate::shared::content` rather than redefining
//! identical shapes.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Content annotations per MCP spec — hints about content purpose
/// and importance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentAnnotations {
    /// Who this content is intended for (e.g. `["user"]`, `["assistant"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<String>>,
    /// Relative importance hint (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<f64>,
    /// ISO 8601 timestamp of when the annotated content was last modified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
}

/// Icon descriptor per MCP spec.
/// Fields: `src` (required URI), optional `mimeType`, `sizes`, `theme`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Icon {
    /// URI pointing to the icon resource (HTTP / HTTPS or `data:` URI).
    pub src: String,
    /// MIME type override (e.g. `"image/png"`, `"image/svg+xml"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Sizes at which the icon is available (e.g. `["48x48", "96x96"]`
    /// or `["any"]`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sizes: Option<Vec<String>>,
    /// Theme the icon is designed for (`"light"` or `"dark"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

/// Embedded resource block, used in tool results, prompt messages, and
/// sampling messages.
///
/// Per the MCP schema the embedded `resource` is
/// `TextResourceContents | BlobResourceContents` — i.e. **exactly one
/// of** `text` / `blob` is present. The struct keeps both as
/// `Option` so the on-the-wire field set is byte-identical across
/// revisions, but [`Deserialize`] enforces the one-of: a payload that
/// carries neither (or both) is rejected. The serialized form is
/// unchanged (`skip_serializing_if` drops the absent member).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResource {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<ContentAnnotations>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl EmbeddedResource {
    /// Whether this block satisfies the spec's exactly-one-of(text|blob)
    /// invariant.
    pub fn is_well_formed(&self) -> bool {
        self.text.is_some() ^ self.blob.is_some()
    }
}

impl<'de> Deserialize<'de> for EmbeddedResource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Shadow struct mirrors the field set / renames so the wire
        // shape is parsed identically; the one-of check runs after.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Shadow {
            uri: String,
            #[serde(default)]
            mime_type: Option<String>,
            #[serde(default)]
            text: Option<String>,
            #[serde(default)]
            blob: Option<String>,
            #[serde(default)]
            annotations: Option<ContentAnnotations>,
            #[serde(rename = "_meta", default)]
            meta: Option<Value>,
        }
        let s = Shadow::deserialize(deserializer)?;
        let resource = EmbeddedResource {
            uri: s.uri,
            mime_type: s.mime_type,
            text: s.text,
            blob: s.blob,
            annotations: s.annotations,
            meta: s.meta,
        };
        if !resource.is_well_formed() {
            return Err(serde::de::Error::custom(
                "embedded resource MUST carry exactly one of `text` or `blob`",
            ));
        }
        Ok(resource)
    }
}

/// Tool result content block: text, image, audio, embedded resource,
/// or a resource link. The tagged-union shape (`type` discriminator
/// on the wire) is identical across MCP revisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolContent {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations: Option<ContentAnnotations>,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations: Option<ContentAnnotations>,
    },
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations: Option<ContentAnnotations>,
    },
    Resource {
        resource: EmbeddedResource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations: Option<ContentAnnotations>,
    },
    #[serde(rename = "resource_link")]
    ResourceLink {
        uri: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "mimeType")]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        annotations: Option<ContentAnnotations>,
        #[serde(skip_serializing_if = "Option::is_none")]
        size: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        icons: Option<Vec<Icon>>,
    },
}

impl ToolContent {
    /// Convenience constructor for text content (most common in
    /// gateway-generated responses).
    pub fn text(text: impl Into<String>) -> Self {
        ToolContent::Text {
            text: text.into(),
            annotations: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn icon_serializes_with_spec_field_names() {
        let icon = Icon {
            src: "https://example.com/icon.png".to_owned(),
            mime_type: Some("image/png".to_owned()),
            sizes: Some(vec!["48x48".to_owned()]),
            theme: Some("dark".to_owned()),
        };
        let v = serde_json::to_value(&icon).unwrap();
        assert_eq!(v["src"], "https://example.com/icon.png");
        assert_eq!(v["mimeType"], "image/png");
        assert_eq!(v["sizes"][0], "48x48");
        assert_eq!(v["theme"], "dark");
        assert!(v.get("url").is_none(), "no legacy `url` field");
    }

    #[test]
    fn icon_deserializes_spec_fields() {
        let v = json!({
            "src": "https://example.com/icon.png",
            "mimeType": "image/svg+xml",
            "sizes": ["24x24", "48x48"],
            "theme": "light"
        });
        let icon: Icon = serde_json::from_value(v).unwrap();
        assert_eq!(icon.src, "https://example.com/icon.png");
        assert_eq!(icon.mime_type.as_deref(), Some("image/svg+xml"));
        assert_eq!(icon.sizes.as_ref().unwrap().len(), 2);
        assert_eq!(icon.theme.as_deref(), Some("light"));
    }

    #[test]
    fn content_annotations_has_last_modified() {
        let ann = ContentAnnotations {
            audience: None,
            priority: Some(0.5),
            last_modified: Some("2026-01-15T10:30:00Z".to_owned()),
        };
        let v = serde_json::to_value(&ann).unwrap();
        assert_eq!(v["lastModified"], "2026-01-15T10:30:00Z");
        assert_eq!(v["priority"], 0.5);
    }

    #[test]
    fn tool_content_text_constructor() {
        let c = ToolContent::text("hello");
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["type"], "text");
        assert_eq!(v["text"], "hello");
    }

    #[test]
    fn embedded_resource_accepts_text_only() {
        let v = json!({ "uri": "u", "text": "hi", "mimeType": "text/plain" });
        let r: EmbeddedResource = serde_json::from_value(v).unwrap();
        assert_eq!(r.text.as_deref(), Some("hi"));
        assert!(r.blob.is_none());
        assert!(r.is_well_formed());
    }

    #[test]
    fn embedded_resource_accepts_blob_only() {
        let v = json!({ "uri": "u", "blob": "AA==", "mimeType": "image/png" });
        let r: EmbeddedResource = serde_json::from_value(v).unwrap();
        assert_eq!(r.blob.as_deref(), Some("AA=="));
        assert!(r.text.is_none());
    }

    #[test]
    fn embedded_resource_rejects_neither_text_nor_blob() {
        let v = json!({ "uri": "u", "mimeType": "text/plain" });
        let err = serde_json::from_value::<EmbeddedResource>(v).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn embedded_resource_rejects_both_text_and_blob() {
        let v = json!({ "uri": "u", "text": "hi", "blob": "AA==" });
        let err = serde_json::from_value::<EmbeddedResource>(v).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn embedded_resource_serialized_form_unchanged() {
        // Byte-shape regression: text variant emits `text`, no `blob`.
        let r = EmbeddedResource {
            uri: "u".to_owned(),
            mime_type: Some("text/plain".to_owned()),
            text: Some("hi".to_owned()),
            blob: None,
            annotations: None,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["uri"], "u");
        assert_eq!(v["text"], "hi");
        assert_eq!(v["mimeType"], "text/plain");
        assert!(v.get("blob").is_none());
        assert!(v.get("_meta").is_none());
    }
}
