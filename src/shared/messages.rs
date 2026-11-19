//! Parsed messages and transport rejections that cross the boundary
//! between the gateway's protocol handler and its runtime.
//!
//! [`ProtocolMessage`] carries the typed operation plus the metadata
//! (label, JSON-RPC id) the runtime's request orchestration uses for
//! metrics, errors and request-id tracking.

use serde_json::Value;

use crate::v_2026_07_28::wire::operations::ProtocolOperation;

/// A parsed client message, ready for dispatch.
#[derive(Debug, Clone)]
pub struct ProtocolMessage {
    /// Stable label used for metric and log labels (e.g.,
    /// `"capabilities.tools_call"`, `"lifecycle.discover"`).
    pub label: &'static str,
    /// The routed operation.
    pub operation: ProtocolOperation,
    /// JSON-RPC `id` of the originating client request. Echoed on
    /// errors emitted by the runtime before the handler sees the
    /// message. `None` for notifications.
    pub jsonrpc_id: Option<Value>,
}

impl ProtocolMessage {
    /// Wrap a routed operation, taking the label and the JSON-RPC id
    /// from the operation itself.
    pub fn new(operation: ProtocolOperation) -> Self {
        Self {
            label: operation.label(),
            jsonrpc_id: operation.client_request_id(),
            operation,
        }
    }
}

/// Transport-layer rejection that bypasses the protocol handler's
/// normal response path.
///
/// The protocol handler's transport-header validation returns it to
/// signal a header / body mismatch, a missing required header, or an
/// unsupported protocol version *before* the body is dispatched.
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
    use crate::v_2026_07_28::wire::operations::{CapabilityOperation, LifecycleOperation};
    use crate::v_2026_07_28::wire::tools::ToolCallParams;
    use serde_json::json;

    #[test]
    fn new_takes_label_and_id_from_the_operation() {
        let msg = ProtocolMessage::new(ProtocolOperation::Capabilities(
            CapabilityOperation::ToolsCall {
                request_id: json!(7),
                params: ToolCallParams {
                    name: "search".to_owned(),
                    arguments: None,
                    meta: None,
                    request_state: None,
                    input_responses: None,
                },
            },
        ));
        assert_eq!(msg.label, "capabilities.tools_call");
        assert_eq!(msg.jsonrpc_id, Some(json!(7)));
        assert!(matches!(
            msg.operation,
            ProtocolOperation::Capabilities(CapabilityOperation::ToolsCall { .. })
        ));
    }

    #[test]
    fn a_notification_carries_no_jsonrpc_id() {
        let msg = ProtocolMessage::new(ProtocolOperation::Lifecycle(
            LifecycleOperation::NotificationAccepted,
        ));
        assert_eq!(msg.label, "lifecycle.notification_accepted");
        assert!(msg.jsonrpc_id.is_none());
    }
}
