//! Operation discriminants — the modern (`2026-07-28`) typed enum
//! the dispatcher matches on after parsing a `ClientMessage`.
//!
//! Mirrors `protocol::v_2025_11_25::wire::operations::ProtocolOperation`
//! in shape but with a different variant set:
//!
//! - **No `Initialize` / `Initialized` / `Ping`** — replaced by a
//!   single `Discover`. Stateless model has no handshake follow-up.
//! - **No `Logging::SetLevel`** — per-request `_meta.logLevel`
//!   replaces session-scoped log-level subscription.
//! - **No `Tasks::*` arm** — tasks become an extension; routing
//!   for `io.modelcontextprotocol/tasks/*` lives in
//!   `v_2026_07_28/extensions/tasks/wire.rs`.
//! - **No `ResourcesSubscribe` / `ResourcesUnsubscribe`** —
//!   subscriptions move to `subscriptions/listen`.
//! - **No `Capabilities::Complete`** — `completion/complete` is
//!   handled through its own wire types and pipeline arm.
//! - **No `ServerRequestResponse`** — MRTR resumption flows
//!   through the modern transport's body-`_meta.inputResponses`
//!   path, not as a synthetic operation variant.
//!
//! The result is a smaller, flatter enum that's closer to the
//! actual method surface a modern client sees today.

use serde::Serialize;
use serde_json::Value;

use crate::v_2026_07_28::extensions::events::wire::{
    ListEventsParams, METHOD_LIST_EVENTS, METHOD_POLL_EVENTS, METHOD_SUBSCRIBE_EVENTS,
    METHOD_UNSUBSCRIBE_EVENTS, PollEventsParams, SubscribeEventsParams, UnsubscribeEventsParams,
};
use crate::v_2026_07_28::extensions::tasks::wire::{
    CancelTaskParams, GetTaskParams, UpdateTaskParams,
};
use crate::v_2026_07_28::wire::completion::CompletionCompleteParams;
use crate::v_2026_07_28::wire::lifecycle::DiscoverParams;
use crate::v_2026_07_28::wire::prompts::{PromptGetParams, PromptsListParams};
use crate::v_2026_07_28::wire::resources::{
    ResourceReadParams, ResourceTemplatesListParams, ResourcesListParams,
};
use crate::v_2026_07_28::wire::subscriptions::SubscriptionsListenParams;
use crate::v_2026_07_28::wire::tools::{ToolCallParams, ToolsListParams};

#[derive(Debug, Clone, Serialize)]
pub enum ProtocolOperation {
    Lifecycle(LifecycleOperation),
    Capabilities(CapabilityOperation),
    /// SEP-2663 tasks extension methods
    /// (`io.modelcontextprotocol/tasks/*`).
    TasksExtension(TasksExtensionOperation),
    /// MCP Events extension methods (`events/*`, experimental).
    EventsExtension(EventsExtensionOperation),
}

/// MCP Events operations. `events/stream` is absent: it is a long-lived
/// stream, not a finite request, and is not routed here.
///
/// The params of `Subscribe` hold the client's `whsec_` secret. Its
/// `Debug` is redacted, but `Serialize` is not: never serialize this
/// operation into a log or an audit record.
#[derive(Debug, Clone, Serialize)]
pub enum EventsExtensionOperation {
    List {
        request_id: Value,
        params: ListEventsParams,
    },
    Subscribe {
        request_id: Value,
        params: SubscribeEventsParams,
    },
    Unsubscribe {
        request_id: Value,
        params: UnsubscribeEventsParams,
    },
    Poll {
        request_id: Value,
        params: PollEventsParams,
    },
    /// A routed `events/*` request whose params do not parse. It is an
    /// operation rather than a routing error because only the gateway
    /// knows whether it serves the extension: while it does not, the
    /// answer is the `-32601` of any unknown method, not a `-32602` that
    /// reveals the method exists. `message` quotes no request value.
    InvalidParams {
        request_id: Value,
        method: &'static str,
        message: String,
    },
}

