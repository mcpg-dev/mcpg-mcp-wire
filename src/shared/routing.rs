//! Params extraction shared by both wires' method routers.
//!
//! Every routed method starts the same way: pull `params` off the request,
//! deserialize into the method's own type, and turn either failure into
//! `-32602` carrying the request's id. Written out per arm that is a dozen
//! lines each and twenty-odd copies across the two routers, which is how four
//! of the legacy list arms came to emit the same `"invalid list params"` string
//! and leave a client unable to tell which method rejected its cursor.
//!
//! Both helpers name the method in their message, so that ambiguity is not
//! expressible.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::shared::error::ProtocolError;
use crate::shared::jsonrpc::JsonRpcRequest;

/// Deserialize a method's required `params`.
///
/// Absent `params` is `-32602`, as is a body that does not fit `T`.
pub fn required_params<T: DeserializeOwned>(
    request: &JsonRpcRequest,
    method: &str,
) -> Result<T, ProtocolError> {
    let params = request.params.clone().ok_or_else(|| {
        ProtocolError::invalid_params(
            Some(request.id.clone()),
            format!("missing {method} params"),
            None,
        )
    })?;
    deserialize_params(params, request, method)
}

/// Deserialize a method's optional `params`, defaulting when absent.
///
/// Absent `params` yields `T::default()`; a present-but-malformed body is still
/// `-32602` rather than a silent default, so a forged cursor or a typo'd field
/// is reported instead of quietly restarting pagination.
pub fn optional_params<T: DeserializeOwned + Default>(
    request: &JsonRpcRequest,
    method: &str,
) -> Result<T, ProtocolError> {
    match request.params.clone() {
        Some(params) => deserialize_params(params, request, method),
        None => Ok(T::default()),
    }
}

fn deserialize_params<T: DeserializeOwned>(
    params: Value,
    request: &JsonRpcRequest,
    method: &str,
) -> Result<T, ProtocolError> {
    serde_json::from_value(params).map_err(|error| {
        ProtocolError::invalid_params(
            Some(request.id.clone()),
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

    fn request(params: Option<Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: json!(7),
            method: "tools/list".to_owned(),
            params,
        }
    }

    #[test]
    fn required_params_deserializes_a_well_formed_body() {
        let parsed: Named = required_params(&request(Some(json!({"name": "echo"}))), "tools/call")
            .expect("a well-formed body must parse");
        assert_eq!(parsed.name, "echo");
    }

    #[test]
    fn required_params_reports_the_method_that_rejected() {
        let err = required_params::<Named>(&request(None), "tools/call")
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
        let err = required_params::<Named>(&request(Some(json!({"nome": "x"}))), "tools/call")
            .expect_err("a body that does not fit must be rejected");
        assert!(err.message().contains("tools/call"), "{}", err.message());
    }

    #[test]
    fn optional_params_defaults_when_absent() {
        let parsed: Cursor = optional_params(&request(None), "tools/list").expect("defaults");
        assert_eq!(parsed, Cursor { cursor: None });
    }

    /// A present-but-malformed body is a client error, not a silent restart —
    /// otherwise a forged cursor reads as "start from page one".
    #[test]
    fn optional_params_still_rejects_a_malformed_body() {
        let err =
            optional_params::<Cursor>(&request(Some(json!(["not", "an", "object"]))), "tools/list")
                .expect_err("malformed params must not fall back to the default");
        assert!(err.message().contains("tools/list"), "{}", err.message());
    }
}
