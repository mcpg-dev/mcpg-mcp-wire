//! Method-name → operation router for MCP revision `2026-07-28`.
//!
//! Mirror of [`v_2025_11_25::wire::routing`](crate::v_2025_11_25::wire::routing)
//! for the modern method surface. The body is the same shape — one
//! `match` on `method`, each arm parses the params into a typed
//! struct and wraps in a [`ProtocolOperation`] variant — but the
//! method set is different:
//!
//! - **`server/discover`** is the lone discovery method (replaces
//!   `initialize`).
//! - **`tools/{list,call}`**, **`prompts/{list,get}`**,
//!   **`resources/{list,read}`**, **`resources/templates/list`**,
//!   and **`completion/complete`** are the capability methods.
//! - **`subscriptions/listen`** and the tasks-extension methods
//!   (`io.modelcontextprotocol/tasks/*`) route to their respective
//!   dispatch arms.
//! - **Legacy methods that DON'T exist in modern** return
//!   `method_not_found`: `initialize`, `notifications/initialized`,
//!   `ping`, `logging/setLevel`, `resources/{subscribe,unsubscribe}`,
//!   `tasks/result`, `tasks/list`. The modern handler refuses
//!   them rather than silently accepting under the modern wire.
//!
//! Notifications follow the same pattern as legacy: known names
//! route to typed lifecycle variants; unknown spec-namespaced names
//! log at WARN; unknown non-namespaced names log at DEBUG; both
//! return `NotificationAccepted`.

use crate::shared::error::ProtocolError;
use crate::shared::jsonrpc::{ClientMessage, JSONRPC_VERSION};
use crate::shared::routing as shared_routing;
use crate::v_2025_11_25::wire::common::CancelledNotificationParams;
use crate::v_2026_07_28::extensions::tasks::wire::{
    CancelTaskParams, GetTaskParams, METHOD_CANCEL_TASK, METHOD_GET_TASK, METHOD_UPDATE_TASK,
    UpdateTaskParams,
};
use crate::v_2026_07_28::wire::completion::CompletionCompleteParams;
use crate::v_2026_07_28::wire::lifecycle::DiscoverParams;
use crate::v_2026_07_28::wire::operations::{
    CapabilityOperation, LifecycleOperation, ProtocolOperation, TasksExtensionOperation,
};
use crate::v_2026_07_28::wire::prompts::{PromptGetParams, PromptsListParams};
use crate::v_2026_07_28::wire::resources::{
    ResourceReadParams, ResourceTemplatesListParams, ResourcesListParams,
};
use crate::v_2026_07_28::wire::subscriptions::SubscriptionsListenParams;
use crate::v_2026_07_28::wire::tools::{ToolCallParams, ToolsListParams};

