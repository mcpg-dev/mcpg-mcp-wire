//! `tools/call` params and result as the gateway runtime handles them.
//!
//! - [`ToolCallParams`] — the tool name, its arguments and `_meta`.
//! - [`ToolCallResult`] — the content a call resolves to once the
//!   backend has produced it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::content::ToolContent;

/// Parameters of a `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallParams {
    pub name: String,
    #[serde(default)]
    pub arguments: Option<Value>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result body of a `tools/call`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<Value>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_error: bool,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_call_params_deserializes_meta() {
        let params: ToolCallParams = serde_json::from_value(json!({
            "name": "my_tool",
            "arguments": { "key": "val" },
            "_meta": { "org.paymentauth/credential": { "challenge": "abc" } }
        }))
        .unwrap();
        assert_eq!(params.name, "my_tool");
        let meta = params.meta.expect("meta");
        assert!(meta.get("org.paymentauth/credential").is_some());
    }

    #[test]
    fn tool_call_params_without_meta_has_none() {
        let params: ToolCallParams = serde_json::from_value(json!({
            "name": "my_tool",
            "arguments": { "key": "val" }
        }))
        .unwrap();
        assert!(params.meta.is_none());
    }

    #[test]
    fn tool_call_params_serialize_absent_arguments_as_null_and_skip_meta() {
        let params = ToolCallParams {
            name: "t".to_owned(),
            arguments: None,
            meta: None,
        };
        assert_eq!(
            serde_json::to_value(&params).unwrap(),
            json!({ "name": "t", "arguments": null })
        );
    }

    #[test]
    fn tool_call_result_serializes_meta() {
        let result = ToolCallResult {
            content: vec![ToolContent::text("ok")],
            structured_content: None,
            is_error: false,
            meta: Some(json!({ "org.paymentauth/receipt": { "status": "success" } })),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            json["_meta"]["org.paymentauth/receipt"]["status"],
            "success"
        );
        assert!(json.get("isError").is_none(), "false isError is omitted");
    }

    #[test]
    fn tool_call_result_round_trips_error_and_structured_content() {
        let result = ToolCallResult {
            content: vec![ToolContent::text("boom")],
            structured_content: Some(json!({ "code": 7 })),
            is_error: true,
            meta: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["isError"], true);
        assert_eq!(json["structuredContent"]["code"], 7);
        assert!(json.get("_meta").is_none());
        let back: ToolCallResult = serde_json::from_value(json).unwrap();
        assert!(back.is_error);
    }
}
