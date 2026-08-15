//! Version-agnostic `completion/complete` wire types.
//!
//! `completion/complete` is the autocomplete surface clients call when
//! a user is editing a prompt argument or resource-template variable.
//! The wire shape is identical across the `2025-11-25` and
//! `2026-07-28` revisions, so the types live here once and each
//! version module re-exports them. Per-version method-name constants
//! stay version-side.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parameters for `completion/complete`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionCompleteParams {
    #[serde(rename = "ref")]
    pub reference: CompletionReference,
    pub argument: CompletionArgument,
    /// Previously-resolved arguments the server should take into
    /// account when producing suggestions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<CompletionContext>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Completion `context` object — arguments the user / model has
/// already filled in within the same completion session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompletionContext {
    #[serde(default)]
    pub arguments: std::collections::BTreeMap<String, String>,
}

/// Completion reference — either a prompt or a resource template.
/// `{ type: "ref/prompt", name }` or `{ type: "ref/resource", uri }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionReference {
    #[serde(rename = "type")]
    pub ref_type: String,
    /// Prompt name (when `ref_type` is `"ref/prompt"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Resource template URI (when `ref_type` is `"ref/resource"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

/// The argument the client is asking completions for. `name` is the
/// argument identifier (a prompt argument name or a URI-template
/// variable name); `value` is the in-progress prefix the user has
/// typed so far.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionArgument {
    pub name: String,
    pub value: String,
}

/// Result body for `completion/complete`.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionResult {
    pub completion: CompletionValues,
}

/// Inner payload of a `completion/complete` result. Spec caps `values`
/// at 100 entries; `hasMore` / `total` are optional hints for the
/// client's UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionValues {
    pub values: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn params_round_trip_prompt_reference() {
        let v = json!({
            "ref": { "type": "ref/prompt", "name": "greet" },
            "argument": { "name": "name", "value": "Ji" }
        });
        let params: CompletionCompleteParams = serde_json::from_value(v).unwrap();
        assert_eq!(params.reference.ref_type, "ref/prompt");
        assert_eq!(params.reference.name.as_deref(), Some("greet"));
        assert!(params.reference.uri.is_none());
        assert_eq!(params.argument.value, "Ji");
        assert!(params.context.is_none());
        assert!(params.meta.is_none());
    }

    #[test]
    fn params_round_trip_resource_template_reference_with_context() {
        let v = json!({
            "ref": { "type": "ref/resource", "uri": "file:///{path}" },
            "argument": { "name": "path", "value": "src/" },
            "context": { "arguments": { "region": "us-west1" } },
            "_meta": { "io.modelcontextprotocol/traceparent": "00-x-y-01" }
        });
        let params: CompletionCompleteParams = serde_json::from_value(v).unwrap();
        assert_eq!(params.reference.ref_type, "ref/resource");
        assert_eq!(params.reference.uri.as_deref(), Some("file:///{path}"));
        assert_eq!(
            params
                .context
                .unwrap()
                .arguments
                .get("region")
                .map(String::as_str),
            Some("us-west1")
        );
        assert!(params.meta.is_some());
    }

    #[test]
    fn completion_values_serialize_with_camel_case_optionals() {
        let result = CompletionResult {
            completion: CompletionValues {
                values: vec!["a".to_owned(), "b".to_owned()],
                has_more: Some(true),
                total: Some(42),
            },
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["completion"]["values"][0], "a");
        assert_eq!(v["completion"]["hasMore"], true);
        assert_eq!(v["completion"]["total"], 42);
    }

    #[test]
    fn completion_values_omit_unset_optionals() {
        let v = serde_json::to_value(&CompletionValues {
            values: vec![],
            has_more: None,
            total: None,
        })
        .unwrap();
        assert!(v.get("hasMore").is_none());
        assert!(v.get("total").is_none());
    }
}
