//! Version-erased message types that cross the boundary between the
//! per-version [`ProtocolHandler`] impls and the version-blind runtime
//! services.
//!
//! Each version handler defines its own concrete operation enum;
//! [`ProtocolMessage`] carries the boxed variant plus enough
//! version-agnostic metadata (label, JSON-RPC id, method name) for the
//! runtime's request orchestration to dispatch metrics, errors, and
//! request-id tracking without knowing the concrete enum.
//!
//! [`ProtocolHandler`]: super::traits::ProtocolHandler

use std::any::Any;
use std::collections::BTreeMap;

use bytes::Bytes;
use serde_json::Value;

use crate::version::ProtocolVersion;

/// Version-erased opaque parsed message.
///
/// Each [`ProtocolHandler`](super::traits::ProtocolHandler) impl
/// defines its own internal operation enum; this wrapper carries the
/// boxed concrete type plus a short stable label for metrics and a
/// JSON-RPC envelope echo for error replies.
pub struct ProtocolMessage {
    /// Stable label used for metric and log labels (e.g.,
    /// `"tools.call"`, `"server.discover"`, `"subscriptions.listen"`).
    pub label: &'static str,
    /// The concrete per-version operation. Downcast inside the
    /// matching `ProtocolHandler::dispatch`.
    pub inner: Box<dyn Any + Send + Sync>,
    /// JSON-RPC `id` of the originating client request. Echoed on
    /// errors emitted by the runtime before the handler sees the
    /// message. `None` for notifications.
    pub jsonrpc_id: Option<Value>,
    /// JSON-RPC `method` string from the body. Used to validate the
    /// `Mcp-Method` HTTP header on modern transports and to label
    /// observability events.
    pub mcp_method: Option<String>,
    /// Negotiated protocol version that produced this message.
    pub negotiated_version: ProtocolVersion,
}

impl ProtocolMessage {
    /// Downcast helper: borrow the inner value as `T`, returning
    /// `None` if the message was minted by a different handler.
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }

    /// Downcast helper: take ownership of the inner value as `T`,
    /// returning `Err(self)` if the message was minted by a different
    /// handler.
    pub fn downcast<T: 'static>(self) -> Result<Box<T>, Self> {
        let ProtocolMessage {
            label,
            inner,
            jsonrpc_id,
            mcp_method,
            negotiated_version,
        } = self;
        match inner.downcast::<T>() {
            Ok(boxed) => Ok(boxed),
            Err(inner) => Err(ProtocolMessage {
                label,
                inner,
                jsonrpc_id,
                mcp_method,
                negotiated_version,
            }),
        }
    }
}

/// Suspension produced by the version-blind pipeline engine, carrying the
/// server-initiated input request(s) a wire has to render.
///
/// - The legacy wire flattens this into a server-initiated JSON-RPC request
///   published on the delivery bus, answered HTTP 202 / SSE.
/// - The modern wire turns it into an `InputRequiredResult` with
///   `requestState` carried inline.
pub struct PipelineSuspension {
    /// Stable id of the pipeline this suspension belongs to.
    pub pipeline_id: String,
    /// Stable id of the suspending step.
    pub step_id: String,
    /// JSON-RPC id of the request whose dispatch suspended.
    pub jsonrpc_id: Value,
    /// Opaque serialized pipeline state, ready to be persisted as a
    /// `requestState` blob (modern) or alongside a `pipeline_store`
    /// row (legacy). Empty `Bytes` when the legacy
    /// `pipeline_store` carries the state out-of-band.
    pub serialized_state: Bytes,
    /// The input request(s) the suspending step needs the client to
    /// fulfil before resumption.
    pub requests: SuspensionRequests,
    /// Server-minted id for the wire-level server-initiated request
    /// (legacy 2025-11-25 only — the `id` of `ServerJsonRpcRequest`
    /// the client sees over SSE). Modern revisions don't need this
    /// because MRTR's `InputRequiredResult` is the response to the
    /// original `tools/call`, not a separate request, and the
    /// correlation token lives in the `inputRequests` map key.
    pub server_request_id: Option<String>,
    /// `taskId` to inject as `_meta.io.modelcontextprotocol/related-task`
    /// on the outbound envelope. Set by the resumption / task-augmented
    /// dispatch paths in `runtime/mod.rs`; left `None` for vanilla
    /// `tools/call` suspensions.
    pub related_task_id: Option<String>,
}

