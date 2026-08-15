//! Multi Round-Trip Requests (MRTR) wire types — SEP-2322.
//!
//! MRTR replaces the legacy SSE-based suspension flow with an
//! **inline body** mechanism. When a `tools/call` (or any other
//! method whose dispatch can pause for user / model input)
//! suspends, the server returns an [`InputRequiredResult`] as the
//! `result` of the JSON-RPC response:
//!
//! ```text
//! {
//!   "jsonrpc": "2.0",
//!   "id": <original tools/call id>,
//!   "result": {
//!     "resultType": "input_required",
//!     "requestState": "<opaque encoded blob or handle>",
//!     "inputRequests": {
//!       "<correlation-token>": {
//!         "type": "elicitation" | "sampling" | "roots",
//!         "params": { ... }
//!       },
//!       ...
//!     },
//!     "instructions": "optional human-readable hint"
//!   }
//! }
//! ```
//!
//! The client fulfils each request and re-issues the same
//! `tools/call` with the answers tucked into `_meta` under the
//! reserved `io.modelcontextprotocol/inputResponses` key, plus the
//! server-minted `requestState` echoed back so the server can
//! resume:
//!
//! ```text
//! {
//!   "jsonrpc": "2.0", "id": <new id>, "method": "tools/call",
//!   "params": {
//!     "name": "<tool>",
//!     "arguments": { ... },
//!     "_meta": {
//!       "io.modelcontextprotocol/requestState": "<echoed blob/handle>",
//!       "io.modelcontextprotocol/inputResponses": {
//!         "<correlation-token>": <answer value>
//!       }
//!     }
//!   }
//! }
//! ```
//!
//! **Differences from legacy 2025-11-25:**
//!
//! - **No SSE round-trip.** The legacy flow returned `HTTP 202
//!   NotificationAccepted` + put a `ServerJsonRpcRequest` on the
//!   session's SSE delivery bus; the client then sent the
//!   response back as a `ClientMessage::Response`. MRTR is all in
//!   the request/response body — no stateful delivery bus, no
//!   stream coordination.
//! - **Multiple input requests in flight.** The legacy flow could
//!   only carry a single suspension per dispatch; MRTR's
//!   `inputRequests` is a map, so the server can request several
//!   things at once (e.g., an elicitation AND a sampling) and the
//!   client returns answers keyed by the same correlation tokens.
//! - **Server-opaque state.** Pipeline state rides in
//!   `requestState`, a server-chosen encoding (encrypted inline
//!   for small payloads, KV-store handle for large ones — see
//!   `dispatch/request_state.rs`).
//!
//! This module defines only the **wire types**. The codec sits in
//! `dispatch::request_state`; the dispatch arm builds
//! `InputRequiredResult` instead of returning `-32603`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Well-known `_meta` keys.
// ---------------------------------------------------------------------------

/// `_meta` key the **server** stamps onto `InputRequiredResult`
/// (already inside the `requestState` field on the result) and the
/// **client** echoes back on the resumption request body.
pub const META_KEY_REQUEST_STATE: &str = "io.modelcontextprotocol/requestState";

/// `_meta` key the **client** uses on the resumption request body
/// to carry its answers to the prior `InputRequiredResult`.
pub const META_KEY_INPUT_RESPONSES: &str = "io.modelcontextprotocol/inputResponses";

// ---------------------------------------------------------------------------
// `InputRequiredResult` — the suspended-tools-call result body.
// ---------------------------------------------------------------------------

/// `resultType` discriminator value the spec stamps on
/// `InputRequiredResult` so clients can tell it apart from a
/// `ToolCallResult`.
pub const RESULT_TYPE_INPUT_REQUIRED: &str = "input_required";

/// Result body returned by a `tools/call` (or any method that can
/// suspend) when the server needs the client to provide additional
/// inputs before the call can complete.
///
/// `resultType` is always `"input_required"`; clients distinguish
/// this from a normal `ToolCallResult` by either the field or by
/// the presence of `requestState` / `inputRequests`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputRequiredResult {
    /// Discriminator. Always `"input_required"`; defaulted on
    /// construction.
    #[serde(default = "default_result_type_input_required")]
    pub result_type: String,
    /// Opaque server-encoded pipeline state. The client echoes
    /// this back verbatim in the resumption request's
    /// `_meta.io.modelcontextprotocol/requestState`; the server
    /// decodes it to pick up where it left off. Format is
    /// server-chosen (encrypted inline ≤ 8 KiB or KV-store handle
    /// above — see the `dispatch::request_state` codec).
    pub request_state: String,
    /// Map of correlation token → input request. Keys are
    /// server-chosen (e.g., random UUID); the client uses the
    /// same key in
    /// `_meta.io.modelcontextprotocol/inputResponses` to associate
    /// each answer with its request.
    pub input_requests: BTreeMap<String, InputRequest>,
    /// Optional human-readable hint clients MAY surface to the
    /// user (e.g., "The tool needs your confirmation to proceed").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// `_meta` for forward-compat (server-side trace context,
    /// idempotency, etc.).
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

