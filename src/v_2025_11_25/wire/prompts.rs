//! Prompts wire types for MCP revision `2025-11-25`.
//!
//! - [`PromptGetParams`] — body of a `prompts/get` request.
//! - [`PromptsListResult`] — body of a `prompts/list` result.
//! - [`PromptGetResult`] — body of a `prompts/get` result.
//! - [`PromptMessage`] — one message in the rendered prompt output.
//! - [`PromptMessageContent`] — the tagged-union content of a
//!   single prompt message (per MCP 2025-11-25, a single
//!   `ContentBlock`: Text | Image | Audio | ResourceLink | Resource).
//!
//! Operation-enum variants live in [`operations`](super::operations); the
//! method-name → operation router lives in [`routing`](super::routing).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::descriptors::PromptDescriptor;
use crate::shared::content::{ContentAnnotations, EmbeddedResource, Icon};

/// Parameters for `prompts/get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptGetParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result body for `prompts/list`.
#[derive(Debug, Clone, Serialize)]
pub struct PromptsListResult {
    pub prompts: Vec<PromptDescriptor>,
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

/// Result body for `prompts/get`.
#[derive(Debug, Clone, Serialize)]
pub struct PromptGetResult {
    pub messages: Vec<PromptMessage>,
}

/// One message in a prompt's rendered output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMessage {
    pub role: String,
    pub content: PromptMessageContent,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Content block for prompt messages — per MCP 2025-11-25, this is a
/// single `ContentBlock` (Text | Image | Audio | ResourceLink |
/// EmbeddedResource). Distinct from [`ToolContent`] only insofar as
/// the prompt-side enum has historically been defined separately;
/// the wire shape is identical to the tool-result content set.
///
/// [`ToolContent`]: crate::shared::content::ToolContent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PromptMessageContent {
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
    Resource {
        resource: EmbeddedResource,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        annotations: Option<ContentAnnotations>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_get_params_deserializes_meta() {
        let json = serde_json::json!({ "name": "greet", "_meta": { "hint": true } });
        let params: PromptGetParams = serde_json::from_value(json).unwrap();
        assert!(params.meta.is_some());
    }

    #[test]
    fn prompt_get_params_optional_arguments() {
        let json = serde_json::json!({ "name": "greet" });
        let params: PromptGetParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.name, "greet");
        assert!(params.arguments.is_none());
        assert!(params.meta.is_none());
    }

    #[test]
    fn prompts_list_result_omits_null_cursor() {
        let result = PromptsListResult {
            prompts: vec![],
            next_cursor: None,
            ttl_ms: None,
            cache_scope: None,
            cache_token: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("nextCursor").is_none());
    }

    #[test]
    fn prompt_message_content_text_serializes_with_type_tag() {
        let content = PromptMessageContent::Text {
            text: "hello".to_owned(),
            annotations: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "text");
        assert_eq!(json["text"], "hello");
    }

    #[test]
    fn prompt_message_content_resource_link_uses_snake_case_tag() {
        // The tag uses an explicit `#[serde(rename = "resource_link")]`
        // because the default snake_case rename would be the same string
        // anyway — but pinning this in a test guards against a future
        // attr removal from drifting the wire tag.
        let content = PromptMessageContent::ResourceLink {
            uri: "https://example.com/x".to_owned(),
            name: "x".to_owned(),
            title: None,
            description: None,
            mime_type: None,
            annotations: None,
            size: None,
            icons: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json["type"], "resource_link");
    }
}
