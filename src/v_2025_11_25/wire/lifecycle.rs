//! Lifecycle and capability-negotiation wire types for MCP
//! revision `2025-11-25`.
//!
//! Covers the `initialize` request / response shape plus every type
//! exchanged inside `capabilities` on both the client side
//! (`ClientCapabilities` + sub-types) and the server side
//! (`ServerCapabilities` + sub-types).
//!
//! Per-method operation enums (`LifecycleOperation` etc.) live in
//! [`operations`](super::operations) and the
//! `map_client_message_to_operation` router in [`routing`](super::routing);
//! this file owns only the *types* exchanged at lifecycle handshake time.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::content::Icon;

// ---------------------------------------------------------------------------
// `initialize` request: client → server.
// ---------------------------------------------------------------------------

/// Parameters for the `initialize` request — the first message in
/// the MCP handshake.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    pub client_info: ImplementationInfo,
}

/// Client-declared capability flags from the `initialize` request.
/// The gateway uses these to gate server-initiated requests: a
/// pipeline step that requires sampling will fail if the client did
/// not declare `sampling: {}`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub roots: Option<ClientRootsCapability>,
    #[serde(default)]
    pub sampling: Option<ClientSamplingCapability>,
    #[serde(default)]
    pub elicitation: Option<ClientElicitationCapability>,
    #[serde(default)]
    pub tasks: Option<ClientTasksCapability>,
    #[serde(default)]
    pub experimental: Option<Value>,
    /// SEP-2133 reverse-DNS-keyed extension declarations. Carried by
    /// the modern (2026-07-28) wire in
    /// `_meta.io.modelcontextprotocol/clientCapabilities.extensions`;
    /// the gateway reads it to gate the tasks extension (SEP-2663).
    /// Absent on the 2025-11-25 wire (skipped when `None`), so the
    /// legacy serialization is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, Value>>,
}

/// `capabilities.roots` declaration from the client.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientRootsCapability {
    /// Client can process `notifications/roots/list_changed`.
    #[serde(
        default,
        rename = "listChanged",
        skip_serializing_if = "Option::is_none"
    )]
    pub list_changed: Option<bool>,
}

/// `capabilities.sampling` declaration from the client.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientSamplingCapability {
    /// Client accepts tool-enabled `sampling/createMessage` requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<CapabilityFlag>,
    /// Client accepts `includeContext` on sampling requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<CapabilityFlag>,
}

/// `capabilities.elicitation` declaration from the client.
///
/// MCP 2025-11-25 compatibility rule: a bare `"elicitation": {}`
/// means the client supports form-mode elicitation; explicit
/// sub-keys opt into additional modes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientElicitationCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<CapabilityFlag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<CapabilityFlag>,
}

impl ClientCapabilities {
    /// Check if the client supports sampling with tools.
    /// Per spec: client must declare `{ sampling: { tools: {} } }`.
    pub fn supports_sampling_tools(&self) -> bool {
        self.sampling
            .as_ref()
            .and_then(|s| s.tools.as_ref())
            .is_some()
    }

    /// Check if the client supports sampling context inclusion.
    pub fn supports_sampling_context(&self) -> bool {
        self.sampling
            .as_ref()
            .and_then(|s| s.context.as_ref())
            .is_some()
    }

    /// Client supports sampling at all (advertised `sampling: {...}`).
    pub fn supports_sampling(&self) -> bool {
        self.sampling.is_some()
    }

    /// Client supports roots (advertised `roots: {...}`).
    pub fn supports_roots(&self) -> bool {
        self.roots.is_some()
    }

    /// Client advertised it can process
    /// `notifications/roots/list_changed`.
    pub fn supports_roots_list_changed(&self) -> bool {
        self.roots
            .as_ref()
            .and_then(|r| r.list_changed)
            .unwrap_or(false)
    }