fn default_result_type_input_required() -> String {
    RESULT_TYPE_INPUT_REQUIRED.to_owned()
}

impl InputRequiredResult {
    /// Build a fresh `InputRequiredResult` with the discriminator
    /// pre-stamped. Servers should always use this constructor so
    /// the wire field is never accidentally omitted.
    pub fn new(request_state: String, input_requests: BTreeMap<String, InputRequest>) -> Self {
        Self {
            result_type: RESULT_TYPE_INPUT_REQUIRED.to_owned(),
            request_state,
            input_requests,
            instructions: None,
            meta: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-entry `InputRequest`.
// ---------------------------------------------------------------------------

/// One input the server needs the client to provide. Tagged-union
/// with `method` as the discriminator carrying the full JSON-RPC
/// method name — the SEP-2322 wire shape the upstream conformance
/// suite asserts on (`{"method":"elicitation/create","params":...}`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum InputRequest {
    /// Equivalent of legacy `elicitation/create` — server asks the
    /// client to collect user input (form or URL flow).
    #[serde(rename = "elicitation/create")]
    Elicitation {
        /// Pre-built params object the server would have sent as
        /// the `params` of `elicitation/create` on the legacy wire.
        params: Value,
    },
    /// Equivalent of legacy `sampling/createMessage` — server asks
    /// the client to run an LLM completion.
    #[serde(rename = "sampling/createMessage")]
    Sampling {
        /// Pre-built params object equivalent to
        /// `sampling/createMessage`'s body.
        params: Value,
    },
    /// Equivalent of legacy `roots/list` — server asks the client
    /// for its declared roots. SEP-2577 deprecated roots with a
    /// 12-month sunset; modern servers SHOULD avoid emitting this
    /// after the runway ends.
    #[serde(rename = "roots/list")]
    Roots {
        /// Typically `{}` (no params); kept as `Value` for
        /// forwards-compat with future per-request hints.
        #[serde(default)]
        params: Value,
    },
}

// ---------------------------------------------------------------------------
// `InputResponses` parsing helper.
// ---------------------------------------------------------------------------

/// Typed view over the client-supplied `_meta.io.modelcontextprotocol/
/// inputResponses` map. Each entry is keyed by the same correlation
/// token the server emitted in `InputRequiredResult.inputRequests`;
/// the value is either the answer payload (`Ok`) or an explicit
/// `Err` envelope when the client could not fulfil the request
/// (e.g., user declined an elicitation, sampling model errored).
#[derive(Debug, Clone, Default)]
pub struct InputResponses {
    pub entries: BTreeMap<String, InputResponseValue>,
}

/// One entry in [`InputResponses`]. Untagged on the wire so the
/// client can either return the raw answer payload OR an explicit
/// `{ error: { code, message, data? } }` envelope without the
/// gateway needing a tag field.
///
/// Variant order is significant: `Err` is tried first because
/// serde's untagged enum tries variants in declaration order. If
/// `Ok(Value)` came first, an error envelope like
/// `{ "error": { ... } }` would be (mis-)accepted as a plain
/// answer Value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum InputResponseValue {
    /// Client signalled an error / cancellation for this input.
    Err { error: InputResponseError },
    /// Concrete answer payload (e.g., an `ElicitResult` body, a
    /// `CreateMessageResult` body).
    Ok(Value),
}

/// Inner shape of the `Err` variant of [`InputResponseValue`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputResponseError {
    pub code: i32,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl InputResponses {
    /// Parse a raw `_meta.io.modelcontextprotocol/inputResponses`
    /// JSON object into the typed view. Returns
    /// `Ok(InputResponses { entries: empty })` when the input is
    /// `null` or missing; returns `Err` when the shape is wrong
    /// (e.g., the field is not a JSON object).
    pub fn from_value(value: &Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let Some(obj) = value.as_object() else {
            return Err(format!(
                "{META_KEY_INPUT_RESPONSES} must be a JSON object, got: {}",
                value_kind(value)
            ));
        };
        let mut entries: BTreeMap<String, InputResponseValue> = BTreeMap::new();
        for (key, val) in obj {
            let parsed: InputResponseValue = serde_json::from_value(val.clone())
                .map_err(|e| format!("invalid {META_KEY_INPUT_RESPONSES}[{key}]: {e}"))?;
            entries.insert(key.clone(), parsed);
        }
        Ok(Self { entries })
    }
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn well_known_meta_keys_match_spec() {
        assert_eq!(
            META_KEY_REQUEST_STATE,
            "io.modelcontextprotocol/requestState"
        );
        assert_eq!(
            META_KEY_INPUT_RESPONSES,
            "io.modelcontextprotocol/inputResponses"
        );
        assert_eq!(RESULT_TYPE_INPUT_REQUIRED, "input_required");
    }

    #[test]
    fn input_required_result_serializes_with_camel_case() {
        let mut requests = BTreeMap::new();
        requests.insert(
            "elic-1".to_owned(),
            InputRequest::Elicitation {
                params: json!({ "message": "confirm?" }),
            },
        );
        let result = InputRequiredResult::new("encoded-state".to_owned(), requests);
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["resultType"], "input_required");
        assert_eq!(v["requestState"], "encoded-state");
        assert_eq!(v["inputRequests"]["elic-1"]["method"], "elicitation/create");
        assert_eq!(
            v["inputRequests"]["elic-1"]["params"]["message"],
            "confirm?"
        );
        assert!(v.get("instructions").is_none());
    }

