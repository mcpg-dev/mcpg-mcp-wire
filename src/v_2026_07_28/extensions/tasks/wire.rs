//! Wire types for the SEP-2663 `io.modelcontextprotocol/tasks`
//! extension (final 2026-07-28 shape).
//!
//! **Materialization model.** Tasks are *server-directed*. There is
//! no client `createTask` method. A long-running tool elects to go
//! async by having the server return a [`CreateTaskResult`]
//! (`resultType: "task"`) *in lieu of* the standard `tools/call`
//! result. The client opts in once, per request, by declaring the
//! `io.modelcontextprotocol/tasks` extension in
//! `_meta.io.modelcontextprotocol/clientCapabilities.extensions`; a
//! server MUST NOT return a task to a client that did not declare it.
//!
//! **Methods (bare, under the extension capability):**
//! - `tasks/get` — poll task state (and terminal `result`/`error`).
//! - `tasks/update` — the client→server channel that feeds
//!   `inputResponses` to an `input_required` task (empty ack).
//! - `tasks/cancel` — cooperative cancellation (empty ack).
//!
//! **Notification:** `notifications/tasks` over `subscriptions/listen`
//! carries the full task state.
//!
//! The extension's reverse-DNS key in `ServerCapabilities.extensions`
//! is `io.modelcontextprotocol/tasks` — see [`EXTENSION_NAMESPACE`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::v_2026_07_28::wire::mrtr::InputRequest;

// ---------------------------------------------------------------------------
// Namespace + method-name constants.
// ---------------------------------------------------------------------------

/// SEP-2133 extension key advertised in
/// `ServerCapabilities.extensions` and declared by clients in
/// per-request `clientCapabilities.extensions`.
pub const EXTENSION_NAMESPACE: &str = "io.modelcontextprotocol/tasks";

/// Poll a task's current state.
pub const METHOD_GET_TASK: &str = "tasks/get";
/// Feed `inputResponses` to an `input_required` task.
pub const METHOD_UPDATE_TASK: &str = "tasks/update";
/// Request cooperative cancellation of a task.
pub const METHOD_CANCEL_TASK: &str = "tasks/cancel";

/// Notification method for task-status changes the server pushes on
/// the `subscriptions/listen` stream.
pub const METHOD_NOTIFICATIONS_TASKS: &str = "notifications/tasks";

/// `resultType` discriminator stamped on [`CreateTaskResult`] so a
/// client can distinguish an async task handle from a standard
/// `tools/call` result. Mirrors the MRTR vocabulary
/// (`complete` / `input_required` / `task`).
pub const RESULT_TYPE_TASK: &str = "task";

// ---------------------------------------------------------------------------
// Core task object.
// ---------------------------------------------------------------------------

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Working,
    /// Task is awaiting client input. The `tasks/get` response
    /// surfaces the outstanding requests in `inputRequests`; the
    /// client answers via `tasks/update` `inputResponses`.
    InputRequired,
    Completed,
    Failed,
    Cancelled,
}

/// Task state shape (SEP-2663 final). The status-specific fields
/// (`result`, `error`, `inputRequests`) are inlined optionally; a
/// `tasks/get` response or `notifications/tasks` notification fills
/// in whichever applies to the current `status`.
///
/// `requestState` is the gateway's MRTR resume handle for an
/// `input_required` task: it is the same opaque, principal-bound
/// blob the MRTR `InputRequiredResult` carries, threaded through so
/// a `tasks/update` can resume the suspended pipeline. It rides in
/// the task `_meta`-adjacent surface and is omitted from the wire
/// when absent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub task_id: String,
    pub status: TaskStatus,
    /// Human-readable status hint (e.g., "compiling…",
    /// "cancelled by user").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last-update timestamp.
    pub last_updated_at: String,
    /// Time-to-live from creation in integer milliseconds; `null`
    /// means unlimited. The server MAY discard the task after the
    /// TTL elapses.
    pub ttl_ms: Option<u64>,
    /// Suggested client polling interval in integer milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_interval_ms: Option<u64>,
    /// Final result body — present only when `status` is
    /// `completed`. Mirrors the result type of the original request
    /// (e.g., a `ToolCallResult`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// JSON-RPC error body — present only when `status` is
    /// `failed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    /// Outstanding server→client requests — present only when
    /// `status` is `input_required`. Keyed by correlation token;
    /// the client answers each via `tasks/update` `inputResponses`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_requests: Option<BTreeMap<String, InputRequest>>,
    /// Opaque MRTR resume handle for an `input_required` task. The
    /// client echoes nothing back here (it answers via `tasks/update`
    /// keyed by `taskId`); the field is surfaced for forward-compat
    /// and omitted when the task is not awaiting input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_state: Option<String>,
}

// ---------------------------------------------------------------------------
// `CreateTaskResult` — the `resultType: "task"` materialization of a
// `tools/call`. SEP-2663 defines this as `Result & Task` (flat), so
// the task fields are inlined alongside `resultType`.
// ---------------------------------------------------------------------------

