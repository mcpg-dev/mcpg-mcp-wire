//! JSON-RPC `id` field validation rules (string | non-empty number,
//! never null / boolean / object / array / empty string).
//!
//! Same in every MCP revision — JSON-RPC 2.0 itself defines the
//! allowed shapes and MCP adds the "non-empty string" tightening so
//! ids can be reliably correlated for task replay.

use serde_json::Value;

use crate::shared::error::ProtocolError;

/// Upper bound on a string `id`, in bytes.
///
/// An id is a correlation token, so a couple of hundred bytes is already
/// generous — but the gateway remembers ids in a per-session replay window,
/// which turns an unbounded id into caller-controlled server memory. The
/// bound is enforced here so it applies once for every wire version, and
/// mirrors the `MAX_KEY_LEN` the idempotency key on the same request path
/// has always had.
pub const MAX_JSONRPC_ID_LEN: usize = 256;

/// Validate that a JSON-RPC `id` is one of the allowed shapes:
/// non-empty string or number. Rejects null, bool, object, array,
/// empty-string, and over-long ids.
pub fn validate_jsonrpc_id(id: &Value) -> Result<(), ProtocolError> {
    match id {
        Value::Null => Err(ProtocolError::invalid_request(
            "JSON-RPC `id` MUST NOT be null",
        )),
        Value::Bool(_) | Value::Object(_) | Value::Array(_) => Err(ProtocolError::invalid_request(
            "JSON-RPC `id` MUST be a string or number",
        )),
        Value::String(s) if s.is_empty() => Err(ProtocolError::invalid_request(
            "JSON-RPC `id` MUST NOT be an empty string",
        )),
        Value::String(s) if s.len() > MAX_JSONRPC_ID_LEN => Err(ProtocolError::invalid_request(
            "JSON-RPC `id` exceeds the maximum length",
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_string_id() {
        assert!(validate_jsonrpc_id(&json!("r-1")).is_ok());
    }

    #[test]
    fn accepts_number_id() {
        assert!(validate_jsonrpc_id(&json!(42)).is_ok());
        assert!(validate_jsonrpc_id(&json!(0)).is_ok());
        assert!(validate_jsonrpc_id(&json!(-1)).is_ok());
    }

    /// The gateway remembers ids in a per-session replay window, so an
    /// unbounded id is caller-controlled server memory.
    #[test]
    fn rejects_over_long_string_id() {
        let ok = "a".repeat(MAX_JSONRPC_ID_LEN);
        assert!(validate_jsonrpc_id(&json!(ok)).is_ok());

        let too_long = "a".repeat(MAX_JSONRPC_ID_LEN + 1);
        assert!(validate_jsonrpc_id(&json!(too_long)).is_err());

        let absurd = "A".repeat(4 * 1024 * 1024);
        assert!(validate_jsonrpc_id(&json!(absurd)).is_err());
    }

    #[test]
    fn rejects_null() {
        let err = validate_jsonrpc_id(&Value::Null).unwrap_err();
        assert!(err.message().to_lowercase().contains("null"));
    }

    #[test]
    fn rejects_bool_object_array() {
        assert!(validate_jsonrpc_id(&json!(true)).is_err());
        assert!(validate_jsonrpc_id(&json!({"x": 1})).is_err());
        assert!(validate_jsonrpc_id(&json!([1, 2])).is_err());
    }

    #[test]
    fn rejects_empty_string() {
        let err = validate_jsonrpc_id(&json!("")).unwrap_err();
        assert!(err.message().to_lowercase().contains("empty"));
    }
}