    #[test]
    fn input_required_result_with_instructions_and_meta() {
        let mut requests = BTreeMap::new();
        requests.insert(
            "samp-1".to_owned(),
            InputRequest::Sampling {
                params: json!({
                    "messages": [{"role":"user", "content":"hi"}],
                    "maxTokens": 100
                }),
            },
        );
        let result = InputRequiredResult {
            result_type: RESULT_TYPE_INPUT_REQUIRED.to_owned(),
            request_state: "s".to_owned(),
            input_requests: requests,
            instructions: Some("Please complete the sampling".to_owned()),
            meta: Some(json!({ "io.modelcontextprotocol/traceparent": "00-x-y-01" })),
        };
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["instructions"], "Please complete the sampling");
        assert_eq!(
            v["_meta"]["io.modelcontextprotocol/traceparent"],
            "00-x-y-01"
        );
    }

    #[test]
    fn input_request_round_trips_all_three_variants() {
        for (variant_json, expected_method) in [
            (
                json!({ "method": "elicitation/create", "params": { "mode": "form" } }),
                "elicitation/create",
            ),
            (
                json!({ "method": "sampling/createMessage", "params": { "maxTokens": 4096 } }),
                "sampling/createMessage",
            ),
            (
                json!({ "method": "roots/list", "params": {} }),
                "roots/list",
            ),
        ] {
            let parsed: InputRequest = serde_json::from_value(variant_json.clone()).unwrap();
            let back = serde_json::to_value(&parsed).unwrap();
            assert_eq!(back["method"], expected_method);
        }
    }

    #[test]
    fn input_responses_parses_ok_value_entries() {
        let v = json!({
            "elic-1": { "action": "accept", "content": { "confirm": true } },
            "samp-1": { "content": [{ "type": "text", "text": "answer" }] }
        });
        let parsed = InputResponses::from_value(&v).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        match parsed.entries.get("elic-1").unwrap() {
            InputResponseValue::Ok(payload) => {
                assert_eq!(payload["action"], "accept");
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn input_responses_parses_error_entries() {
        let v = json!({
            "elic-1": {
                "error": {
                    "code": -32603,
                    "message": "user cancelled",
                    "data": { "reason": "esc" }
                }
            }
        });
        let parsed = InputResponses::from_value(&v).unwrap();
        match parsed.entries.get("elic-1").unwrap() {
            InputResponseValue::Err { error } => {
                assert_eq!(error.code, -32603);
                assert_eq!(error.message, "user cancelled");
                assert_eq!(error.data.as_ref().unwrap()["reason"], "esc");
            }
            other => panic!("expected Err, got {other:?}"),
        }
    }

    #[test]
    fn input_responses_from_null_returns_empty() {
        let parsed = InputResponses::from_value(&Value::Null).unwrap();
        assert!(parsed.entries.is_empty());
    }

    #[test]
    fn input_responses_from_non_object_is_error() {
        let err = InputResponses::from_value(&json!(42)).unwrap_err();
        assert!(err.contains("must be a JSON object"));
    }

    #[test]
    fn default_result_type_helper_returns_expected_string() {
        // The serde `#[serde(default = ...)]` machinery uses this
        // helper when `resultType` is omitted on the wire.
        assert_eq!(default_result_type_input_required(), "input_required");
    }

    #[test]
    fn input_required_result_new_constructor_stamps_result_type() {
        let mut requests = BTreeMap::new();
        requests.insert("k".to_owned(), InputRequest::Roots { params: json!({}) });
        let r = InputRequiredResult::new("blob".to_owned(), requests);
        assert_eq!(r.result_type, RESULT_TYPE_INPUT_REQUIRED);
    }
}