    /// Client supports elicitation at all (advertised
    /// `elicitation: {...}`). Per MCP 2025-11-25, a bare `{}` means
    /// form-mode support is implied.
    pub fn supports_elicitation(&self) -> bool {
        self.elicitation.is_some()
    }

    /// Check if the client supports form-based elicitation.
    ///
    /// Per MCP 2025-11-25 an elicitation capability with no sub-keys
    /// (`"elicitation": {}`) implies form-mode support. An explicit
    /// `form: {}` sub-key keeps that signal. Only when the client
    /// declares other modes (e.g. `url: {}`) without `form` is
    /// form-mode considered opted out.
    pub fn supports_elicitation_form(&self) -> bool {
        let Some(elicit) = self.elicitation.as_ref() else {
            return false;
        };
        if elicit.form.is_some() {
            return true;
        }
        // Empty object (no explicit sub-keys) → form is the
        // compatibility default.
        elicit.form.is_none() && elicit.url.is_none()
    }

    /// Check if the client supports URL-based elicitation.
    pub fn supports_elicitation_url(&self) -> bool {
        self.elicitation
            .as_ref()
            .and_then(|s| s.url.as_ref())
            .is_some()
    }

    /// Check if the client supports tasks at all (advertised
    /// `tasks: {...}`).
    pub fn supports_tasks(&self) -> bool {
        self.tasks.is_some()
    }

    /// Client declared support for task-augmented
    /// `sampling/createMessage`.
    pub fn supports_task_sampling(&self) -> bool {
        self.tasks
            .as_ref()
            .and_then(|t| t.requests.as_ref())
            .and_then(|r| r.sampling.as_ref())
            .and_then(|s| s.create_message.as_ref())
            .is_some()
    }

    /// Client declared support for task-augmented
    /// `elicitation/create`.
    pub fn supports_task_elicitation(&self) -> bool {
        self.tasks
            .as_ref()
            .and_then(|t| t.requests.as_ref())
            .and_then(|r| r.elicitation.as_ref())
            .and_then(|e| e.create.as_ref())
            .is_some()
    }

    /// Client declared support for task-augmented `roots/list`.
    pub fn supports_task_roots(&self) -> bool {
        self.tasks
            .as_ref()
            .and_then(|t| t.requests.as_ref())
            .and_then(|r| r.roots.as_ref())
            .and_then(|root| root.list.as_ref())
            .is_some()
    }
}

/// Implementation identity carried by both `clientInfo` and
/// `serverInfo`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
}

// ---------------------------------------------------------------------------
// `initialize` result: server → client.
// ---------------------------------------------------------------------------

/// Server response to `initialize` — declares the negotiated protocol
/// version, server capabilities, and implementation info.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: ServerCapabilities,
    pub server_info: ImplementationInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Server capability advertisement — tells the client which MCP
/// features the gateway supports.
#[derive(Debug, Clone, Serialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<CapabilityFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<CapabilityFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<ListCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ListCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<TasksCapability>,
    /// Echo the client's `capabilities.experimental` object verbatim
    /// so clients can confirm we observed their extension
    /// declarations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>,
    /// MCP SEP-2133 extension advertisements. Each entry is keyed by
    /// the reverse-DNS extension identifier (e.g.
    /// `dev.mcpg/idempotency`) and carries the extension's negotiated
    /// parameters. Omitted on the wire when no extensions are
    /// advertised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, Value>>,
}

// ---------------------------------------------------------------------------
// Capability flags used inside both `ClientCapabilities` and
// `ServerCapabilities`.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityFlag {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCapability {
    #[serde(default)]
    pub list_changed: bool,
}

/// Resources capability — extends `ListCapability` with subscribe
/// support.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCapability {
    #[serde(default)]
    pub list_changed: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub subscribe: bool,
}

// ---------------------------------------------------------------------------
// Tasks capability sub-types (negotiation only; the task wire types —
// `Task`, `CreateTaskResult`, `tasks/*` params — live in `tasks.rs`).
// ---------------------------------------------------------------------------

