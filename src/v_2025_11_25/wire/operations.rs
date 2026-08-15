//! Operation discriminants — the typed enum the runtime matches on
//! after parsing a `ClientMessage`.
//!
//! `ProtocolOperation` is the top-level enum; it splits into four
//! sub-enums (`LifecycleOperation`, `CapabilityOperation`,
//! `TaskOperation`, `LoggingOperation`) plus a synthetic
//! `ServerRequestResponse` variant that the parser uses to route
//! client-side responses to server-initiated requests back into the
//! pipeline-resumption flow.
//!
//! These enums are the boundary the routing function in
//! `routing.rs` produces and the per-operation dispatch arms in
//! `runtime` consume. The variant shape is the wire contract —
//! adding / removing a variant is a wire-compatibility change.
//!
//! `DRAFT-2026-v1` reshapes this surface significantly: lifecycle
//! collapses (no `initialize` / `initialized`), `Logging` disappears
//! (per-request `_meta.logLevel`), tasks become an extension, and
//! `subscriptions/listen` adds a new capability arm. The
//! `v_2025_11_25` shape stays frozen for the compatibility window.

use serde::Serialize;
use serde_json::Value;

use crate::shared::jsonrpc::JsonRpcErrorBody;
use crate::v_2025_11_25::wire::common::ListParams;
use crate::v_2025_11_25::wire::completion::CompletionCompleteParams;
use crate::v_2025_11_25::wire::elicitation::ElicitationCompleteParams;
use crate::v_2025_11_25::wire::lifecycle::InitializeParams;
use crate::v_2025_11_25::wire::logging::LoggingSetLevelParams;
use crate::v_2025_11_25::wire::prompts::PromptGetParams;
use crate::v_2025_11_25::wire::resources::{ResourceReadParams, ResourceSubscribeParams};
use crate::v_2025_11_25::wire::tasks::{
    TaskCancelParams, TaskGetParams, TaskResultParams, TasksListParams,
};
use crate::v_2025_11_25::wire::tools::ToolCallParams;

