//! Tasks wire types for MCP revision `2025-11-25`.
//!
//! Covers the full `tasks/*` method family plus the
//! task-augmentation hook on `tools/call`:
//!
//! - Core task object: [`Task`], [`TaskStatus`].
//! - Result envelopes: [`CreateTaskResult`] (initial response when a
//!   request is task-augmented), [`TasksListResult`].
//! - Request params: [`TaskGetParams`], [`TaskCancelParams`],
//!   [`TaskResultParams`], [`TasksListParams`], [`TaskAugmentParams`]
//!   (the `task` sub-object on a task-augmented `tools/call`).
//! - Server-pushed status notification: [`TaskStatusNotification`]
//!   (+ [`TaskStatusNotificationParams`]).
//! - Well-known `_meta` keys: [`RELATED_TASK_META_KEY`],
//!   [`MODEL_IMMEDIATE_RESPONSE_META_KEY`].
//! - Constructor helper: [`related_task_meta`].
//!
//! ## Modern counterpart
//!
//! SEP-2663 rewrites this surface on `2026-07-28`: tasks are an extension
//! (`io.modelcontextprotocol/tasks`), blocking `tasks/result` and `tasks/list`
//! are gone, `tasks/update` is added, the request-time `task` opt-in is gone
//! (the server decides per request), and the result discriminator is
//! `resultType: "task"`. Those shapes live in
//! `v_2026_07_28::extensions::tasks::wire`; the ones here are frozen for the
//! `2025-11-25` compatibility window.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Core task object.
// ---------------------------------------------------------------------------

/// Task status as defined by MCP 2025-11-25.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Working,
    Completed,
    Failed,
    Cancelled,
    /// Task needs additional user input (via elicitation) before
    /// proceeding.
    InputRequired,
}

/// Task state representation for storage and serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub task_id: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub created_at: String,
    pub last_updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

// ---------------------------------------------------------------------------
// Result envelopes.
// ---------------------------------------------------------------------------

/// The initial result returned when a task is spawned by a
/// task-augmented request.
#[derive(Debug, Clone, Serialize)]
pub struct CreateTaskResult {
    pub task: Task,
}

/// Result for `tasks/list`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TasksListResult {
    pub tasks: Vec<Task>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// ---------------------------------------------------------------------------
// `notifications/tasks/status` server push.
// ---------------------------------------------------------------------------

/// JSON-RPC notification for task status changes, sent via SSE.
/// Method: `notifications/tasks/status`.
#[derive(Debug, Clone, Serialize)]
pub struct TaskStatusNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: TaskStatusNotificationParams,
}

/// Parameters for task status notifications.
///
/// Per MCP 2025-11-25 the params are effectively
/// `NotificationParams & Task`: the full [`Task`] fields (taskId,
/// status, statusMessage, createdAt, lastUpdatedAt, ttl, pollInterval)
/// are flattened into the params alongside the optional `_meta`.
/// Emitting only a subset of [`Task`] fields would break spec-aware
/// clients that rely on the full envelope.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusNotificationParams {
    #[serde(flatten)]
    pub task: Task,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

// ---------------------------------------------------------------------------
// Well-known `_meta` keys + constructor.
// ---------------------------------------------------------------------------

/// Well-known `_meta` key for task correlation per MCP spec.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";

/// Well-known `_meta` key for model-immediate-response per MCP spec.
/// When a task is created, the server MAY include this in
/// `CreateTaskResult._meta` with an immediate string the model can
/// use while the task runs in background.
pub const MODEL_IMMEDIATE_RESPONSE_META_KEY: &str =
    "io.modelcontextprotocol/model-immediate-response";

/// Build a `_meta` value with task correlation metadata.
/// Inject `_meta.io.modelcontextprotocol/related-task: { taskId }` into a
/// server-initiated request's `params`, preserving any `_meta` entries already
/// there (traceparent, audit fields) so they keep round-tripping. Non-object
/// `params` are coerced to an object so the injection always lands.
pub fn inject_related_task_meta(mut params: Value, task_id: &str) -> Value {
    if !params.is_object() {
        params = serde_json::json!({});
    }
    if let Some(obj) = params.as_object_mut() {
        let meta_entry = obj.entry("_meta").or_insert_with(|| serde_json::json!({}));
        if let Some(meta_obj) = meta_entry.as_object_mut() {
            meta_obj.insert(
                RELATED_TASK_META_KEY.to_owned(),
                serde_json::json!({ "taskId": task_id }),
            );
        }
    }
    params
}

pub fn related_task_meta(task_id: &str) -> Value {
    serde_json::json!({
        RELATED_TASK_META_KEY: { "taskId": task_id }
    })
}

// ---------------------------------------------------------------------------
// Request parameters.
// ---------------------------------------------------------------------------

/// Parameters for a task-augmented `tools/call` — the `task`
/// sub-object that opts the call into task semantics (2025-11-25
/// only; DRAFT-2026-v1 drops the request-time opt-in).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAugmentParams {
    #[serde(default)]
    pub ttl: Option<u64>,
}

