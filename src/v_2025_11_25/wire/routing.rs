//! Method-name → operation router for MCP revision `2025-11-25`.
//!
//! [`map_client_message_to_operation`] is the single entry point that
//! converts a parsed [`ClientMessage`] into the typed
//! [`ProtocolOperation`] the runtime's dispatch loop matches on.
//!
//! The function is one big `match` on the JSON-RPC `method` string,
//! per-method. Each arm:
//! 1. Pulls out the request's `params`.
//! 2. Deserializes into the version-specific params struct.
//! 3. Wraps in the matching [`ProtocolOperation`] variant.
//!
//! Notifications take a smaller branch (they have no `id`), and
//! `ClientMessage::Response` bounces through the synthetic
//! `ProtocolOperation::ServerRequestResponse` variant for pipeline
//! resumption.
//!
//! ## Modern counterpart
//!
//! [`v_2026_07_28::wire::routing`](crate::v_2026_07_28::wire::routing)
//! is the same layer for the modern revision, with its own method set.

use crate::shared::error::ProtocolError;
use crate::shared::jsonrpc::{ClientMessage, JSONRPC_VERSION};
use crate::shared::routing as shared_routing;
use crate::v_2025_11_25::wire::common::{CancelledNotificationParams, ListParams};
use crate::v_2025_11_25::wire::completion::CompletionCompleteParams;
use crate::v_2025_11_25::wire::elicitation::ElicitationCompleteParams;
use crate::v_2025_11_25::wire::lifecycle::InitializeParams;
use crate::v_2025_11_25::wire::logging::LoggingSetLevelParams;
use crate::v_2025_11_25::wire::operations::{
    CapabilityOperation, LifecycleOperation, LoggingOperation, ProtocolOperation, TaskOperation,
};
use crate::v_2025_11_25::wire::prompts::PromptGetParams;
use crate::v_2025_11_25::wire::resources::{ResourceReadParams, ResourceSubscribeParams};
use crate::v_2025_11_25::wire::tasks::{
    TaskCancelParams, TaskGetParams, TaskResultParams, TasksListParams,
};
use crate::v_2025_11_25::wire::tools::ToolCallParams;