/// One or many simultaneous input requests carried by a suspension.
pub enum SuspensionRequests {
    /// Single sampling request (legacy versions emit one suspension
    /// per `sampling` step).
    Sampling(SuspensionRequestParams),
    /// Single elicitation request.
    Elicitation(SuspensionRequestParams),
    /// Single `roots/list` request. `params` carries
    /// `_meta.traceparent` (SEP-414) and any other server-side hints;
    /// it is `{}` by default.
    Roots(SuspensionRequestParams),
    /// Multiple simultaneous input requests (modern MRTR-style). Map
    /// key is the correlation token used in `inputResponses`.
    Many(BTreeMap<String, SuspensionRequestParams>),
}

impl SuspensionRequests {
    /// Single-request `method` + `params` accessor for legacy
    /// versions. Returns `None` for `Many` (modern-only).
    pub fn single_method_and_params(&self) -> Option<(&str, &Value)> {
        match self {
            Self::Sampling(p) | Self::Elicitation(p) | Self::Roots(p) => {
                Some((p.method.as_str(), &p.params))
            }
            Self::Many(_) => None,
        }
    }
}

/// Per-request parameters carried by a single input request inside a
/// [`PipelineSuspension`]. The handler is responsible for stamping
/// these into the version-specific wire shape (a JSON-RPC request
/// body, a `sampling/createMessage` params object, etc.).
pub struct SuspensionRequestParams {
    /// Wire-method name (e.g., `"sampling/createMessage"`,
    /// `"elicitation/create"`, `"roots/list"`).
    pub method: String,
    /// Pre-built `params` object for the input request.
    pub params: Value,
}

/// Transport-layer rejection that bypasses the protocol handler's
/// normal response path.
///
/// Used by
/// [`ProtocolHandler::validate_transport_headers`](super::traits::ProtocolHandler::validate_transport_headers)
/// to signal a header / body mismatch, missing required header, or
/// unsupported protocol version *before* the body is even parsed.
///
/// The transport converts this into an HTTP response: the `status` is
/// the HTTP status code; the JSON-RPC error body
/// (`error_code` / `message` / `data`) is rendered into the response
/// body when `jsonrpc_id` is `Some` (i.e., the rejection is correlated
/// to an identifiable request).
pub struct TransportRejection {
    pub status: u16,
    pub error_code: i32,
    pub message: String,
    pub data: Option<Value>,
    pub jsonrpc_id: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downcast_round_trips_inner_type() {
        let msg = ProtocolMessage {
            label: "test.op",
            inner: Box::new(42u32),
            jsonrpc_id: Some(Value::from(1)),
            mcp_method: Some("test/op".to_owned()),
            negotiated_version: ProtocolVersion::V_2025_11_25,
        };
        assert_eq!(msg.downcast_ref::<u32>(), Some(&42));
        let Ok(boxed) = msg.downcast::<u32>() else {
            panic!("downcast should succeed for the boxed inner type");
        };
        assert_eq!(*boxed, 42);
    }

    #[test]
    fn downcast_wrong_type_preserves_message() {
        let msg = ProtocolMessage {
            label: "test.op",
            inner: Box::new(42u32),
            jsonrpc_id: None,
            mcp_method: None,
            negotiated_version: ProtocolVersion::V_2025_11_25,
        };
        let Err(recovered) = msg.downcast::<String>() else {
            panic!("downcast to wrong type should return Err with the original message");
        };
        assert_eq!(recovered.label, "test.op");
        assert_eq!(recovered.downcast_ref::<u32>(), Some(&42));
    }
}