/// Returned in lieu of a standard `tools/call` result when the
/// server elects async execution. `resultType` is always `"task"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    /// Always `"task"`; defaulted on construction.
    #[serde(default = "default_result_type_task")]
    pub result_type: String,
    /// The seed task (flattened — SEP-2663 `Result & Task`).
    #[serde(flatten)]
    pub task: Task,
}

fn default_result_type_task() -> String {
    RESULT_TYPE_TASK.to_owned()
}

impl CreateTaskResult {
    pub fn new(task: Task) -> Self {
        Self {
            result_type: RESULT_TYPE_TASK.to_owned(),
            task,
        }
    }
}

// ---------------------------------------------------------------------------
// `tasks/get` — poll state (and terminal result/error).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskParams {
    pub task_id: String,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// `tasks/get` response — `Result & DetailedTask` (flat). Carries
/// `resultType: "complete"` (this is the standard result for the
/// `tasks/get` request) plus the inlined task fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskResult {
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    #[serde(flatten)]
    pub task: Task,
}

fn default_result_type_complete() -> String {
    "complete".to_owned()
}

impl GetTaskResult {
    pub fn new(task: Task) -> Self {
        Self {
            result_type: "complete".to_owned(),
            task,
        }
    }
}

// ---------------------------------------------------------------------------
// `tasks/update` — client→server `inputResponses` ack channel.
// ---------------------------------------------------------------------------

/// `tasks/update` params: the client's answers to a task's
/// outstanding `inputRequests`. Keyed by the same correlation token
/// the server emitted; the server feeds these to the suspended MRTR
/// pipeline. Empty ack on success.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskParams {
    pub task_id: String,
    /// Responses to outstanding `inputRequests`. Untyped here; the
    /// dispatch arm parses it through the MRTR
    /// [`crate::v_2026_07_28::wire::mrtr::InputResponses`]
    /// codec so a task awaiting input resumes through the exact same
    /// machinery as an inline MRTR resume.
    pub input_responses: Value,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// `tasks/update` result — empty acknowledgement carrying only the
/// required `resultType: "complete"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskResult {
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
}