/// Server-side task capability advertisement.
///
/// MCP 2025-11-25 defines the server task capability as a tree that
/// declares not just that the server understands `tasks/*` but also
/// which request types it will accept with `task` augmentation.
/// Concretely, a server that supports task-augmented `tools/call`
/// MUST include `requests.tools.call` so the client can rely on that
/// flow.
#[derive(Debug, Clone, Serialize, Default)]
pub struct TasksCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<CapabilityFlag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<CapabilityFlag>,
    /// Per-request-method task support. Absent when the server
    /// exposes no task-augmented request types.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ServerTaskRequestsCapability>,
}

/// Declares which server-bound request methods may be task-augmented.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ServerTaskRequestsCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ServerTaskToolsCapability>,
}

/// Task-augmented `tools/*` support (only `tools/call` is defined
/// today).
#[derive(Debug, Clone, Serialize, Default)]
pub struct ServerTaskToolsCapability {
    #[serde(skip_serializing_if = "Option::is_none", rename = "call")]
    pub call: Option<CapabilityFlag>,
}

/// Client-side task capability.
///
/// Clients advertise support for `tasks/*` and may declare which
/// server-initiated request methods they can serve with a `task`
/// parameter. MCPG uses this to decide whether task-augmented
/// `elicitation/create`, `sampling/createMessage`, or `roots/list`
/// requests are safe to send.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientTasksCapability {
    /// Per-request-method task support on the client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests: Option<ClientTaskRequestsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientTaskRequestsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling: Option<ClientTaskSamplingCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<ClientTaskElicitationCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<ClientTaskRootsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientTaskSamplingCapability {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "createMessage"
    )]
    pub create_message: Option<CapabilityFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientTaskElicitationCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create: Option<CapabilityFlag>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientTaskRootsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<CapabilityFlag>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_elicitation_capability_implies_form_support() {
        // MCP 2025-11-25: `"elicitation": {}` means form-mode support.
        let caps: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "elicitation": {}
        }))
        .unwrap();
        assert!(caps.supports_elicitation());
        assert!(caps.supports_elicitation_form());
        assert!(!caps.supports_elicitation_url());
    }

    #[test]
    fn url_only_elicitation_capability_does_not_imply_form() {
        // Explicit `url: {}` without `form` opts OUT of form mode.
        let caps: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "elicitation": { "url": {} }
        }))
        .unwrap();
        assert!(caps.supports_elicitation_url());
        assert!(!caps.supports_elicitation_form());
    }

    #[test]
    fn client_capabilities_sampling_tools_check() {
        let caps: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "sampling": { "tools": {} }
        }))
        .unwrap();
        assert!(caps.supports_sampling_tools());
        assert!(!caps.supports_sampling_context());
    }

    #[test]
    fn client_capabilities_elicitation_url_check() {
        let caps: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "elicitation": { "form": {}, "url": {} }
        }))
        .unwrap();
        assert!(caps.supports_elicitation_form());
        assert!(caps.supports_elicitation_url());
    }

    #[test]
    fn client_capabilities_tasks_check() {
        let caps: ClientCapabilities = serde_json::from_value(serde_json::json!({
            "tasks": { "cancel": {} }
        }))
        .unwrap();
        assert!(caps.supports_tasks());

        let caps2: ClientCapabilities = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!caps2.supports_tasks());
    }

    #[test]
    fn resource_capability_serialization() {
        let cap = ResourceCapability {
            list_changed: false,
            subscribe: true,
        };
        let json = serde_json::to_value(&cap).unwrap();
        assert_eq!(json["listChanged"], false);
        assert_eq!(json["subscribe"], true);
    }

    #[test]
    fn resource_capability_omits_subscribe_when_false() {
        let cap = ResourceCapability {
            list_changed: false,
            subscribe: false,
        };
        let json = serde_json::to_value(&cap).unwrap();
        assert!(json.get("subscribe").is_none());
    }
}