/// Route a parsed [`ClientMessage`] to the modern
/// [`ProtocolOperation`] variant.
///
/// `Err(ProtocolError)` for: unsupported JSON-RPC version, missing /
/// invalid params on methods that require them, and unknown methods.
/// Unknown notifications never error — they log + return
/// `NotificationAccepted`.
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
                "server/discover" => {
                    let params: DiscoverParams =
                        shared_routing::required_params(&request, "server/discover")?;
                    Ok(ProtocolOperation::Lifecycle(LifecycleOperation::Discover {
                        request_id: request.id,
                        params,
                    }))
                }
                "tools/list" => {
                    let params: ToolsListParams =
                        shared_routing::optional_params(&request, "tools/list")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ToolsList {
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
                "prompts/list" => {
                    let params: PromptsListParams =
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
                    let params: ResourcesListParams =
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
                "resources/templates/list" => {
                    let params: ResourceTemplatesListParams =
                        shared_routing::optional_params(&request, "resources/templates/list")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::ResourcesTemplatesList {
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
                "subscriptions/listen" => {
                    let params: SubscriptionsListenParams =
                        shared_routing::required_params(&request, "subscriptions/listen")?;
                    Ok(ProtocolOperation::Capabilities(
                        CapabilityOperation::SubscriptionsListen {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                // SEP-2663 tasks-extension methods (bare). There is
                // no `createTask` — tasks materialize server-side
                // during `tools/call`.
                method if method == METHOD_GET_TASK => {
                    let params: GetTaskParams =
                        shared_routing::required_params(&request, "subscriptions/listen")?;
                    Ok(ProtocolOperation::TasksExtension(
                        TasksExtensionOperation::GetTask {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                method if method == METHOD_CANCEL_TASK => {
                    let params: CancelTaskParams =
                        shared_routing::required_params(&request, "subscriptions/listen")?;
                    Ok(ProtocolOperation::TasksExtension(
                        TasksExtensionOperation::CancelTask {
                            request_id: request.id,
                            params,
                        },
                    ))
                }
                method if method == METHOD_UPDATE_TASK => {
                    let params: UpdateTaskParams =
                        shared_routing::required_params(&request, "subscriptions/listen")?;
                    Ok(ProtocolOperation::TasksExtension(
                        TasksExtensionOperation::UpdateTask {
                            request_id: request.id,
                            params,
                        },
                    ))
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
                "notifications/cancelled" => match notification.params {
                    // Absent params → accepted (no cancel target); a
                    // present-but-malformed body is a client error, not a
                    // silent no-op that leaves the tool running.
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
                // The modern revision does NOT carry
                // `notifications/initialized`, `notifications/elicitation/complete`,
                // or `notifications/roots/list_changed` (logging
                // / roots / sampling are deprecated; elicitation
                // completion flows through MRTR). Anything else
                // namespaced under `notifications/` is logged at
                // WARN; everything else at DEBUG. Both succeed
                // because the spec forbids errors on unknown
                // notifications.
                _method => {
                    if _method.starts_with("notifications/") {
                        tracing::warn!(
                            method = _method,
                            version = "2026-07-28",
                            "accepted unknown MCP notification; \
                             add a handler if this is a known spec method"
                        );
                    } else {
                        tracing::debug!(
                            method = _method,
                            version = "2026-07-28",
                            "ignoring non-namespaced client notification"
                        );
                    }
                    Ok(ProtocolOperation::Lifecycle(
                        LifecycleOperation::NotificationAccepted,
                    ))
                }
            }
        }
        ClientMessage::Response(_response) => {
            // MRTR resumption flows through the request body's
            // `_meta.io.modelcontextprotocol/inputResponses` map,
            // NOT through a separate `Response` envelope. A
            // standalone JSON-RPC response on the modern wire
            // means the client confused itself with the server.
            Err(ProtocolError::invalid_request(
                "2026-07-28 does not accept client-side JSON-RPC responses; \
                 MRTR resumption rides on the request body",
            ))
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
    fn discover_routes_to_lifecycle_discover() {
        let msg = parse_client_message(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "server/discover",
            "params": {
                "protocolVersion": "2026-07-28",
                "clientInfo": { "name": "c", "version": "0" }
            }
        }))
        .unwrap();
        let op = map_client_message_to_operation(msg).unwrap();
        assert!(matches!(
            op,
            ProtocolOperation::Lifecycle(LifecycleOperation::Discover { .. })
        ));
    }

    #[test]
    fn discover_missing_params_errors() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!(1),
            method: "server/discover".to_owned(),
            params: None,
        });
        let err = map_client_message_to_operation(msg).unwrap_err();
        assert_eq!(err.code(), -32602);
    }

    #[test]
    fn tools_list_routes_with_cursor_param() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: Value::Number(1.into()),
            method: "tools/list".to_owned(),
            params: Some(json!({ "cursor": "page-2" })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ToolsList { params, .. }) => {
                assert_eq!(params.cursor.as_deref(), Some("page-2"));
            }
            other => panic!("expected ToolsList, got {other:?}"),
        }
    }

    #[test]
    fn tools_list_routes_without_params_defaults() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!(1),
            method: "tools/list".to_owned(),
            params: None,
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ToolsList { params, .. }) => {
                assert!(params.cursor.is_none());
            }
            other => panic!("expected ToolsList, got {other:?}"),
        }
    }

    #[test]
    fn tools_call_routes_with_args() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!(2),
            method: "tools/call".to_owned(),
            params: Some(json!({ "name": "search", "arguments": { "q": "hi" } })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        match op {
            ProtocolOperation::Capabilities(CapabilityOperation::ToolsCall { params, .. }) => {
                assert_eq!(params.name, "search");
                assert_eq!(params.arguments.unwrap()["q"], "hi");
            }
            other => panic!("expected ToolsCall, got {other:?}"),
        }
    }

    #[test]
    fn prompts_get_routes_with_name() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!(3),
            method: "prompts/get".to_owned(),
            params: Some(json!({ "name": "code_review" })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        assert!(matches!(
            op,
            ProtocolOperation::Capabilities(CapabilityOperation::PromptsGet { .. })
        ));
    }

    #[test]
    fn resources_read_routes_with_uri() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!(4),
            method: "resources/read".to_owned(),
            params: Some(json!({ "uri": "mcpg://x" })),
        });
        let op = map_client_message_to_operation(msg).unwrap();
        assert!(matches!(
            op,
            ProtocolOperation::Capabilities(CapabilityOperation::ResourcesRead { .. })
        ));
    }

    #[test]
    fn resources_templates_list_routes_without_params() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!(5),
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
            other => panic!("expected ResourcesTemplatesList, got {other:?}"),
        }
    }

    #[test]
    fn legacy_methods_rejected_with_method_not_found() {
        // The modern handler must reject every method that the
        // modern wire deliberately doesn't carry. This is a guard
        // against the registry accidentally routing legacy
        // requests to the modern handler.
        for method in [
            "initialize",
            "ping",
            "logging/setLevel",
            "resources/subscribe",
            "resources/unsubscribe",
            "tasks/result",
            "tasks/list",
            // No legacy methods left to surface in this test —
            // every Phase-3+ wired surface is now reachable on
            // the modern wire. The tasks-extension methods route
            // via the `io.modelcontextprotocol/tasks/*` namespace
            // and are tested separately in 5.C's namespaced
            // routing coverage.
        ] {
            let msg = ClientMessage::Request(JsonRpcRequest {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                id: json!(1),
                method: method.to_owned(),
                params: None,
            });
            let err = map_client_message_to_operation(msg).unwrap_err();
            assert_eq!(err.code(), -32601, "{method} should be MethodNotFound");
        }
    }

    #[test]
    fn cancelled_notification_with_params_routes_to_typed_variant() {
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
                assert_eq!(reason.as_deref(), Some("user abort"));
            }
            other => panic!("expected NotificationCancelled, got {other:?}"),
        }
    }

    #[test]
    fn cancelled_notification_without_params_falls_back_to_accepted() {
        let msg = ClientMessage::Notification(JsonRpcNotification {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            method: "notifications/cancelled".to_owned(),
            params: None,
        });
        let op = map_client_message_to_operation(msg).unwrap();
        assert!(matches!(
            op,
            ProtocolOperation::Lifecycle(LifecycleOperation::NotificationAccepted)
        ));
    }

    #[test]
    fn unknown_notification_accepted_silently() {
        // The modern revision drops `notifications/initialized` and
        // `notifications/roots/list_changed`. They MUST still parse
        // as `NotificationAccepted` — the spec forbids erroring on
        // unknown notifications.
        for method in [
            "notifications/initialized",
            "notifications/roots/list_changed",
            "vendor/unknown",
        ] {
            let msg = ClientMessage::Notification(JsonRpcNotification {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                method: method.to_owned(),
                params: None,
            });
            let op = map_client_message_to_operation(msg).unwrap();
            assert!(matches!(
                op,
                ProtocolOperation::Lifecycle(LifecycleOperation::NotificationAccepted)
            ));
        }
    }

    #[test]
    fn client_response_is_rejected_under_modern_wire() {
        let msg = ClientMessage::Response(JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!("srv-1"),
            result: Some(json!({})),
            error: None,
        });
        let err = map_client_message_to_operation(msg).unwrap_err();
        assert_eq!(err.code(), -32600);
        assert!(err.message().contains("MRTR"));
    }

    #[test]
    fn tasks_extension_methods_route_to_bare_variants() {
        let cases: &[(&str, fn(&ProtocolOperation) -> bool)] = &[
            ("tasks/get", |op| {
                matches!(
                    op,
                    ProtocolOperation::TasksExtension(TasksExtensionOperation::GetTask { .. })
                )
            }),
            ("tasks/cancel", |op| {
                matches!(
                    op,
                    ProtocolOperation::TasksExtension(TasksExtensionOperation::CancelTask { .. })
                )
            }),
            ("tasks/update", |op| {
                matches!(
                    op,
                    ProtocolOperation::TasksExtension(TasksExtensionOperation::UpdateTask { .. })
                )
            }),
        ];
        for (method, matcher) in cases {
            // `tasks/update` needs `inputResponses`; get/cancel need
            // only `taskId`.
            let params = if *method == "tasks/update" {
                json!({ "taskId": "t-1", "inputResponses": {} })
            } else {
                json!({ "taskId": "t-1" })
            };
            let msg = ClientMessage::Request(JsonRpcRequest {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                id: json!(1),
                method: (*method).to_owned(),
                params: Some(params),
            });
            let op = map_client_message_to_operation(msg).unwrap();
            assert!(matcher(&op), "unexpected variant for `{method}`: {op:?}");
        }
    }

    #[test]
    fn tasks_extension_missing_params_errors() {
        for method in ["tasks/get", "tasks/cancel", "tasks/update"] {
            let msg = ClientMessage::Request(JsonRpcRequest {
                jsonrpc: JSONRPC_VERSION.to_owned(),
                id: json!(1),
                method: method.to_owned(),
                params: None,
            });
            let err = map_client_message_to_operation(msg).unwrap_err();
            assert_eq!(err.code(), -32602, "{method} should be InvalidParams");
        }
    }

    #[test]
    fn legacy_createtask_method_is_not_routed() {
        // `createTask` no longer exists on the modern wire; it falls
        // through to method-not-found.
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.to_owned(),
            id: json!(1),
            method: "io.modelcontextprotocol/tasks/createTask".to_owned(),
            params: None,
        });
        let err = map_client_message_to_operation(msg).unwrap_err();
        assert_eq!(err.code(), -32601);
    }

    #[test]
    fn unsupported_jsonrpc_version_rejected() {
        let msg = ClientMessage::Request(JsonRpcRequest {
            jsonrpc: "1.0".to_owned(),
            id: json!(1),
            method: "server/discover".to_owned(),
            params: None,
        });
        let err = map_client_message_to_operation(msg).unwrap_err();
        assert_eq!(err.code(), -32600);
    }
}
