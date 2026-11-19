//! The task record the gateway's task store persists.
//!
//! [`Task`] and [`TaskStatus`] are a storage shape: their serde form is
//! what stored rows hold, so it must stay byte-identical. The tasks
//! extension's wire shapes (SEP-2663) live in
//! [`v_2026_07_28::extensions::tasks::wire`](crate::v_2026_07_28::extensions::tasks::wire).

use serde::{Deserialize, Serialize};

/// Lifecycle state of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Working,
    Completed,
    Failed,
    Cancelled,
    /// The task needs input from the client before it can proceed.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_status_serializes_snake_case() {
        for (status, wire) in [
            (TaskStatus::Working, "working"),
            (TaskStatus::Completed, "completed"),
            (TaskStatus::Failed, "failed"),
            (TaskStatus::Cancelled, "cancelled"),
            (TaskStatus::InputRequired, "input_required"),
        ] {
            assert_eq!(serde_json::to_value(status).unwrap(), wire);
            let back: TaskStatus = serde_json::from_value(json!(wire)).unwrap();
            assert_eq!(back, status);
        }
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
        assert_eq!(
            v,
            json!({
                "taskId": "task-x",
                "status": "working",
                "createdAt": "2026-04-12T00:00:00Z",
                "lastUpdatedAt": "2026-04-12T00:00:00Z"
            })
        );
    }

    /// A stored row reads back field for field.
    #[test]
    fn task_round_trips_every_field() {
        let stored = json!({
            "taskId": "task-abc",
            "status": "input_required",
            "statusMessage": "waiting",
            "createdAt": "2026-04-12T00:00:00Z",
            "lastUpdatedAt": "2026-04-13T00:00:00Z",
            "ttl": 60_000,
            "pollInterval": 2_000
        });
        let task: Task = serde_json::from_value(stored.clone()).unwrap();
        assert_eq!(task.status, TaskStatus::InputRequired);
        assert_eq!(task.ttl, Some(60_000));
        assert_eq!(task.poll_interval, Some(2_000));
        assert_eq!(serde_json::to_value(&task).unwrap(), stored);
    }
}