/// Parameters for `tasks/get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGetParams {
    pub task_id: String,
}

/// Parameters for `tasks/cancel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCancelParams {
    pub task_id: String,
}

/// Parameters for `tasks/result`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResultParams {
    pub task_id: String,
}

/// Parameters for `tasks/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasksListParams {
    #[serde(default)]
    pub cursor: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(TaskStatus::Working).unwrap(),
            "working"
        );
        assert_eq!(
            serde_json::to_value(TaskStatus::Completed).unwrap(),
            "completed"
        );
        assert_eq!(serde_json::to_value(TaskStatus::Failed).unwrap(), "failed");
        assert_eq!(
            serde_json::to_value(TaskStatus::Cancelled).unwrap(),
            "cancelled"
        );
    }

    #[test]
    fn task_status_input_required_serializes() {
        let status = TaskStatus::InputRequired;
        let json = serde_json::to_value(status).unwrap();
        assert_eq!(json, "input_required");
    }

    #[test]
    fn task_status_input_required_deserializes() {
        let status: TaskStatus =
            serde_json::from_value(serde_json::json!("input_required")).unwrap();
        assert_eq!(status, TaskStatus::InputRequired);
    }

    #[test]
    fn task_serializes_camel_case_and_omits_optionals() {
        let task = Task {
            task_id: "task-x".to_owned(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: "2026-04-12T00:00:00Z".to_owned(),
            last_updated_at: "2026-04-12T00:00:00Z".to_owned(),
            ttl: None,
            poll_interval: None,
        };
        let v = serde_json::to_value(&task).unwrap();
        assert_eq!(v["taskId"], "task-x");
        assert_eq!(v["status"], "working");
        assert_eq!(v["createdAt"], "2026-04-12T00:00:00Z");
        assert_eq!(v["lastUpdatedAt"], "2026-04-12T00:00:00Z");
        assert!(v.get("statusMessage").is_none());
        assert!(v.get("ttl").is_none());
        assert!(v.get("pollInterval").is_none());
    }

    #[test]
    fn task_status_notification_carries_full_task_and_related_meta() {
        let task = Task {
            task_id: "task-abc".to_owned(),
            status: TaskStatus::Completed,
            status_message: Some("Done".to_owned()),
            created_at: "2026-04-12T00:00:00Z".to_owned(),
            last_updated_at: "2026-04-13T00:00:00Z".to_owned(),
            ttl: Some(60_000),
            poll_interval: Some(2_000),
        };
        let notif = TaskStatusNotification {
            jsonrpc: "2.0",
            method: "notifications/tasks/status",
            params: TaskStatusNotificationParams {
                task: task.clone(),
                meta: Some(related_task_meta(&task.task_id)),
            },
        };
        let v = serde_json::to_value(&notif).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["method"], "notifications/tasks/status");
        // Full Task fields must flatten onto params so spec-aware
        // clients see every task attribute, not just a status summary.
        assert_eq!(v["params"]["taskId"], "task-abc");
        assert_eq!(v["params"]["status"], "completed");
        assert_eq!(v["params"]["statusMessage"], "Done");
        assert_eq!(v["params"]["createdAt"], "2026-04-12T00:00:00Z");
        assert_eq!(v["params"]["lastUpdatedAt"], "2026-04-13T00:00:00Z");
        assert_eq!(v["params"]["ttl"], 60_000);
        assert_eq!(v["params"]["pollInterval"], 2_000);
        assert_eq!(
            v["params"]["_meta"][RELATED_TASK_META_KEY]["taskId"],
            "task-abc"
        );
    }

    #[test]
    fn related_task_meta_helper() {
        let meta = related_task_meta("task-42");
        let obj = meta.as_object().unwrap();
        let inner = obj.get(RELATED_TASK_META_KEY).unwrap();
        assert_eq!(inner["taskId"], "task-42");
    }

    #[test]
    fn task_get_cancel_result_params_deserialize_camel_case_task_id() {
        let v = json!({ "taskId": "task-1" });
        let g: TaskGetParams = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(g.task_id, "task-1");
        let c: TaskCancelParams = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(c.task_id, "task-1");
        let r: TaskResultParams = serde_json::from_value(v).unwrap();
        assert_eq!(r.task_id, "task-1");
    }

    #[test]
    fn tasks_list_params_default_has_no_cursor() {
        let params: TasksListParams = serde_json::from_value(json!({})).unwrap();
        assert!(params.cursor.is_none());
    }

    #[test]
    fn tasks_list_result_omits_null_cursor() {
        let r = TasksListResult {
            tasks: vec![],
            next_cursor: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert!(v.get("nextCursor").is_none());
    }

    #[test]
    fn task_augment_params_default_has_no_ttl() {
        let params: TaskAugmentParams = serde_json::from_value(json!({})).unwrap();
        assert!(params.ttl.is_none());
    }

    #[test]
    fn model_immediate_response_meta_key_is_canonical() {
        assert_eq!(
            MODEL_IMMEDIATE_RESPONSE_META_KEY,
            "io.modelcontextprotocol/model-immediate-response"
        );
    }
}
