//! `subscriptions/listen` wire types — SEP-2575 (Sessionless MCP) +
//! SEP-2567.
//!
//! In `2026-07-28` the legacy patchwork of long-lived delivery
//! channels (`GET /mcp` SSE stream, `resources/subscribe` /
//! `resources/unsubscribe`, `notifications/resources/updated`) is
//! replaced by a single method: **`subscriptions/listen`**.
//!
//! A client POSTs a `subscriptions/listen` request listing the
//! topics it wants to follow; the server holds the response
//! connection open and streams events as SSE messages until the
//! client disconnects or the server times out. Each event is a
//! standard JSON-RPC notification whose `params._meta` carries the
//! server-minted `subscriptionId` so the client can correlate
//! events back to the original `subscriptions/listen` request.
//!
//! Wire shape:
//!
//! ```text
//! POST /mcp                    Mcp-Protocol-Version: 2026-07-28
//!
//! { "jsonrpc": "2.0", "id": 7, "method": "subscriptions/listen",
//!   "params": {
//!     "subscriptions": [
//!       { "type": "resources/updated", "uri": "file:///x" },
//!       { "type": "tools/listChanged" }
//!     ]
//!   } }
//!
//! ── HTTP/1.1 200 OK · Content-Type: text/event-stream ──
//!
//! data: {
//!   "jsonrpc": "2.0",
//!   "id": 7,
//!   "result": { "subscriptionId": "<server-minted>" }
//! }
//!
//! data: {
//!   "jsonrpc": "2.0",
//!   "method": "notifications/resources/updated",
//!   "params": {
//!     "uri": "file:///x",
//!     "_meta": { "io.modelcontextprotocol/subscriptionId": "<id>" }
//!   }
//! }
//!
//! ...
//! ```
//!
//! This module owns only the **wire types**. The dispatch arm
//! (long-lived POST-SSE response, event-stream wiring into the
//! existing delivery bus and subscription store) lands in Phase
//! 4.F.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Method-name + well-known meta keys.
// ---------------------------------------------------------------------------

pub const METHOD_SUBSCRIPTIONS_LISTEN: &str = "subscriptions/listen";

/// `_meta` key the server stamps on every event it emits on a
/// `subscriptions/listen` stream so the client can correlate it
/// back to its subscription request.
pub const META_KEY_SUBSCRIPTION_ID: &str = "io.modelcontextprotocol/subscriptionId";

// ---------------------------------------------------------------------------
// Request params.
// ---------------------------------------------------------------------------

/// Parameters for `subscriptions/listen`. Carries the list of
/// topics the client wants to follow. The response holds the HTTP
/// connection open and streams events as SSE messages.
///
/// Two equivalent wire shapes are accepted on input:
///
/// 1. **Typed-array form** (MCPG-canonical):
///    `{ "subscriptions": [{ "type": "tools/listChanged" }, ...] }`.
///    Every variant of [`SubscriptionTarget`] is reachable.
///
/// 2. **Flag-object form** (spec / SEP-2575):
///    `{ "notifications": { "toolsListChanged": true,
///        "promptsListChanged": true, "resourcesListChanged": true } }`.
///    Boolean flags toggle list-changed subscriptions. Resource-
///    URI subscriptions aren't expressible in this shape; clients
///    that need them MUST use the typed-array form.
///
/// On serialization we emit the typed-array form (canonical).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionsListenParams {
    /// Topics to subscribe to. Order is preserved in the
    /// initial-event confirmation; the client uses it to disambiguate
    /// duplicate-typed subscriptions (e.g., two `resources/updated`
    /// entries with different URIs).
    pub subscriptions: Vec<SubscriptionTarget>,
    /// `_meta` for forward-compat (per-subscription throttling
    /// hints, trace context, etc.).
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl SubscriptionsListenParams {
    /// Build the `notifications` object the server reflects back in
    /// the acknowledgement frame — the honored subset of what the
    /// client asked for. Only the list-changed flags the server
    /// actually accepted are present (`true`); `resourceSubscriptions`
    /// lists the resource URIs for which per-resource `updated`
    /// delivery was established. An empty object means nothing was
    /// honored.
    ///
    /// `established_resources` is the set of `resources/updated` URIs the
    /// gateway registered, which is a subset of the requested ones: a URI no
    /// resource route resolves, or one the subscription store rejected (per-
    /// session limit, backend failure), produces no events and so is not
    /// honored. Taking it as an argument is what keeps the ack from claiming a
    /// subscription that will never fire — the caller cannot build this object
    /// without saying what it actually registered.
    pub fn honored_notifications(&self, established_resources: &[String]) -> Value {
        let mut tools = false;
        let mut prompts = false;
        let mut resources = false;
        for target in &self.subscriptions {
            match target {
                SubscriptionTarget::ToolsListChanged => tools = true,
                SubscriptionTarget::PromptsListChanged => prompts = true,
                SubscriptionTarget::ResourcesListChanged => resources = true,
                // Reflected from `established_resources` instead, so a target
                // the gateway skipped is not acked.
                SubscriptionTarget::ResourcesUpdated { .. } => {}
                // Task-status delivery is governed by the tasks
                // extension, not the resources/list-changed surface;
                // it is not reflected in this object.
                SubscriptionTarget::TasksStatus { .. } => {}
            }
        }
        let mut obj = serde_json::Map::new();
        if tools {
            obj.insert("toolsListChanged".to_owned(), Value::Bool(true));
        }
        if prompts {
            obj.insert("promptsListChanged".to_owned(), Value::Bool(true));
        }
        if resources {
            obj.insert("resourcesListChanged".to_owned(), Value::Bool(true));
        }
        if !established_resources.is_empty() {
            obj.insert(
                "resourceSubscriptions".to_owned(),
                Value::Array(
                    established_resources
                        .iter()
                        .map(|uri| Value::String(uri.clone()))
                        .collect(),
                ),
            );
        }
        Value::Object(obj)
    }
}

