//! Requests a server needs the client to fulfil: `elicitation/create`,
//! `sampling/createMessage` and `roots/list`.
//!
//! The pipeline engine builds each one as a [`ServerJsonRpcRequest`]
//! carrying an [`ElicitationCreateParams`] or
//! [`SamplingCreateMessageParams`] body. MRTR (SEP-2322) renders every
//! such request as one `inputRequests` entry of an
//! [`InputRequiredResult`](crate::v_2026_07_28::wire::mrtr::InputRequiredResult),
//! keyed by the request's `id`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::content::{ContentAnnotations, ToolContent};

/// `sampling/createMessage.maxTokens` is required. Substituted when an
/// operator omits it from a pipeline sampling step.
pub const DEFAULT_SAMPLING_MAX_TOKENS: u64 = 4096;

/// A request the server needs the client to fulfil. `id` is the
/// correlation token of the request's `inputRequests` entry.
#[derive(Debug, Clone, Serialize)]
pub struct ServerJsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub method: String,
    pub params: Value,
}

/// `elicitation/create` parameters.
///
/// The `mode` field determines which sibling fields are required:
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
    /// URL mode: unique identifier of this elicitation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation_id: Option<String>,
    /// URL mode: URL the client should navigate the user to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Task metadata, when the request belongs to a task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
    /// Hint for how to present the elicitation: `"inline"`,
    /// `"popup"`, or `"newWindow"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation_hint: Option<String>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Sampling context-inclusion hint. MCP defines exactly three
/// variants; any other value on the wire fails deserialization so a
/// misspelled config cannot silently degrade to the default.
///
/// Controls which MCP server context the client should include when
/// fulfilling a `sampling/createMessage` request. Serialized as
/// camelCase on the wire (`"none"`, `"thisServer"`, `"allServers"`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SamplingIncludeContext {
    None,
    ThisServer,
    AllServers,
}

/// `sampling/createMessage` parameters. All fields except `messages`
/// and `maxTokens` are optional hints from the server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingCreateMessageParams {
    pub messages: Vec<SamplingMessage>,
    /// Required on the wire; a pipeline sampling step that omits it
    /// sends [`DEFAULT_SAMPLING_MAX_TOKENS`].
    pub max_tokens: u64,
    /// Model preferences ordered by priority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<Value>,
    /// System prompt to inject.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Context inclusion hint. A typed enum, so an unknown value fails
    /// deserialization instead of passing as "none".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_context: Option<SamplingIncludeContext>,
    /// Temperature (0.0 to 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Metadata (opaque).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    /// Tools available to the model during sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,
    /// Tool choice constraint (`"auto"`, `"none"`,
    /// `{"type": "tool", "name": "..."}`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    /// Task metadata, when the request belongs to a task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<Value>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// One conversation turn carried inside [`SamplingCreateMessageParams::messages`].