impl EventsExtensionOperation {
    /// The JSON-RPC `id` of the request.
    pub fn request_id(&self) -> &Value {
        match self {
            Self::List { request_id, .. }
            | Self::Subscribe { request_id, .. }
            | Self::Unsubscribe { request_id, .. }
            | Self::Poll { request_id, .. }
            | Self::InvalidParams { request_id, .. } => request_id,
        }
    }

    /// The JSON-RPC method this operation was routed from.
    pub fn method(&self) -> &'static str {
        match self {
            Self::List { .. } => METHOD_LIST_EVENTS,
            Self::Subscribe { .. } => METHOD_SUBSCRIBE_EVENTS,
            Self::Unsubscribe { .. } => METHOD_UNSUBSCRIBE_EVENTS,
            Self::Poll { .. } => METHOD_POLL_EVENTS,
            Self::InvalidParams { method, .. } => method,
        }
    }

    /// The metrics label, one per method; a request whose params did not
    /// parse counts under its method.
    pub fn label(&self) -> &'static str {
        match self.method() {
            METHOD_LIST_EVENTS => "events_ext.list",
            METHOD_SUBSCRIBE_EVENTS => "events_ext.subscribe",
            METHOD_UNSUBSCRIBE_EVENTS => "events_ext.unsubscribe",
            METHOD_POLL_EVENTS => "events_ext.poll",
            _ => "events_ext.unknown",
        }
    }
}

/// SEP-2663 tasks-extension operations. Routed by the modern
/// router when the inbound method matches one of the bare
/// `tasks/{get,update,cancel}` strings. There is no client
/// `createTask` — a task is materialized server-side during
/// `tools/call` (see the `resultType: "task"` seam).
#[derive(Debug, Clone, Serialize)]
pub enum TasksExtensionOperation {
    GetTask {
        request_id: Value,
        params: GetTaskParams,
    },
    CancelTask {
        request_id: Value,
        params: CancelTaskParams,
    },
    UpdateTask {
        request_id: Value,
        params: UpdateTaskParams,
    },
}

