//! Params extraction for the method router.
//!
//! Every routed method starts the same way: take `params` off the request,
//! deserialize into the method's own type, and turn either failure into
//! `-32602` carrying the request's id. Both helpers take the params by value,
//! so the deserializer moves the client's strings instead of copying them,
//! and name the method in their message, so a client can always tell which
//! method rejected its params.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::shared::error::ProtocolError;

/// Deserialize a method's required `params` of request `id`.
///
/// Absent `params` is `-32602`, as is a body that does not fit `T`.
pub fn required_params<T: DeserializeOwned>(
    params: Option<Value>,
    id: &Value,
    method: &str,
) -> Result<T, ProtocolError> {
    let params = params.ok_or_else(|| {
        ProtocolError::invalid_params(Some(id.clone()), format!("missing {method} params"), None)
    })?;
    deserialize_params(params, id, method)
}

/// Deserialize a method's optional `params` of request `id`, defaulting when
/// absent.
///
/// Absent `params` yields `T::default()`; a present-but-malformed body is still
/// `-32602` rather than a silent default, so a forged cursor or a typo'd field
/// is reported instead of quietly restarting pagination.
pub fn optional_params<T: DeserializeOwned + Default>(
    params: Option<Value>,
    id: &Value,
    method: &str,
) -> Result<T, ProtocolError> {
    match params {
        Some(params) => deserialize_params(params, id, method),
        None => Ok(T::default()),
    }
}

fn deserialize_params<T: DeserializeOwned>(
    params: Value,
    id: &Value,
    method: &str,
) -> Result<T, ProtocolError> {
    serde_json::from_value(params).map_err(|error| {
        ProtocolError::invalid_params(
            Some(id.clone()),
            format!("invalid {method} params: {error}"),
            None,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Deserialize, Default, PartialEq)]
    struct Cursor {
        #[serde(default)]
        cursor: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    struct Named {
        name: String,
    }

    fn id() -> Value {
        json!(7)
    }

    #[test]
    fn required_params_deserializes_a_well_formed_body() {
        let parsed: Named = required_params(Some(json!({"name": "echo"})), &id(), "tools/call")
            .expect("a well-formed body must parse");
        assert_eq!(parsed.name, "echo");
    }

    #[test]
    fn required_params_reports_the_method_that_rejected() {
        let err = required_params::<Named>(None, &id(), "tools/call")
            .expect_err("absent params must be rejected");
        assert!(err.message().contains("tools/call"), "{}", err.message());
        let envelope = err.into_jsonrpc_error();
        assert_eq!(
            serde_json::to_value(&envelope).expect("error serializes")["id"],
            json!(7),
            "the request id must survive so the client can correlate the rejection"
        );
    }

    #[test]
    fn required_params_rejects_a_malformed_body_by_method() {
        let err = required_params::<Named>(Some(json!({"nome": "x"})), &id(), "tools/call")
            .expect_err("a body that does not fit must be rejected");
        assert!(err.message().contains("tools/call"), "{}", err.message());
    }

    #[test]
    fn optional_params_defaults_when_absent() {
        let parsed: Cursor = optional_params(None, &id(), "tools/list").expect("defaults");
        assert_eq!(parsed, Cursor { cursor: None });
    }

    /// A present-but-malformed body is a client error, not a silent restart —
    /// otherwise a forged cursor reads as "start from page one".
    #[test]
    fn optional_params_still_rejects_a_malformed_body() {
        let err =
            optional_params::<Cursor>(Some(json!(["not", "an", "object"])), &id(), "tools/list")
                .expect_err("malformed params must not fall back to the default");
        assert!(err.message().contains("tools/list"), "{}", err.message());
    }
}