#[derive(Debug, Clone, Serialize)]
pub struct SamplingMessage {
    pub role: String,
    pub content: SamplingMessageContent,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Tagged-union content payload of a [`SamplingMessage`]. Distinct
/// from [`ToolContent`] in that it carries the `tool_use` and
/// `tool_result` variants required by SEP-1577 (sampling with tools).
///
/// **Invariant (SEP-1577):** the `content` array inside
/// `ToolResult` carries only `text` / `image` / `audio` / `resource`
/// / `resource_link` blocks (the `ToolContent` variants) — nested
/// `tool_use` or `tool_result` is rejected at deserialization time
/// because [`ToolContent`] does not include those variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SamplingMessageContent {
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
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
        #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
        meta: Option<Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        #[serde(default)]
        content: Vec<ToolContent>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "structuredContent")]
        structured_content: Option<Value>,
        #[serde(
            default,
            skip_serializing_if = "std::ops::Not::not",
            rename = "isError"
        )]
        is_error: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sampling_params(max_tokens: u64) -> SamplingCreateMessageParams {
        SamplingCreateMessageParams {
            messages: vec![],
            max_tokens,
            model_preferences: None,
            system_prompt: None,
            include_context: None,
            temperature: None,
            stop_sequences: None,
            metadata: None,
            tools: None,
            tool_choice: None,
            task: None,
            meta: None,
        }
    }

    #[test]
    fn server_jsonrpc_request_serializes() {
        let request = ServerJsonRpcRequest {
            jsonrpc: "2.0",
            id: Value::String("srv-req-1".to_owned()),
            method: "elicitation/create".to_owned(),
            params: json!({ "message": "Do you confirm?" }),
        };
        let v = serde_json::to_value(&request).unwrap();
        assert_eq!(
            v,
            json!({
                "jsonrpc": "2.0",
                "id": "srv-req-1",
                "method": "elicitation/create",
                "params": { "message": "Do you confirm?" }
            })
        );
    }

    #[test]
    fn elicitation_params_form_mode_serializes() {
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
        assert_eq!(v["mode"], "form");
        assert_eq!(v["message"], "Confirm deployment?");
        assert!(v["requestedSchema"].is_object());
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
    fn sampling_create_message_params_serialize() {
        let mut params = sampling_params(512);
        params.messages = vec![SamplingMessage {
            role: "user".to_owned(),
            content: SamplingMessageContent::Text {
                text: "Summarize this data".to_owned(),
                annotations: None,
            },
            meta: None,
        }];
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["content"]["type"], "text");
        assert_eq!(v["maxTokens"], 512);
    }

    #[test]
    fn sampling_params_all_fields_serialize() {
        let params = SamplingCreateMessageParams {
            messages: vec![],
            max_tokens: 100,
            model_preferences: Some(json!({ "hints": [] })),
            system_prompt: Some("You are helpful.".to_owned()),
            include_context: Some(SamplingIncludeContext::ThisServer),
            temperature: Some(0.7),
            stop_sequences: Some(vec!["STOP".to_owned()]),
            metadata: Some(json!({ "key": "val" })),
            tools: Some(vec![json!({ "name": "calc" })]),
            tool_choice: Some(json!("auto")),
            task: Some(json!({ "taskId": "t-1" })),
            meta: Some(json!({ "custom": true })),
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["maxTokens"], 100);
        assert_eq!(v["systemPrompt"], "You are helpful.");
        assert_eq!(v["includeContext"], "thisServer");
        assert_eq!(v["temperature"], 0.7);
        assert_eq!(v["stopSequences"][0], "STOP");
        assert!(v["tools"].is_array());
        assert_eq!(v["toolChoice"], "auto");
        assert_eq!(v["task"]["taskId"], "t-1");
        assert!(v["_meta"]["custom"].as_bool().unwrap());
    }

    /// `maxTokens` is required on the wire, so it always serializes,
    /// even at the default.
    #[test]
    fn sampling_create_message_serializes_max_tokens() {
        let v = serde_json::to_value(sampling_params(DEFAULT_SAMPLING_MAX_TOKENS)).unwrap();
        assert_eq!(v["maxTokens"], 4096);
    }

    #[test]
    fn sampling_tool_result_rejects_nested_tool_use() {
        let bad = json!({
            "type": "tool_result",
            "toolUseId": "tu-1",
            "content": [{ "type": "tool_use", "id": "x", "name": "y", "input": {} }]
        });
        let parsed: Result<SamplingMessageContent, _> = serde_json::from_value(bad);
        assert!(parsed.is_err(), "nested tool_use must not deserialize");
    }

    #[test]
    fn sampling_tool_result_accepts_text_image_audio() {
        for variant in [
            json!({ "type": "text", "text": "hello" }),
            json!({ "type": "image", "data": "AA==", "mimeType": "image/png" }),
            json!({ "type": "audio", "data": "AA==", "mimeType": "audio/mpeg" }),
        ] {
            let msg = json!({
                "type": "tool_result",
                "toolUseId": "tu-1",
                "content": [variant]
            });
            let parsed: Result<SamplingMessageContent, _> = serde_json::from_value(msg);
            assert!(parsed.is_ok(), "must accept allowed variant");
        }
    }

    #[test]
    fn sampling_include_context_round_trips_and_rejects_unknown_values() {
        for (variant, wire) in [
            (SamplingIncludeContext::None, "none"),
            (SamplingIncludeContext::ThisServer, "thisServer"),
            (SamplingIncludeContext::AllServers, "allServers"),
        ] {
            assert_eq!(serde_json::to_value(&variant).unwrap(), wire);
            let back: SamplingIncludeContext = serde_json::from_value(json!(wire)).unwrap();
            assert_eq!(back, variant);
        }
        let parsed: Result<SamplingIncludeContext, _> = serde_json::from_value(json!("everywhere"));
        assert!(parsed.is_err(), "unknown includeContext value must fail");
    }
}
