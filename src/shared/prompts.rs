//! `prompts/get` params and result as the gateway runtime handles them.
//!
//! - [`PromptGetParams`] — body of a `prompts/get` request.
//! - [`PromptGetResult`] — the rendered prompt.
//! - [`PromptMessage`] + [`PromptMessageContent`] — one message of the
//!   rendered prompt and its single content block.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// Content block of a prompt message: a single `ContentBlock` (Text |
/// Image | Audio | ResourceLink | EmbeddedResource). The wire shape is
/// the tool-result content set ([`ToolContent`]).
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
    use serde_json::json;

    #[test]
    fn prompt_get_params_deserializes_meta() {
        let params: PromptGetParams =
            serde_json::from_value(json!({ "name": "greet", "_meta": { "hint": true } })).unwrap();
        assert!(params.meta.is_some());
    }

    #[test]
    fn prompt_get_params_optional_arguments() {
        let params: PromptGetParams = serde_json::from_value(json!({ "name": "greet" })).unwrap();
        assert_eq!(params.name, "greet");
        assert!(params.arguments.is_none());
        assert!(params.meta.is_none());
    }

    #[test]
    fn prompt_message_content_text_serializes_with_type_tag() {
        let content = PromptMessageContent::Text {
            text: "hello".to_owned(),
            annotations: None,
        };
        let json = serde_json::to_value(&content).unwrap();
        assert_eq!(json, json!({ "type": "text", "text": "hello" }));
    }

    #[test]
    fn prompt_message_content_resource_link_uses_snake_case_tag() {
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

    #[test]
    fn prompt_get_result_serializes_messages() {
        let result = PromptGetResult {
            messages: vec![PromptMessage {
                role: "user".to_owned(),
                content: PromptMessageContent::Text {
                    text: "hi".to_owned(),
                    annotations: None,
                },
                meta: None,
            }],
        };
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({ "messages": [{ "role": "user", "content": { "type": "text", "text": "hi" } }] })
        );
    }
}
