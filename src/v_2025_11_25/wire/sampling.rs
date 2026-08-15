//! Sampling wire types for MCP revision `2025-11-25`.
//!
//! `sampling/createMessage` is the server-initiated path where the
//! server asks the client to run an LLM completion on its behalf.
//! The client has full discretion over which model is used and
//! whether to surface the request to the user.
//!
//! ## Modern counterpart
//!
//! Like elicitation, sampling is an `inputRequests` map entry inside MRTR
//! (`InputRequiredResult`, SEP-2322) on `2026-07-28` rather than a standalone
//! server-initiated request. SEP-2577 additionally deprecates the sampling
//! primitive with a 12-month sunset. The shapes here are frozen for the
//! `2025-11-25` compatibility window.
//!
//! The `DEFAULT_SAMPLING_MAX_TOKENS` constant
//! (substituted into pipeline-emitted sampling steps that omit
//! `maxTokens`) lives at the version's `wire/mod.rs` root.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::content::{ContentAnnotations, ToolContent};

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

/// Server-to-client sampling request parameters per MCP 2025-11-25.
/// All fields except `messages` and `maxTokens` are optional hints
/// from the server.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingCreateMessageParams {
    pub messages: Vec<SamplingMessage>,
    /// MCP 2025-11-25 §Sampling: `maxTokens` is REQUIRED.
    /// Operators that omit it from a pipeline sampling step receive
    /// `DEFAULT_SAMPLING_MAX_TOKENS` (re-exported from the version
    /// root) so the wire envelope is always spec-compliant.
    pub max_tokens: u64,
    /// Model preferences ordered by priority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_preferences: Option<Value>,
    /// System prompt to inject.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Context inclusion hint. Narrowed from
    /// `Option<String>` to a typed enum so unknown values fail
    /// deserialization instead of silently masquerading as "none".
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
    /// Task ID if this sampling is associated with a running task.
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
    use crate::v_2025_11_25::wire::DEFAULT_SAMPLING_MAX_TOKENS;
    use serde_json::json;

    #[test]
    fn sampling_create_message_params_serialize() {
        let params = SamplingCreateMessageParams {
            messages: vec![SamplingMessage {
                role: "user".to_owned(),
                content: SamplingMessageContent::Text {
                    text: "Summarize this data".to_owned(),
                    annotations: None,
                },
                meta: None,
            }],
            max_tokens: 512,
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
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["messages"][0]["role"], "user");
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
        assert!(v["_meta"]["custom"].as_bool().unwrap());
    }

    /// `maxTokens` is REQUIRED on the wire — must always
    /// serialise, even when set to the gateway's default.
    #[test]
    fn sampling_create_message_serializes_max_tokens() {
        let p = SamplingCreateMessageParams {
            messages: vec![],
            max_tokens: DEFAULT_SAMPLING_MAX_TOKENS,
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
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["maxTokens"], 4096);
    }

    /// Spec-level invariant — content inside
    /// `SamplingMessage::ToolResult` is restricted to text / image /
    /// audio / resource / resource_link. The `ToolContent` enum
    /// omits nested tool_use / tool_result, so a deserialized payload
    /// carrying nested tool types must fail.
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

    /// Sanity-check the allowed content variants.
    #[test]
    fn sampling_tool_result_accepts_text_image_audio_resource() {
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
    fn sampling_include_context_rejects_unknown_value() {
        let bad = json!("everywhere");
        let parsed: Result<SamplingIncludeContext, _> = serde_json::from_value(bad);
        assert!(parsed.is_err(), "unknown includeContext value must fail");
    }
}