impl<'de> Deserialize<'de> for SubscriptionsListenParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        // Custom intermediate that accepts both wire shapes. The
        // typed-array form lands in `subscriptions`; the flag-object
        // form lands in `notifications` and gets translated to
        // `SubscriptionTarget` variants.
        #[derive(Deserialize)]
        struct Notifications {
            #[serde(default)]
            #[serde(alias = "tools_list_changed")]
            tools_list_changed: bool,
            #[serde(default)]
            #[serde(alias = "prompts_list_changed")]
            prompts_list_changed: bool,
            #[serde(default)]
            #[serde(alias = "resources_list_changed")]
            resources_list_changed: bool,
            #[serde(default, rename = "toolsListChanged")]
            tools_list_changed_camel: bool,
            #[serde(default, rename = "promptsListChanged")]
            prompts_list_changed_camel: bool,
            #[serde(default, rename = "resourcesListChanged")]
            resources_list_changed_camel: bool,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Wire {
            #[serde(default)]
            subscriptions: Option<Vec<SubscriptionTarget>>,
            #[serde(default)]
            notifications: Option<Notifications>,
            #[serde(rename = "_meta", default)]
            meta: Option<Value>,
        }

        let wire = Wire::deserialize(deserializer).map_err(D::Error::custom)?;
        let mut targets = wire.subscriptions.unwrap_or_default();
        if let Some(n) = wire.notifications {
            if n.tools_list_changed || n.tools_list_changed_camel {
                targets.push(SubscriptionTarget::ToolsListChanged);
            }
            if n.prompts_list_changed || n.prompts_list_changed_camel {
                targets.push(SubscriptionTarget::PromptsListChanged);
            }
            if n.resources_list_changed || n.resources_list_changed_camel {
                targets.push(SubscriptionTarget::ResourcesListChanged);
            }
        }
        Ok(SubscriptionsListenParams {
            subscriptions: targets,
            meta: wire.meta,
        })
    }
}

/// One subscription target. Tagged-union on `type`; covers every
/// notification kind the modern wire emits today.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum SubscriptionTarget {
    /// Per-resource update events
    /// (`notifications/resources/updated`). The legacy spec used
    /// `resources/subscribe` + `resources/unsubscribe`; modern
    /// subscribes via this `subscriptions/listen` entry instead.
    #[serde(rename = "resources/updated")]
    ResourcesUpdated {
        /// Resource URI being watched.
        uri: String,
    },
    /// Tool-catalog change events
    /// (`notifications/tools/listChanged`).
    #[serde(rename = "tools/listChanged")]
    ToolsListChanged,
    /// Prompt-catalog change events.
    #[serde(rename = "prompts/listChanged")]
    PromptsListChanged,
    /// Resource-catalog change events. Distinct from
    /// `resources/updated` — `listChanged` fires when entries
    /// appear/disappear; `updated` fires when an entry's content
    /// changes.
    #[serde(rename = "resources/listChanged")]
    ResourcesListChanged,
    /// SEP-2663 tasks-extension status events
    /// (`notifications/io.modelcontextprotocol/tasks/status`).
    /// Optionally scoped to a specific `taskId`; absent means the
    /// client wants every task-status event in its session.
    #[serde(rename = "io.modelcontextprotocol/tasks/status")]
    TasksStatus {
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "taskId")]
        task_id: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// Server-side frame builders for a `subscriptions/listen` POST-SSE stream.