/// Route a parsed [`ClientMessage`] to the appropriate
/// [`ProtocolOperation`] variant.
///
/// Validates the JSON-RPC version and `id`, then matches the
/// `method` field to the correct MCP operation. Unknown methods
/// return [`ProtocolError::method_not_found`]. For responses (no
/// `method`), routes to
/// [`ProtocolOperation::ServerRequestResponse`] for pipeline
/// resumption.
pub fn map_client_message_to_operation(
    message: ClientMessage,
) -> Result<ProtocolOperation, ProtocolError> {
    match message {
        ClientMessage::Request(request) => {
            if request.jsonrpc != JSONRPC_VERSION {
                return Err(ProtocolError::invalid_request(
                    "unsupported jsonrpc version",
                ));
            }

            match request.method.as_str() {
                "initialize" => {
                    let params: InitializeParams =
                        shared_routing::required_params(&request, "initialize")?;

                    Ok(ProtocolOperation::Lifecycle(
                        LifecycleOperation::Initialize {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "tools/list" => {
                    let params: ListParams =
                        shared_routing::optional_params(&request, "tools/list")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ToolsList {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "prompts/list" => {
                    let params: ListParams =
                        shared_routing::optional_params(&request, "prompts/list")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::PromptsList {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "prompts/get" => {
                    let params: PromptGetParams =
                        shared_routing::required_params(&request, "prompts/get")?;

                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::PromptsGet {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "resources/list" => {
                    let params: ListParams =
                        shared_routing::optional_params(&request, "resources/list")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ResourcesList {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "resources/read" => {
                    let params: ResourceReadParams =
                        shared_routing::required_params(&request, "resources/read")?;

                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ResourcesRead {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "resources/subscribe" => {
                    let params: ResourceSubscribeParams =
                        shared_routing::required_params(&request, "resources/subscribe")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ResourcesSubscribe {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "resources/unsubscribe" => {
                    let params: ResourceSubscribeParams =
                        shared_routing::required_params(&request, "resources/unsubscribe")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ResourcesUnsubscribe {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "resources/templates/list" => {
                    let params: ListParams =
                        shared_routing::optional_params(&request, "resources/templates/list")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ResourcesTemplatesList {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "tools/call" => {
                    let params: ToolCallParams =
                        shared_routing::required_params(&request, "tools/call")?;

                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ToolsCall {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "completion/complete" => {
                    let params: CompletionCompleteParams =
                        shared_routing::required_params(&request, "completion/complete")?;

                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::Complete {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                "logging/setLevel" => {
                    let params: LoggingSetLevelParams =
                        shared_routing::required_params(&request, "logging/setLevel")?;

                    Ok(ProtocolOperation::Logging(LoggingOperation::SetLevel {
                        request_id: request.id,
                        params,
                    }))
                }
                "ping" => Ok(ProtocolOperation::Lifecycle(LifecycleOperation::Ping {
                    request_id: request.id,
                })),
                "tasks/get" => {
                    let params: TaskGetParams =
                        shared_routing::required_params(&request, "tasks/get")?;
                    Ok(ProtocolOperation::Tasks(TaskOperation::Get {
                        request_id: request.id,
                        params,
                    }))
                }
                "tasks/result" => {
                    let params: TaskResultParams =
                        shared_routing::required_params(&request, "tasks/result")?;
                    Ok(ProtocolOperation::Tasks(TaskOperation::Result {
                        request_id: request.id,
                        params,
                    }))
                }
                "tasks/cancel" => {
                    let params: TaskCancelParams =
                        shared_routing::required_params(&request, "tasks/cancel")?;
                    Ok(ProtocolOperation::Tasks(TaskOperation::Cancel {
                        request_id: request.id,
                        params,
                    }))
                }
                "tasks/list" => {
                    let params: TasksListParams = request
                        .params
                        .map(serde_json::from_value)
                        .transpose()
                        .map_err(|error| {
                            ProtocolError::invalid_params(
                                Some(request.id.clone()),
                                format!("invalid tasks/list params: {error}"),
                                None,
                            )
                        })?
                        .unwrap_or(TasksListParams { cursor: None });
                    Ok(ProtocolOperation::Tasks(TaskOperation::List {
                        request_id: request.id,
                        params,
                    }))
                }
                method => Err(ProtocolError::method_not_found(Some(request.id), method)),
            }
        }
        ClientMessage::Notification(notification) => {
            if notification.jsonrpc != JSONRPC_VERSION {
                return Err(ProtocolError::invalid_request(
                    "unsupported jsonrpc version",
                ));
            }

            match notification.method.as_str() {
                "notifications/initialized" => Ok(ProtocolOperation::Lifecycle(
                    LifecycleOperation::Initialized,
                )),
                "notifications/cancelled" => match notification.params {
                    // Absent params → accepted (no cancel target); a
                    // present-but-malformed body is a client error, not a
                    // silent no-op.
                    None => Ok(ProtocolOperation::Lifecycle(
                        LifecycleOperation::NotificationAccepted,
                    )),
                    Some(value) => {
                        let params: CancelledNotificationParams = serde_json::from_value(value)
                            .map_err(|error| {
                                ProtocolError::invalid_params(
                                    None,
                                    format!("invalid notifications/cancelled params: {error}"),
                                    None,
                                )
                            })?;
                        Ok(ProtocolOperation::Lifecycle(
                            LifecycleOperation::NotificationCancelled {
                                request_id: params.request_id,
                                reason: params.reason,
                            },
                        ))
                    }
                },
                "notifications/roots/list_changed" => Ok(ProtocolOperation::Lifecycle(
                    LifecycleOperation::RootsListChanged,
                )),
                "notifications/elicitation/complete" => {
                    let params_value = notification.params.ok_or_else(|| {
                        ProtocolError::invalid_params(
                            None,
                            "notifications/elicitation/complete requires params",
                            None,
                        )
                    })?;
                    let params: ElicitationCompleteParams = serde_json::from_value(params_value)
                        .map_err(|error| {
                            ProtocolError::invalid_params(
                                None,
                                format!("invalid elicitation/complete params: {error}"),
                                None,
                            )
                        })?;
                    Ok(ProtocolOperation::Lifecycle(
                        LifecycleOperation::ElicitationComplete { params },
                    ))
                }
                // Per MCP spec: silently ignore unknown notifications.
                // Spec-named notifications without a handler log at WARN
                // so operators notice; everything else degrades to DEBUG.
                _method => {
                    if _method.starts_with("notifications/") {
                        tracing::warn!(
                            method = _method,
                            "accepted unknown MCP notification; add a handler if this is a known spec method"
                        );
                    } else {
                        tracing::debug!(
                            method = _method,
                            "ignoring non-namespaced client notification"
                        );
                    }
                    Ok(ProtocolOperation::Lifecycle(
                        LifecycleOperation::NotificationAccepted,
                    ))
                }
            }
        }
        ClientMessage::Response(response) => {
            if response.jsonrpc != JSONRPC_VERSION {
                return Err(ProtocolError::invalid_request(
                    "unsupported jsonrpc version in response",
                ));
            }
            Ok(ProtocolOperation::ServerRequestResponse {
                response_id: response.id,
                result: response.result,
                error: response.error,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::jsonrpc::{
        JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, parse_client_message,
    };
    use serde_json::{Value, json};

    #[test]
    fn initialize_request_maps_to_lifecycle_operation() {
        let message = parse_client_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        }))
        .expect("message parsed");

        let operation = map_client_message_to_operation(message).expect("operation mapped");

        assert!(matches!(
            operation,
            ProtocolOperation::Lifecycle(LifecycleOperation::Initialize { .. })
        ));
    }

    #[test]
    fn tools_call_maps_with_valid_header() {
        let message = parse_client_message(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "mcpg.runtime.snapshot",
                "arguments": {}
            }
        }))
        .expect("message parsed");

        let operation = map_client_message_to_operation(message).expect("operation mapped");

        assert!(matches!(
            operation,
            ProtocolOperation::Capabilities(CapabilityOperation::ToolsCall { .. })
        ));
    }

    #[test]
    fn prompts_get_maps_with_valid_header() {
        let message = parse_client_message(json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "prompts/get",
            "params": {
                "name": "mcpg_operational_overview"
            }
        }))
        .expect("message parsed");

        let operation = map_client_message_to_operation(message).expect("operation mapped");

        assert!(matches!(
            operation,
            ProtocolOperation::Capabilities(CapabilityOperation::PromptsGet { .. })
        ));
    }

    #[test]
    fn resources_read_maps_with_valid_header() {
        let message = parse_client_message(json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "resources/read",
            "params": {
                "uri": "mcpg://runtime/overview"
            }
        }))
        .expect("message parsed");

        let operation = map_client_message_to_operation(message).expect("operation mapped");

        assert!(matches!(
            operation,
            ProtocolOperation::Capabilities(CapabilityOperation::ResourcesRead { .. })
        ));
    }

    #[test]
    fn logging_set_level_maps_with_valid_header() {
        let message = parse_client_message(json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "logging/setLevel",
            "params": {
                "level": "info"
            }
        }))
        .expect("message parsed");

        let operation = map_client_message_to_operation(message).expect("operation mapped");

        assert!(matches!(
            operation,
            ProtocolOperation::Logging(LoggingOperation::SetLevel { .. })
        ));
    }

    #[test]
    fn client_response_parses_to_server_request_response_operation() {
        let msg = ClientMessage::Response(JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: Value::String("srv-req-42".to_owned()),
            result: Some(json!({ "confirm": true })),
            error: None,
        });
        let op = map_client_message_to_operation(msg).unwrap();
        assert!(matches!(
            op,
            ProtocolOperation::ServerRequestResponse { .. }
        ));
    }

    #[test]
    fn tools_list_routing_parses_cursor_param() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: Value::Number(1.into()),
            method: "tools/list".to_owned(),
            params: Some(json!({ "cursor": "next_page" })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ToolsList { params, .. }) => {
                assert_eq!(params.cursor.as_deref(), Some("next_page"));
            }
            other => panic!("expected ToolsList, got {:?}", other),
        }
    }

    #[test]
    fn tools_list_routing_without_params_defaults() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: Value::Number(1.into()),
            method: "tools/list".to_owned(),
            params: None,
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ToolsList { params, .. }) => {
                assert!(params.cursor.is_none());
            }
            other => panic!("expected ToolsList, got {:?}", other),
        }
    }

    #[test]
    fn resources_subscribe_routing() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: Value::Number(1.into()),
            method: "resources/subscribe".to_owned(),
            params: Some(json!({ "uri": "file:///config.yaml" })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ResourcesSubscribe {
                params,
                ..
            }) => {
                assert_eq!(params.uri, "file:///config.yaml");
            }
            other => panic!("expected ResourcesSubscribe, got {:?}", other),
        }
    }

    #[test]
    fn resources_unsubscribe_routing() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: Value::Number(1.into()),
            method: "resources/unsubscribe".to_owned(),
            params: Some(json!({ "uri": "file:///config.yaml" })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ResourcesUnsubscribe {
                params,
                ..
            }) => {
                assert_eq!(params.uri, "file:///config.yaml");
            }
            other => panic!("expected ResourcesUnsubscribe, got {:?}", other),
        }
    }

    #[test]
    fn resources_templates_list_routing() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: Value::Number(1.into()),
            method: "resources/templates/list".to_owned(),
            params: None,
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ResourcesTemplatesList {
                params,
                ..
            }) => {
                assert!(params.cursor.is_none());
            }
            other => panic!("expected ResourcesTemplatesList, got {:?}", other),
        }
    }

    #[test]
    fn notifications_cancelled_routing_with_params() {
        let msg = ClientMessage::Notification(JsonRpcNotification {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: "notifications/cancelled".to_owned(),
            params: Some(json!({ "requestId": 42, "reason": "user abort" })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Lifecycle(LifecycleOperation::NotificationCancelled {
                request_id,
                reason,
            }) => {
                assert_eq!(request_id, json!(42));
                assert_eq!(reason, Some("user abort".to_owned()));
            }
            other => panic!("expected NotificationCancelled, got {:?}", other),
        }
    }

    #[test]
    fn notifications_cancelled_routing_without_params() {
        let msg = ClientMessage::Notification(JsonRpcNotification {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: "notifications/cancelled".to_owned(),
            params: None,
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Lifecycle(LifecycleOperation::NotificationAccepted) => {}
            other => panic!(
                "expected NotificationAccepted for missing params, got {:?}",
                other
            ),
        }
    }
}
