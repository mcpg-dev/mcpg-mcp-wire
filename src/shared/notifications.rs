//! Notification envelopes and params.
//!
//! - [`CancelledNotificationParams`] — body of a client's
//!   `notifications/cancelled`.
//! - [`ProgressNotification`] + [`ProgressParams`] —
//!   `notifications/progress`.
//! - [`ListChangedNotification`] — the envelope shared by
//!   `notifications/{tools,prompts,resources}/list_changed`.
//! - [`ResourceUpdatedNotification`] + [`ResourceUpdatedParams`] —
//!   `notifications/resources/updated`, delivered on a
//!   `subscriptions/listen` stream.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parameters for `notifications/cancelled`. The client identifies
/// the in-flight request by its JSON-RPC `id`; the optional
/// `reason` is a free-form human string surfaced to logs / audit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelledNotificationParams {
    pub request_id: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// JSON-RPC notification envelope for any `*/list_changed` event.
/// Identical wire shape across tools / prompts / resources — only
/// the `method` string differs at emission time.
#[derive(Debug, Clone, Serialize)]
pub struct ListChangedNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
}

/// JSON-RPC notification for progress updates (`notifications/progress`).
#[derive(Debug, Clone, Serialize)]
pub struct ProgressNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: ProgressParams,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressParams {
    pub progress_token: Value,
    pub progress: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// JSON-RPC notification for resource content updates
/// (`notifications/resources/updated`).
#[derive(Debug, Clone, Serialize)]
pub struct ResourceUpdatedNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: ResourceUpdatedParams,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceUpdatedParams {
    pub uri: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cancelled_notification_params_round_trip() {
        let v = json!({ "requestId": 42, "reason": "user abort" });
        let p: CancelledNotificationParams = serde_json::from_value(v).unwrap();
        assert_eq!(p.request_id, json!(42));
        assert_eq!(p.reason.as_deref(), Some("user abort"));
        let back = serde_json::to_value(&p).unwrap();
        assert_eq!(back, json!({ "requestId": 42, "reason": "user abort" }));
    }

    #[test]
    fn cancelled_notification_params_omit_absent_reason() {
        let p: CancelledNotificationParams =
            serde_json::from_value(json!({ "requestId": "r-1" })).unwrap();
        assert!(p.reason.is_none());
        assert_eq!(
            serde_json::to_value(&p).unwrap(),
            json!({ "requestId": "r-1" })
        );
    }

    #[test]
    fn progress_notification_serializes_camel_case() {
        let notif = ProgressNotification {
            jsonrpc: "2.0",
            method: "notifications/progress",
            params: ProgressParams {
                progress_token: json!("p1"),
                progress: 0.5,
                total: Some(1.0),
                message: Some("halfway".to_owned()),
            },
        };
        let v = serde_json::to_value(&notif).unwrap();
        assert_eq!(v["params"]["progressToken"], "p1");
        assert_eq!(v["params"]["progress"], 0.5);
        assert_eq!(v["params"]["total"], 1.0);
        assert_eq!(v["params"]["message"], "halfway");
    }

    #[test]
    fn progress_params_omit_absent_optionals() {
        let v = serde_json::to_value(ProgressParams {
            progress_token: json!(7),
            progress: 1.0,
            total: None,
            message: None,
        })
        .unwrap();
        assert_eq!(v, json!({ "progressToken": 7, "progress": 1.0 }));
    }

    #[test]
    fn list_changed_notification_serializes() {
        let notif = ListChangedNotification {
            jsonrpc: "2.0",
            method: "notifications/tools/list_changed",
        };
        let v = serde_json::to_value(&notif).unwrap();
        assert_eq!(
            v,
            json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
        );
    }

    #[test]
    fn resource_updated_notification_carries_uri() {
        let notif = ResourceUpdatedNotification {
            jsonrpc: "2.0",
            method: "notifications/resources/updated",
            params: ResourceUpdatedParams {
                uri: "file:///project/config.json".to_owned(),
            },
        };
        let json = serde_json::to_value(&notif).unwrap();
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["method"], "notifications/resources/updated");
        assert_eq!(json["params"]["uri"], "file:///project/config.json");
    }
}