// ---------------------------------------------------------------------------

/// FIRST frame on a `subscriptions/listen` stream: the
/// `notifications/subscriptions/acknowledged` notification carrying
/// the subscription id (= the listen request's JSON-RPC id) and the
/// honored-subset `notifications` object. `honored` is the value
/// [`SubscriptionsListenParams::honored_notifications`] returns.
pub fn acknowledged_notification(subscription_id: &str, honored: &Value) -> Value {
    serde_json::json!({
        "jsonrpc": crate::shared::jsonrpc::JSONRPC_VERSION,
        "method": "notifications/subscriptions/acknowledged",
        "params": {
            "subscriptionId": subscription_id,
            "notifications": honored,
            "_meta": {
                META_KEY_SUBSCRIPTION_ID: subscription_id,
            }
        }
    })
}

/// JSON-RPC response envelope for the `subscriptions/listen` request
/// itself, sent right after the acknowledgement so the client's
/// request-correlator can resolve the call. `request_id` is the
/// listen request's JSON-RPC id, echoed verbatim.
pub fn listen_response_envelope(
    request_id: &Value,
    subscription_id: &str,
    honored: &Value,
) -> Value {
    serde_json::json!({
        "jsonrpc": crate::shared::jsonrpc::JSONRPC_VERSION,
        "id": request_id,
        "result": {
            "resultType": "complete",
            "subscriptionId": subscription_id,
            "notifications": honored,
        }
    })
}

/// Graceful terminal frame emitted when the delivery bus closes
/// (server shutdown / session teardown): the
/// `notifications/subscriptions/complete` notification, correlated to
/// the listen request so the client sees an orderly close rather than
/// a bare socket drop.
pub fn complete_notification(subscription_id: &str) -> Value {
    serde_json::json!({
        "jsonrpc": crate::shared::jsonrpc::JSONRPC_VERSION,
        "method": "notifications/subscriptions/complete",
        "params": {
            "resultType": "complete",
            "subscriptionId": subscription_id,
            "_meta": {
                META_KEY_SUBSCRIPTION_ID: subscription_id,
            }
        }
    })
}