/// Parsed MCP protocol operation.
///
/// `map_client_message_to_operation` converts a raw `ClientMessage`
/// into this enum by matching the JSON-RPC `method` field to the
/// correct variant (lifecycle, capability, task, logging, or
/// server-request response).
#[derive(Debug, Clone, Serialize)]
pub enum ProtocolOperation {
    Lifecycle(LifecycleOperation),
    Capabilities(CapabilityOperation),
    Tasks(TaskOperation),
    Logging(LoggingOperation),
    /// Synthetic variant carrying a client-side response to a
    /// server-initiated request (sampling / elicitation / roots).
    /// The pipeline-resumption flow looks this up by `response_id`.
    ServerRequestResponse {
        response_id: Value,
        result: Option<Value>,
        error: Option<JsonRpcErrorBody>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum LifecycleOperation {
    Initialize {
        request_id: Value,
        params: InitializeParams,
    },
    Initialized,
    Ping {
        request_id: Value,
    },
    NotificationAccepted,
    /// Client is requesting cancellation of a specific request.
    NotificationCancelled {
        request_id: Value,
        reason: Option<String>,
    },
    /// Client delivered the terminal state of a URL-mode elicitation
    /// requested by an earlier `elicitation/create`.
    /// Resumption lookup is keyed by `params.elicitationId`.
    ElicitationComplete {
        params: ElicitationCompleteParams,
    },
    /// Client's root list changed; pipelines with cached roots state
    /// should be invalidated and any consumers notified.
    RootsListChanged,
}

#[derive(Debug, Clone, Serialize)]
pub enum CapabilityOperation {
    ToolsList {
        request_id: Value,
        params: ListParams,
    },
    PromptsList {
        request_id: Value,
        params: ListParams,
    },
    PromptsGet {
        request_id: Value,
        params: PromptGetParams,
    },
    ResourcesList {
        request_id: Value,
        params: ListParams,
    },
    ResourcesRead {
        request_id: Value,
        params: ResourceReadParams,
    },
    ResourcesSubscribe {
        request_id: Value,
        params: ResourceSubscribeParams,
    },
    ResourcesUnsubscribe {
        request_id: Value,
        params: ResourceSubscribeParams,
    },
    ResourcesTemplatesList {
        request_id: Value,
        params: ListParams,
    },
    ToolsCall {
        request_id: Value,
        params: ToolCallParams,
    },
    Complete {
        request_id: Value,
        params: CompletionCompleteParams,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum LoggingOperation {
    SetLevel {
        request_id: Value,
        params: LoggingSetLevelParams,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum TaskOperation {
    Get {
        request_id: Value,
        params: TaskGetParams,
    },
    Result {
        request_id: Value,
        params: TaskResultParams,
    },
    Cancel {
        request_id: Value,
        params: TaskCancelParams,
    },
    List {
        request_id: Value,
        params: TasksListParams,
    },
}

impl ProtocolOperation {
    /// Short stable label used for metrics / log spans / `ProtocolMessage`
    /// identification. Matches the back-half of the existing
    /// `GatewayOperation::label()` strings (`"lifecycle.initialize"` etc.)
    /// without the `"protocol."` prefix — protocol operations are the
    /// only ones flowing through this enum, so the prefix would be
    /// redundant here.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lifecycle(LifecycleOperation::Initialize { .. }) => "lifecycle.initialize",
            Self::Lifecycle(LifecycleOperation::Initialized) => "lifecycle.initialized",
            Self::Lifecycle(LifecycleOperation::Ping { .. }) => "lifecycle.ping",
            Self::Lifecycle(LifecycleOperation::NotificationAccepted) => {
                "lifecycle.notification_accepted"
            }
            Self::Lifecycle(LifecycleOperation::NotificationCancelled { .. }) => {
                "lifecycle.notification_cancelled"
            }
            Self::Lifecycle(LifecycleOperation::ElicitationComplete { .. }) => {
                "lifecycle.elicitation_complete"
            }
            Self::Lifecycle(LifecycleOperation::RootsListChanged) => "lifecycle.roots_list_changed",
            Self::Capabilities(CapabilityOperation::ToolsList { .. }) => "capabilities.tools_list",
            Self::Capabilities(CapabilityOperation::PromptsList { .. }) => {
                "capabilities.prompts_list"
            }
            Self::Capabilities(CapabilityOperation::PromptsGet { .. }) => {
                "capabilities.prompts_get"
            }
            Self::Capabilities(CapabilityOperation::ResourcesList { .. }) => {
                "capabilities.resources_list"
            }
            Self::Capabilities(CapabilityOperation::ResourcesRead { .. }) => {
                "capabilities.resources_read"
            }
            Self::Capabilities(CapabilityOperation::ResourcesSubscribe { .. }) => {
                "capabilities.resources_subscribe"
            }
            Self::Capabilities(CapabilityOperation::ResourcesUnsubscribe { .. }) => {
                "capabilities.resources_unsubscribe"
            }
            Self::Capabilities(CapabilityOperation::ResourcesTemplatesList { .. }) => {
                "capabilities.resources_templates_list"
            }
            Self::Capabilities(CapabilityOperation::ToolsCall { .. }) => "capabilities.tools_call",
            Self::Capabilities(CapabilityOperation::Complete { .. }) => {
                "capabilities.completion_complete"
            }
            Self::Tasks(TaskOperation::Get { .. }) => "tasks.get",
            Self::Tasks(TaskOperation::Result { .. }) => "tasks.result",
            Self::Tasks(TaskOperation::Cancel { .. }) => "tasks.cancel",
            Self::Tasks(TaskOperation::List { .. }) => "tasks.list",
            Self::Logging(LoggingOperation::SetLevel { .. }) => "logging.set_level",
            Self::ServerRequestResponse { .. } => "server_request_response",
        }
    }

    /// JSON-RPC `id` of a client-initiated request. `None` for
    /// notifications and for server-response bounces (which are
    /// server-minted and already unique by construction).
    pub fn client_request_id(&self) -> Option<Value> {
        match self {
            Self::Lifecycle(l) => match l {
                LifecycleOperation::Initialize { request_id, .. }
                | LifecycleOperation::Ping { request_id } => Some(request_id.clone()),
                // `notifications/cancelled` carries the id of the request it
                // TARGETS, which is not an id of its own — a notification has
                // none. Returning it here would enter the target's id into the
                // per-session duplicate-id tracker (a second time, since the
                // targeted request already recorded it) and reject the
                // cancellation as a duplicate, which is to say: make every
                // cancellation of a request you actually issued impossible.
                LifecycleOperation::NotificationCancelled { .. }
                | LifecycleOperation::Initialized
                | LifecycleOperation::NotificationAccepted
                | LifecycleOperation::ElicitationComplete { .. }
                | LifecycleOperation::RootsListChanged => None,
            },
            Self::Capabilities(c) => match c {
                CapabilityOperation::ToolsList { request_id, .. }
                | CapabilityOperation::PromptsList { request_id, .. }
                | CapabilityOperation::PromptsGet { request_id, .. }
                | CapabilityOperation::ResourcesList { request_id, .. }
                | CapabilityOperation::ResourcesRead { request_id, .. }
                | CapabilityOperation::ResourcesSubscribe { request_id, .. }
                | CapabilityOperation::ResourcesUnsubscribe { request_id, .. }
                | CapabilityOperation::ResourcesTemplatesList { request_id, .. }
                | CapabilityOperation::ToolsCall { request_id, .. }
                | CapabilityOperation::Complete { request_id, .. } => Some(request_id.clone()),
            },
            Self::Tasks(t) => match t {
                TaskOperation::Get { request_id, .. }
                | TaskOperation::Result { request_id, .. }
                | TaskOperation::Cancel { request_id, .. }
                | TaskOperation::List { request_id, .. } => Some(request_id.clone()),
            },
            Self::Logging(LoggingOperation::SetLevel { request_id, .. }) => {
                Some(request_id.clone())
            }
            Self::ServerRequestResponse { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn label_for_lifecycle_initialize() {
        let op = ProtocolOperation::Lifecycle(LifecycleOperation::Initialize {
            request_id: json!(1),
            params: InitializeParams {
                protocol_version: "2025-11-25".to_owned(),
                capabilities: Default::default(),
                client_info: crate::v_2025_11_25::wire::lifecycle::ImplementationInfo {
                    name: "x".to_owned(),
                    title: None,
                    version: "0".to_owned(),
                    description: None,
                    icons: None,
                    website_url: None,
                },
            },
        });
        assert_eq!(op.label(), "lifecycle.initialize");
        assert_eq!(op.client_request_id(), Some(json!(1)));
    }

    #[test]
    fn label_for_capabilities_tools_call() {
        let op = ProtocolOperation::Capabilities(CapabilityOperation::ToolsCall {
            request_id: json!(42),
            params: crate::v_2025_11_25::wire::tools::ToolCallParams {
                name: "x".to_owned(),
                arguments: None,
                meta: None,
                task: None,
            },
        });
        assert_eq!(op.label(), "capabilities.tools_call");
        assert_eq!(op.client_request_id(), Some(json!(42)));
    }

    #[test]
    fn client_request_id_none_for_notifications_and_responses() {
        let initialized = ProtocolOperation::Lifecycle(LifecycleOperation::Initialized);
        assert_eq!(initialized.label(), "lifecycle.initialized");
        assert!(initialized.client_request_id().is_none());

        let bounce = ProtocolOperation::ServerRequestResponse {
            response_id: json!("srv-1"),
            result: None,
            error: None,
        };
        assert_eq!(bounce.label(), "server_request_response");
        assert!(bounce.client_request_id().is_none());

        // The id on `notifications/cancelled` belongs to the request being
        // cancelled, not to the notification. Reporting it as this message's
        // own id re-enters it into the per-session duplicate-id tracker and
        // rejects the cancellation with -32600, which made cancelling any
        // request the client had actually issued impossible.
        let cancelled = ProtocolOperation::Lifecycle(LifecycleOperation::NotificationCancelled {
            request_id: json!(42),
            reason: Some("user aborted".to_owned()),
        });
        assert!(
            cancelled.client_request_id().is_none(),
            "a notification has no request id of its own"
        );
    }
}