impl TasksExtensionOperation {
    /// Owned clone of the JSON-RPC `id` for the variant. Used by
    /// the handler's defensive arm + the dispatch arms.
    pub fn request_id_owned(&self) -> Option<Value> {
        match self {
            Self::GetTask { request_id, .. }
            | Self::CancelTask { request_id, .. }
            | Self::UpdateTask { request_id, .. } => Some(request_id.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum LifecycleOperation {
    /// `server/discover` — replaces the legacy `initialize` +
    /// `InitializeResult` + `notifications/initialized` chain with a
    /// single stateless request (SEP-2575 + SEP-2567).
    Discover {
        request_id: Value,
        params: DiscoverParams,
    },
    /// `notifications/cancelled` — client signals it has aborted a
    /// pending request. Kept across versions; same shape.
    NotificationCancelled {
        request_id: Value,
        reason: Option<String>,
    },
    /// Catch-all for client notifications that have no handler
    /// (spec or vendor extensions). MCPG accepts and silently
    /// drops; the routing layer logs at the appropriate level.
    NotificationAccepted,
}

#[derive(Debug, Clone, Serialize)]
pub enum CapabilityOperation {
    ToolsList {
        request_id: Value,
        params: ToolsListParams,
    },
    ToolsCall {
        request_id: Value,
        params: ToolCallParams,
    },
    PromptsList {
        request_id: Value,
        params: PromptsListParams,
    },
    PromptsGet {
        request_id: Value,
        params: PromptGetParams,
    },
    ResourcesList {
        request_id: Value,
        params: ResourcesListParams,
    },
    ResourcesRead {
        request_id: Value,
        params: ResourceReadParams,
    },
    ResourcesTemplatesList {
        request_id: Value,
        params: ResourceTemplatesListParams,
    },
    /// `completion/complete` — autocomplete for prompt arguments
    /// and resource-template variables. Wire shape is unchanged
    /// from 2025-11-25.
    Complete {
        request_id: Value,
        params: CompletionCompleteParams,
    },
    /// `subscriptions/listen` (SEP-2575) — long-lived POST-SSE
    /// stream that replaces the legacy GET-/mcp delivery channel
    /// and `resources/{subscribe,unsubscribe}` methods. The
    /// transport detects this variant and switches into SSE
    /// streaming mode; `Handler::dispatch` is NOT called for
    /// this operation (the response is a stream, not a finite
    /// envelope).
    SubscriptionsListen {
        request_id: Value,
        params: SubscriptionsListenParams,
    },
}

impl ProtocolOperation {
    /// Short stable label for metrics / log spans / `ProtocolMessage`
    /// identification. Mirrors the legacy enum's `label()` strings
    /// (`"lifecycle.<arm>"` / `"capabilities.<arm>"`).
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lifecycle(LifecycleOperation::Discover { .. }) => "lifecycle.discover",
            Self::Lifecycle(LifecycleOperation::NotificationCancelled { .. }) => {
                "lifecycle.notification_cancelled"
            }
            Self::Lifecycle(LifecycleOperation::NotificationAccepted) => {
                "lifecycle.notification_accepted"
            }
            Self::Capabilities(CapabilityOperation::ToolsList { .. }) => "capabilities.tools_list",
            Self::Capabilities(CapabilityOperation::ToolsCall { .. }) => "capabilities.tools_call",
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
            Self::Capabilities(CapabilityOperation::ResourcesTemplatesList { .. }) => {
                "capabilities.resources_templates_list"
            }
            Self::Capabilities(CapabilityOperation::Complete { .. }) => {
                "capabilities.completion_complete"
            }
            Self::Capabilities(CapabilityOperation::SubscriptionsListen { .. }) => {
                "capabilities.subscriptions_listen"
            }
            Self::TasksExtension(TasksExtensionOperation::GetTask { .. }) => "tasks_ext.get_task",
            Self::TasksExtension(TasksExtensionOperation::CancelTask { .. }) => {
                "tasks_ext.cancel_task"
            }
            Self::TasksExtension(TasksExtensionOperation::UpdateTask { .. }) => {
                "tasks_ext.update_task"
            }
            Self::EventsExtension(e) => e.label(),
        }
    }

    /// JSON-RPC `id` for a client-initiated request. Returns `None`
    /// for the two notification variants.
    pub fn client_request_id(&self) -> Option<Value> {
        match self {
            Self::Lifecycle(l) => match l {
                LifecycleOperation::Discover { request_id, .. } => Some(request_id.clone()),
                // `notifications/cancelled` carries the id of the request it
                // TARGETS, which is not an id of its own — a notification has
                // none. Returning it here would enter the target's id into the
                // per-session duplicate-id tracker (a second time, since the
                // targeted request already recorded it) and reject the
                // cancellation as a duplicate, which is to say: make every
                // cancellation of a request you actually issued impossible.
                // The dispatch arm reads the targeted id off the operation.
                LifecycleOperation::NotificationCancelled { .. }
                | LifecycleOperation::NotificationAccepted => None,
            },
            Self::Capabilities(c) => match c {
                CapabilityOperation::ToolsList { request_id, .. }
                | CapabilityOperation::ToolsCall { request_id, .. }
                | CapabilityOperation::PromptsList { request_id, .. }
                | CapabilityOperation::PromptsGet { request_id, .. }
                | CapabilityOperation::ResourcesList { request_id, .. }
                | CapabilityOperation::ResourcesRead { request_id, .. }
                | CapabilityOperation::ResourcesTemplatesList { request_id, .. }
                | CapabilityOperation::Complete { request_id, .. }
                | CapabilityOperation::SubscriptionsListen { request_id, .. } => {
                    Some(request_id.clone())
                }
            },
            Self::TasksExtension(t) => match t {
                TasksExtensionOperation::GetTask { request_id, .. }
                | TasksExtensionOperation::CancelTask { request_id, .. }
                | TasksExtensionOperation::UpdateTask { request_id, .. } => {
                    Some(request_id.clone())
                }
            },
            Self::EventsExtension(e) => Some(e.request_id().clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn label_for_lifecycle_discover() {
        let op = ProtocolOperation::Lifecycle(LifecycleOperation::Discover {
            request_id: json!(1),
            params: DiscoverParams {
                protocol_version: Some("2026-07-28".to_owned()),
                client_info: None,
                capabilities: Default::default(),
                meta: None,
            },
        });
        assert_eq!(op.label(), "lifecycle.discover");
        assert_eq!(op.client_request_id(), Some(json!(1)));
    }

    #[test]
    fn label_for_capabilities_tools_call() {
        let op = ProtocolOperation::Capabilities(CapabilityOperation::ToolsCall {
            request_id: json!(42),
            params: ToolCallParams {
                name: "search".to_owned(),
                arguments: None,
                meta: None,
                request_state: None,
                input_responses: None,
            },
        });
        assert_eq!(op.label(), "capabilities.tools_call");
        assert_eq!(op.client_request_id(), Some(json!(42)));
    }

    #[test]
    fn events_operations_carry_labels_methods_and_ids() {
        let list = EventsExtensionOperation::List {
            request_id: json!("l-1"),
            params: ListEventsParams::default(),
        };
        assert_eq!(list.method(), "events/list");
        let poll = EventsExtensionOperation::Poll {
            request_id: json!(9),
            params: serde_json::from_value(json!({ "name": "a" })).unwrap(),
        };
        assert_eq!(poll.method(), "events/poll");
        let unsubscribe = EventsExtensionOperation::Unsubscribe {
            request_id: json!(10),
            params: serde_json::from_value(
                json!({ "name": "a", "delivery": { "url": "https://h.example/x" } }),
            )
            .unwrap(),
        };
        assert_eq!(unsubscribe.method(), "events/unsubscribe");
        let subscribe = EventsExtensionOperation::Subscribe {
            request_id: json!(11),
            params: serde_json::from_value(json!({
                "name": "a",
                "delivery": {
                    "url": "https://h.example/x",
                    "secret": "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw"
                }
            }))
            .unwrap(),
        };
        assert_eq!(subscribe.method(), "events/subscribe");
        let invalid = EventsExtensionOperation::InvalidParams {
            request_id: json!(12),
            method: "events/subscribe",
            message: "missing field `name`".to_owned(),
        };
        assert_eq!(invalid.method(), "events/subscribe");
        for (op, label, id) in [
            (list, "events_ext.list", json!("l-1")),
            (poll, "events_ext.poll", json!(9)),
            (unsubscribe, "events_ext.unsubscribe", json!(10)),
            (subscribe, "events_ext.subscribe", json!(11)),
            (invalid, "events_ext.subscribe", json!(12)),
        ] {
            let op = ProtocolOperation::EventsExtension(op);
            assert_eq!(op.label(), label);
            assert_eq!(op.client_request_id(), Some(id));
        }
    }

    #[test]
    fn client_request_id_none_for_notifications() {
        let op = ProtocolOperation::Lifecycle(LifecycleOperation::NotificationAccepted);
        assert_eq!(op.label(), "lifecycle.notification_accepted");
        assert!(op.client_request_id().is_none());
    }

    /// A notification has no request id of its own. The id on
    /// `notifications/cancelled` belongs to the request being cancelled and is
    /// already in the per-session duplicate tracker; surfacing it here re-enters
    /// it and rejects the cancellation with `-32600`, making cancellation of any
    /// request the client actually issued impossible.
    #[test]
    fn client_request_id_none_for_notification_cancelled() {
        let op = ProtocolOperation::Lifecycle(LifecycleOperation::NotificationCancelled {
            request_id: json!("r-1"),
            reason: Some("aborted".to_owned()),
        });
        assert_eq!(op.label(), "lifecycle.notification_cancelled");
        assert_eq!(op.client_request_id(), None);
    }
}