/// Match an inbound delivery-bus notification against a client's
/// subscription-target list. Returns `true` if any target accepts the
/// notification.
pub fn subscription_matches(targets: &[SubscriptionTarget], method: &str, payload: &Value) -> bool {
    for target in targets {
        match target {
            SubscriptionTarget::ResourcesUpdated { uri } => {
                if method == "notifications/resources/updated" {
                    let event_uri = payload
                        .get("params")
                        .and_then(|p| p.get("uri"))
                        .and_then(Value::as_str);
                    if event_uri == Some(uri.as_str()) {
                        return true;
                    }
                }
            }
            SubscriptionTarget::ToolsListChanged => {
                if method == "notifications/tools/list_changed"
                    || method == "notifications/tools/listChanged"
                {
                    return true;
                }
            }
            SubscriptionTarget::PromptsListChanged => {
                if method == "notifications/prompts/list_changed"
                    || method == "notifications/prompts/listChanged"
                {
                    return true;
                }
            }
            SubscriptionTarget::ResourcesListChanged => {
                if method == "notifications/resources/list_changed"
                    || method == "notifications/resources/listChanged"
                {
                    return true;
                }
            }
            SubscriptionTarget::TasksStatus { task_id } => {
                // The modern (`2026-07-28`) wire emits the bare SEP-2663
                // `notifications/tasks`; the namespaced + legacy-spelled
                // `notifications/tasks/status` forms are accepted so a
                // subscriber sees task events from either era during the
                // migration window.
                if method == "notifications/tasks"
                    || method == "notifications/io.modelcontextprotocol/tasks/status"
                    || method == "notifications/tasks/status"
                {
                    // Filter by taskId when the client scoped the
                    // subscription. Absent → match every task.
                    let Some(want) = task_id else {
                        return true;
                    };
                    let event_task_id = payload
                        .get("params")
                        .and_then(|p| p.get("taskId"))
                        .and_then(Value::as_str);
                    if event_task_id == Some(want.as_str()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Stamp `params._meta.io.modelcontextprotocol/subscriptionId` on a
/// delivery-bus notification. Mutates in place; creates the `_meta`
/// object when missing.
pub fn inject_subscription_id_meta(payload: &mut Value, subscription_id: &str) {
    let Some(obj) = payload.as_object_mut() else {
        return;
    };
    let params = obj
        .entry("params")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    let Some(params_obj) = params.as_object_mut() else {
        return;
    };
    let meta = params_obj
        .entry("_meta")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Some(meta_obj) = meta.as_object_mut() {
        meta_obj.insert(
            META_KEY_SUBSCRIPTION_ID.to_owned(),
            Value::String(subscription_id.to_owned()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn well_known_constants_match_spec() {
        assert_eq!(METHOD_SUBSCRIPTIONS_LISTEN, "subscriptions/listen");
        assert_eq!(
            META_KEY_SUBSCRIPTION_ID,
            "io.modelcontextprotocol/subscriptionId"
        );
    }

    #[test]
    fn subscription_target_round_trips_all_variants() {
        for variant in [
            json!({ "type": "resources/updated", "uri": "file:///x" }),
            json!({ "type": "tools/listChanged" }),
            json!({ "type": "prompts/listChanged" }),
            json!({ "type": "resources/listChanged" }),
            json!({ "type": "io.modelcontextprotocol/tasks/status" }),
            json!({
                "type": "io.modelcontextprotocol/tasks/status",
                "taskId": "task-42"
            }),
        ] {
            let parsed: SubscriptionTarget = serde_json::from_value(variant.clone()).unwrap();
            let back = serde_json::to_value(&parsed).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn subscription_target_unknown_type_is_rejected() {
        let parsed: Result<SubscriptionTarget, _> =
            serde_json::from_value(json!({ "type": "logging/changed" }));
        assert!(parsed.is_err());
    }

    #[test]
    fn subscriptions_listen_params_full_round_trip() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![
                SubscriptionTarget::ResourcesUpdated {
                    uri: "mcpg://overview".to_owned(),
                },
                SubscriptionTarget::ToolsListChanged,
            ],
            meta: Some(json!({
                "io.modelcontextprotocol/traceparent": "00-x-y-01"
            })),
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(v["subscriptions"][0]["type"], "resources/updated");
        assert_eq!(v["subscriptions"][0]["uri"], "mcpg://overview");
        assert_eq!(v["subscriptions"][1]["type"], "tools/listChanged");
        assert!(v["_meta"]["io.modelcontextprotocol/traceparent"].is_string());
        let back: SubscriptionsListenParams = serde_json::from_value(v).unwrap();
        assert_eq!(back.subscriptions.len(), 2);
    }

    #[test]
    fn honored_notifications_reflects_accepted_targets() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![
                SubscriptionTarget::ToolsListChanged,
                SubscriptionTarget::ResourcesUpdated {
                    uri: "file:///a".to_owned(),
                },
                SubscriptionTarget::ResourcesUpdated {
                    uri: "file:///b".to_owned(),
                },
                // Task-status is governed by the tasks extension, not
                // the list-changed surface — excluded from the object.
                SubscriptionTarget::TasksStatus { task_id: None },
            ],
            meta: None,
        };
        let honored =
            params.honored_notifications(&["file:///a".to_owned(), "file:///b".to_owned()]);
        assert_eq!(honored["toolsListChanged"], true);
        // Targets that weren't requested are absent (not `false`).
        assert!(honored.get("promptsListChanged").is_none());
        assert!(honored.get("resourcesListChanged").is_none());
        let subs = honored["resourceSubscriptions"].as_array().unwrap();
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0], "file:///a");
        assert_eq!(subs[1], "file:///b");
    }

    /// A requested resource the gateway could not register (unknown URI,
    /// per-session limit, store failure) must not appear in the ack — the
    /// client would wait forever for an event that is never produced.
    #[test]
    fn honored_notifications_omits_resources_that_were_not_established() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![
                SubscriptionTarget::ResourcesUpdated {
                    uri: "file:///known".to_owned(),
                },
                SubscriptionTarget::ResourcesUpdated {
                    uri: "file:///unknown".to_owned(),
                },
            ],
            meta: None,
        };
        let honored = params.honored_notifications(&["file:///known".to_owned()]);
        let subs = honored["resourceSubscriptions"].as_array().unwrap();
        assert_eq!(subs, &[json!("file:///known")]);
    }

    /// Every requested resource being rejected drops the key entirely rather
    /// than acking an empty array, so "nothing was honored" reads the same as
    /// it does for the list-changed flags.
    #[test]
    fn honored_notifications_drops_the_key_when_nothing_was_established() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![SubscriptionTarget::ResourcesUpdated {
                uri: "file:///unknown".to_owned(),
            }],
            meta: None,
        };
        assert_eq!(params.honored_notifications(&[]), json!({}));
    }

    #[test]
    fn honored_notifications_empty_when_no_targets() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![],
            meta: None,
        };
        assert_eq!(params.honored_notifications(&[]), json!({}));
    }

    #[test]
    fn subscriptions_listen_params_default_meta_omitted() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![SubscriptionTarget::ToolsListChanged],
            meta: None,
        };
        let v = serde_json::to_value(&params).unwrap();
        assert!(v.get("_meta").is_none());
    }

    // -- Byte-equivalence guards ------------------------------------------
    //
    // The modern HTTP transport builds these three frames by calling the
    // constructors above instead of inlining `json!`. Each guard pins the
    // constructor output against the exact literal the transport used to
    // hand-build, so the wire cannot drift.

    #[test]
    fn acknowledged_notification_matches_inline_wire() {
        for (subscription_id, honored) in [
            ("req-7", json!({ "toolsListChanged": true })),
            ("", json!({})),
        ] {
            let inline = json!({
                "jsonrpc": "2.0",
                "method": "notifications/subscriptions/acknowledged",
                "params": {
                    "subscriptionId": subscription_id,
                    "notifications": honored,
                    "_meta": {
                        "io.modelcontextprotocol/subscriptionId": subscription_id,
                    }
                }
            });
            assert_eq!(acknowledged_notification(subscription_id, &honored), inline);
        }
    }

    #[test]
    fn listen_response_envelope_matches_inline_wire() {
        for request_id in [json!(7), json!("req-7"), Value::Null] {
            let subscription_id = "req-7";
            let honored = json!({ "resourceSubscriptions": ["file:///a"] });
            let inline = json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "result": {
                    "resultType": "complete",
                    "subscriptionId": subscription_id,
                    "notifications": honored,
                }
            });
            assert_eq!(
                listen_response_envelope(&request_id, subscription_id, &honored),
                inline
            );
        }
    }

    #[test]
    fn complete_notification_matches_inline_wire() {
        let subscription_id = "req-7";
        let inline = json!({
            "jsonrpc": "2.0",
            "method": "notifications/subscriptions/complete",
            "params": {
                "resultType": "complete",
                "subscriptionId": subscription_id,
                "_meta": {
                    "io.modelcontextprotocol/subscriptionId": subscription_id,
                }
            }
        });
        assert_eq!(complete_notification(subscription_id), inline);
    }

    #[test]
    fn inject_subscription_id_meta_matches_inline_wire() {
        // The live path forwards the delivery-bus payload with the
        // subscription id stamped into `params._meta`; nothing else on
        // the frame changes.
        let mut payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": { "uri": "file:///x" }
        });
        inject_subscription_id_meta(&mut payload, "sub-1");
        assert_eq!(
            payload,
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/resources/updated",
                "params": {
                    "uri": "file:///x",
                    "_meta": {
                        "io.modelcontextprotocol/subscriptionId": "sub-1",
                    }
                }
            })
        );
    }

    #[test]
    fn subscription_matches_accepts_and_rejects_targets() {
        let targets = vec![
            SubscriptionTarget::ResourcesUpdated {
                uri: "file:///x".to_owned(),
            },
            SubscriptionTarget::ToolsListChanged,
        ];
        let updated = json!({ "params": { "uri": "file:///x" } });
        assert!(subscription_matches(
            &targets,
            "notifications/resources/updated",
            &updated
        ));
        let other_uri = json!({ "params": { "uri": "file:///y" } });
        assert!(!subscription_matches(
            &targets,
            "notifications/resources/updated",
            &other_uri
        ));
        assert!(subscription_matches(
            &targets,
            "notifications/tools/listChanged",
            &json!({})
        ));
        assert!(!subscription_matches(
            &targets,
            "notifications/prompts/listChanged",
            &json!({})
        ));
    }
}