impl Default for UpdateTaskResult {
    fn default() -> Self {
        Self {
            result_type: "complete".to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// `tasks/cancel` — cooperative cancellation (empty ack).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskParams {
    pub task_id: String,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// `tasks/cancel` result — empty acknowledgement carrying only the
/// required `resultType: "complete"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskResult {
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
}

impl Default for CancelTaskResult {
    fn default() -> Self {
        Self {
            result_type: "complete".to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// Server-pushed task-status notification (`notifications/tasks`),
// carried on the `subscriptions/listen` stream. Params are
// `NotificationParams & Task` — the full task flattened.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: TaskStatusNotificationParams,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusNotificationParams {
    #[serde(flatten)]
    pub task: Task,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl TaskStatusNotification {
    /// Build a fresh `notifications/tasks` notification carrying the
    /// given task state.
    pub fn new(task: Task) -> Self {
        Self {
            jsonrpc: crate::shared::jsonrpc::JSONRPC_VERSION,
            method: METHOD_NOTIFICATIONS_TASKS,
            params: TaskStatusNotificationParams { task, meta: None },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_task() -> Task {
        Task {
            task_id: "task-1".to_owned(),
            status: TaskStatus::Working,
            status_message: Some("compiling".to_owned()),
            created_at: "2026-05-23T00:00:00Z".to_owned(),
            last_updated_at: "2026-05-23T00:00:05Z".to_owned(),
            ttl_ms: Some(60_000),
            poll_interval_ms: Some(2_000),
            result: None,
            error: None,
            input_requests: None,
            request_state: None,
        }
    }

    #[test]
    fn well_known_constants_match_spec() {
        assert_eq!(EXTENSION_NAMESPACE, "io.modelcontextprotocol/tasks");
        assert_eq!(METHOD_GET_TASK, "tasks/get");
        assert_eq!(METHOD_UPDATE_TASK, "tasks/update");
        assert_eq!(METHOD_CANCEL_TASK, "tasks/cancel");
        assert_eq!(METHOD_NOTIFICATIONS_TASKS, "notifications/tasks");
        assert_eq!(RESULT_TYPE_TASK, "task");
    }

    #[test]
    fn task_status_round_trips_snake_case() {
        for (status, expected) in [
            (TaskStatus::Working, "working"),
            (TaskStatus::InputRequired, "input_required"),
            (TaskStatus::Completed, "completed"),
            (TaskStatus::Failed, "failed"),
            (TaskStatus::Cancelled, "cancelled"),
        ] {
            let v = serde_json::to_value(status).unwrap();
            assert_eq!(v, expected);
            let back: TaskStatus = serde_json::from_value(v).unwrap();
            assert_eq!(back, status);
        }
    }

    #[test]
    fn task_serializes_camel_case_with_ttl_ms_and_poll_interval_ms() {
        let v = serde_json::to_value(sample_task()).unwrap();
        assert_eq!(v["taskId"], "task-1");
        assert_eq!(v["status"], "working");
        assert_eq!(v["createdAt"], "2026-05-23T00:00:00Z");
        assert_eq!(v["lastUpdatedAt"], "2026-05-23T00:00:05Z");
        assert_eq!(v["ttlMs"], 60_000);
        assert_eq!(v["pollIntervalMs"], 2_000);
        // SEP-2663 renamed these from the draft `ttl`/`pollInterval`.
        assert!(v.get("ttl").is_none());
        assert!(v.get("pollInterval").is_none());
        // Status-specific fields are absent on a working task.
        assert!(v.get("result").is_none());
        assert!(v.get("error").is_none());
        assert!(v.get("inputRequests").is_none());
    }

    #[test]
    fn ttl_ms_serializes_null_when_unlimited() {
        let mut task = sample_task();
        task.ttl_ms = None;
        let v = serde_json::to_value(&task).unwrap();
        // `ttlMs` is `number | null` (required key), so it MUST be
        // present even when unlimited.
        assert!(v.as_object().unwrap().contains_key("ttlMs"));
        assert!(v["ttlMs"].is_null());
    }

    #[test]
    fn create_task_result_is_flat_with_result_type_task() {
        let result = CreateTaskResult::new(sample_task());
        let v = serde_json::to_value(&result).unwrap();
        assert_eq!(v["resultType"], "task");
        // Flattened — task fields sit at the top level, NOT under a
        // `task` wrapper.
        assert_eq!(v["taskId"], "task-1");
        assert_eq!(v["status"], "working");
        assert!(v.get("task").is_none());
    }

    #[test]
    fn get_task_result_completed_carries_result_and_complete_discriminator() {
        let mut task = sample_task();
        task.status = TaskStatus::Completed;
        task.result = Some(json!({ "content": [{ "type": "text", "text": "ok" }] }));
        let v = serde_json::to_value(GetTaskResult::new(task)).unwrap();
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["status"], "completed");
        assert_eq!(v["result"]["content"][0]["text"], "ok");
        assert!(v.get("task").is_none());
    }

    #[test]
    fn get_task_result_failed_carries_error() {
        let mut task = sample_task();
        task.status = TaskStatus::Failed;
        task.error = Some(json!({ "code": -32000, "message": "boom" }));
        let v = serde_json::to_value(GetTaskResult::new(task)).unwrap();
        assert_eq!(v["status"], "failed");
        assert_eq!(v["error"]["code"], -32000);
        assert_eq!(v["error"]["message"], "boom");
    }

    #[test]
    fn get_task_result_input_required_carries_input_requests() {
        let mut task = sample_task();
        task.status = TaskStatus::InputRequired;
        let mut reqs = BTreeMap::new();
        reqs.insert(
            "elic-1".to_owned(),
            InputRequest::Elicitation {
                params: json!({ "message": "confirm?" }),
            },
        );
        task.input_requests = Some(reqs);
        task.request_state = Some("opaque-blob".to_owned());
        let v = serde_json::to_value(GetTaskResult::new(task)).unwrap();
        assert_eq!(v["status"], "input_required");
        assert_eq!(v["inputRequests"]["elic-1"]["method"], "elicitation/create");
        assert_eq!(v["requestState"], "opaque-blob");
    }

    #[test]
    fn get_task_params_use_camel_case_task_id() {
        let v = serde_json::to_value(GetTaskParams {
            task_id: "t".to_owned(),
            meta: None,
        })
        .unwrap();
        assert_eq!(v["taskId"], "t");
    }

    #[test]
    fn update_task_params_carry_input_responses() {
        let p = UpdateTaskParams {
            task_id: "t".to_owned(),
            input_responses: json!({ "elic-1": { "action": "accept" } }),
            meta: None,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["taskId"], "t");
        assert_eq!(v["inputResponses"]["elic-1"]["action"], "accept");
    }

    #[test]
    fn update_and_cancel_results_are_empty_acks_with_complete_discriminator() {
        let u = serde_json::to_value(UpdateTaskResult::default()).unwrap();
        assert_eq!(u["resultType"], "complete");
        // No task body on the ack.
        assert!(u.get("taskId").is_none());
        assert!(u.get("status").is_none());

        let c = serde_json::to_value(CancelTaskResult::default()).unwrap();
        assert_eq!(c["resultType"], "complete");
        assert!(c.get("taskId").is_none());
    }

    #[test]
    fn task_status_notification_uses_bare_method_and_flattens_task() {
        let n = TaskStatusNotification::new(sample_task());
        let v = serde_json::to_value(&n).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["method"], "notifications/tasks");
        assert_eq!(v["params"]["taskId"], "task-1");
        assert_eq!(v["params"]["status"], "working");
        assert_eq!(v["params"]["statusMessage"], "compiling");
        // Notification carries no JSON-RPC id by spec.
        assert!(v.get("id").is_none());
    }
}
