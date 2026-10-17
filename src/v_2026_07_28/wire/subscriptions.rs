//! `subscriptions/listen` wire types (2026-07-28,
//! `basic/patterns/subscriptions`; SEP-2575, SEP-2663 for task ids).
//!
//! A client POSTs a `subscriptions/listen` request naming the
//! notifications it wants; the server answers with an SSE stream that
//! stays open until the client closes it or the server ends the
//! subscription. The subscription's id is the request's JSON-RPC `id`,
//! as sent: every message on the stream carries it under
//! `params._meta` (or `result._meta`).
//!
//! ```text
//! POST /mcp                    Mcp-Protocol-Version: 2026-07-28
//!
//! { "jsonrpc": "2.0", "id": 7, "method": "subscriptions/listen",
//!   "params": { "notifications": {
//!     "toolsListChanged": true,
//!     "resourceSubscriptions": ["file:///x"] } } }
//!
//! ── HTTP/1.1 200 OK · Content-Type: text/event-stream ──
//!
//! data: { "jsonrpc": "2.0", "method": "notifications/subscriptions/acknowledged",
//!         "params": { "notifications": { "toolsListChanged": true,
//!                                        "resourceSubscriptions": ["file:///x"] },
//!                     "_meta": { "io.modelcontextprotocol/subscriptionId": 7 } } }
//!
//! data: { "jsonrpc": "2.0", "method": "notifications/resources/updated",
//!         "params": { "uri": "file:///x",
//!                     "_meta": { "io.modelcontextprotocol/subscriptionId": 7 } } }
//!
//! … and, only when the server ends the subscription itself:
//!
//! data: { "jsonrpc": "2.0", "id": 7,
//!         "result": { "resultType": "complete",
//!                     "_meta": { "io.modelcontextprotocol/subscriptionId": 7 } } }
//! ```
//!
//! The `notifications` filter has the specification's fields plus
//! `eventsListChanged`, for the MCP Events extension's
//! `notifications/events/list_changed`. A server also reads the typed
//! list `{"subscriptions": [{"type": …}]}`; a client writes the filter.

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

/// Parameters for `subscriptions/listen`: what the client wants to
/// receive, as [`SubscriptionTarget`]s.
///
/// It serializes as the specification's `notifications` filter
/// (`toolsListChanged`, `promptsListChanged`, `resourcesListChanged`,
/// `resourceSubscriptions`, `taskIds`, and `eventsListChanged`). It
/// deserializes from that filter, from the typed list
/// `{"subscriptions": [{"type": …}]}`, or from both at once. A task-status
/// target without a task id (every task of the caller) exists only in
/// the typed list, so it cannot be serialized.
#[derive(Debug, Clone)]
pub struct SubscriptionsListenParams {
    /// What to receive, in the order asked.
    pub subscriptions: Vec<SubscriptionTarget>,
    /// The request's `_meta`.
    pub meta: Option<Value>,
}

impl Serialize for SubscriptionsListenParams {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{Error as _, SerializeMap as _};
        let mut filter = serde_json::Map::new();
        let mut resources = Vec::new();
        let mut tasks = Vec::new();
        for target in &self.subscriptions {
            match target {
                SubscriptionTarget::ToolsListChanged => {
                    filter.insert("toolsListChanged".to_owned(), Value::Bool(true));
                }
                SubscriptionTarget::PromptsListChanged => {
                    filter.insert("promptsListChanged".to_owned(), Value::Bool(true));
                }
                SubscriptionTarget::ResourcesListChanged => {
                    filter.insert("resourcesListChanged".to_owned(), Value::Bool(true));
                }
                SubscriptionTarget::EventsListChanged => {
                    filter.insert("eventsListChanged".to_owned(), Value::Bool(true));
                }
                SubscriptionTarget::ResourcesUpdated { uri } => {
                    resources.push(Value::String(uri.clone()));
                }
                SubscriptionTarget::TasksStatus {
                    task_id: Some(task_id),
                } => tasks.push(Value::String(task_id.clone())),
                SubscriptionTarget::TasksStatus { task_id: None } => {
                    return Err(S::Error::custom(
                        "a task-status subscription needs a task id on the 2026-07-28 wire",
                    ));
                }
            }
        }
        if !resources.is_empty() {
            filter.insert("resourceSubscriptions".to_owned(), Value::Array(resources));
        }
        if !tasks.is_empty() {
            filter.insert("taskIds".to_owned(), Value::Array(tasks));
        }
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("notifications", &filter)?;
        if let Some(meta) = &self.meta {
            map.serialize_entry("_meta", meta)?;
        }
        map.end()
    }
}

impl SubscriptionsListenParams {
    /// Build the `notifications` object the server reflects back in
    /// the acknowledgement frame — the honored subset of what the
    /// client asked for. Only the list-changed flags the server
    /// actually accepted are present (`true`); `resourceSubscriptions`
    /// lists the resource URIs for which per-resource `updated`
    /// delivery was established, and `taskIds` the tasks whose status
    /// the stream carries. An empty object means nothing was honored.
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
        let mut events = false;
        let mut task_ids: Vec<Value> = Vec::new();
        for target in &self.subscriptions {
            match target {
                SubscriptionTarget::ToolsListChanged => tools = true,
                SubscriptionTarget::PromptsListChanged => prompts = true,
                SubscriptionTarget::ResourcesListChanged => resources = true,
                SubscriptionTarget::EventsListChanged => events = true,
                // Reflected from `established_resources` instead, so a target
                // the gateway skipped is not acked.
                SubscriptionTarget::ResourcesUpdated { .. } => {}
                SubscriptionTarget::TasksStatus {
                    task_id: Some(task_id),
                } => task_ids.push(Value::String(task_id.clone())),
                // Every task of the caller: the typed list's own form, which
                // the specification's filter has no field for.
                SubscriptionTarget::TasksStatus { task_id: None } => {}
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
        if events {
            obj.insert("eventsListChanged".to_owned(), Value::Bool(true));
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
        if !task_ids.is_empty() {
            obj.insert("taskIds".to_owned(), Value::Array(task_ids));
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

        // The typed list lands in `subscriptions`; the specification's
        // filter lands in `notifications` and becomes targets after it.
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Notifications {
            #[serde(default)]
            tools_list_changed: bool,
            #[serde(default)]
            prompts_list_changed: bool,
            #[serde(default)]
            resources_list_changed: bool,
            #[serde(default)]
            events_list_changed: bool,
            #[serde(default)]
            resource_subscriptions: Vec<String>,
            #[serde(default)]
            task_ids: Vec<String>,
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
            if n.tools_list_changed {
                targets.push(SubscriptionTarget::ToolsListChanged);
            }
            if n.prompts_list_changed {
                targets.push(SubscriptionTarget::PromptsListChanged);
            }
            if n.resources_list_changed {
                targets.push(SubscriptionTarget::ResourcesListChanged);
            }
            if n.events_list_changed {
                targets.push(SubscriptionTarget::EventsListChanged);
            }
            targets.extend(
                n.resource_subscriptions
                    .into_iter()
                    .map(|uri| SubscriptionTarget::ResourcesUpdated { uri }),
            );
            targets.extend(
                n.task_ids
                    .into_iter()
                    .map(|task_id| SubscriptionTarget::TasksStatus {
                        task_id: Some(task_id),
                    }),
            );
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
    /// MCP Events type-catalog change events
    /// (`notifications/events/list_changed`).
    #[serde(rename = "events/listChanged")]
    EventsListChanged,
}

// ---------------------------------------------------------------------------
// Server-side frame builders for a `subscriptions/listen` POST-SSE stream.
// ---------------------------------------------------------------------------

/// `_meta` naming subscription `subscription_id`.
fn subscription_meta(subscription_id: &Value) -> Value {
    let mut meta = serde_json::Map::new();
    meta.insert(META_KEY_SUBSCRIPTION_ID.to_owned(), subscription_id.clone());
    Value::Object(meta)
}

/// The first frame of a `subscriptions/listen` stream:
/// `notifications/subscriptions/acknowledged` with the honored filter
/// ([`SubscriptionsListenParams::honored_notifications`]).
/// `subscription_id` is the listen request's JSON-RPC id, as sent.
pub fn acknowledged_notification(subscription_id: &Value, honored: &Value) -> Value {
    serde_json::json!({
        "jsonrpc": crate::shared::jsonrpc::JSONRPC_VERSION,
        "method": "notifications/subscriptions/acknowledged",
        "params": {
            "notifications": honored,
            "_meta": subscription_meta(subscription_id),
        }
    })
}

/// The last frame of a long-lived stream the server ends itself
/// (`subscriptions/listen`, or an extension's, such as `events/stream`):
/// the result of request `id`, which is also the subscription id. A
/// stream the client closes ends without one.
pub fn closed_stream_result(id: &Value) -> Value {
    serde_json::json!({
        "jsonrpc": crate::shared::jsonrpc::JSONRPC_VERSION,
        "id": id,
        "result": {
            "resultType": "complete",
            "_meta": subscription_meta(id),
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
            SubscriptionTarget::EventsListChanged => {
                if method
                    == crate::v_2026_07_28::extensions::events::wire::METHOD_NOTIFICATIONS_LIST_CHANGED
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Stamp `params._meta.io.modelcontextprotocol/subscriptionId` on a
/// delivery-bus notification. Mutates in place; creates the `_meta`
/// object when missing.
pub fn inject_subscription_id_meta(payload: &mut Value, subscription_id: &Value) {
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
        meta_obj.insert(META_KEY_SUBSCRIPTION_ID.to_owned(), subscription_id.clone());
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
            json!({ "type": "events/listChanged" }),
        ] {
            let parsed: SubscriptionTarget = serde_json::from_value(variant.clone()).unwrap();
            let back = serde_json::to_value(&parsed).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn events_list_changed_is_honored_and_matched() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![SubscriptionTarget::EventsListChanged],
            meta: None,
        };
        assert_eq!(
            params.honored_notifications(&[]),
            json!({ "eventsListChanged": true })
        );
        let targets = [SubscriptionTarget::EventsListChanged];
        let notification =
            json!({ "jsonrpc": "2.0", "method": "notifications/events/list_changed" });
        assert!(subscription_matches(
            &targets,
            "notifications/events/list_changed",
            &notification
        ));
        assert!(!subscription_matches(
            &targets,
            "notifications/tools/list_changed",
            &notification
        ));
    }

    #[test]
    fn subscription_target_unknown_type_is_rejected() {
        let parsed: Result<SubscriptionTarget, _> =
            serde_json::from_value(json!({ "type": "logging/changed" }));
        assert!(parsed.is_err());
    }

    #[test]
    fn listen_params_serialize_as_the_specification_filter_and_round_trip() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![
                SubscriptionTarget::ResourcesUpdated {
                    uri: "mcpg://overview".to_owned(),
                },
                SubscriptionTarget::ToolsListChanged,
                SubscriptionTarget::TasksStatus {
                    task_id: Some("task-42".to_owned()),
                },
                SubscriptionTarget::EventsListChanged,
            ],
            meta: Some(json!({
                "io.modelcontextprotocol/traceparent": "00-x-y-01"
            })),
        };
        let v = serde_json::to_value(&params).unwrap();
        assert_eq!(
            v,
            json!({
                "notifications": {
                    "toolsListChanged": true,
                    "eventsListChanged": true,
                    "resourceSubscriptions": ["mcpg://overview"],
                    "taskIds": ["task-42"]
                },
                "_meta": { "io.modelcontextprotocol/traceparent": "00-x-y-01" }
            })
        );
        let back: SubscriptionsListenParams = serde_json::from_value(v).unwrap();
        assert_eq!(back.subscriptions.len(), 4);
        for target in &params.subscriptions {
            assert!(back.subscriptions.contains(target), "{target:?}");
        }
    }

    #[test]
    fn a_task_status_target_without_an_id_has_no_specification_form() {
        let params = SubscriptionsListenParams {
            subscriptions: vec![SubscriptionTarget::TasksStatus { task_id: None }],
            meta: None,
        };
        assert!(serde_json::to_value(&params).is_err());
    }

    #[test]
    fn the_typed_list_is_still_read() {
        let params: SubscriptionsListenParams = serde_json::from_value(json!({
            "subscriptions": [
                { "type": "tools/listChanged" },
                { "type": "io.modelcontextprotocol/tasks/status" }
            ],
            "notifications": { "resourceSubscriptions": ["file:///a"] }
        }))
        .unwrap();
        assert_eq!(
            params.subscriptions,
            vec![
                SubscriptionTarget::ToolsListChanged,
                SubscriptionTarget::TasksStatus { task_id: None },
                SubscriptionTarget::ResourcesUpdated {
                    uri: "file:///a".to_owned()
                },
            ]
        );
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
                // Every task of the caller has no field in the filter.
                SubscriptionTarget::TasksStatus { task_id: None },
                SubscriptionTarget::TasksStatus {
                    task_id: Some("task-9".to_owned()),
                },
            ],
            meta: None,
        };
        let honored =
            params.honored_notifications(&["file:///a".to_owned(), "file:///b".to_owned()]);
        assert_eq!(honored["toolsListChanged"], true);
        assert_eq!(honored["taskIds"], json!(["task-9"]));
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

    // -- The frames, as the specification shows them --------------------

    #[test]
    fn the_acknowledgement_carries_the_request_id_as_sent() {
        for id in [json!(1), json!("req-7")] {
            assert_eq!(
                acknowledged_notification(&id, &json!({ "toolsListChanged": true })),
                json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/subscriptions/acknowledged",
                    "params": {
                        "notifications": { "toolsListChanged": true },
                        "_meta": { "io.modelcontextprotocol/subscriptionId": id }
                    }
                })
            );
        }
    }

    #[test]
    fn a_closed_stream_answers_its_request() {
        for id in [json!(1), json!("req-7")] {
            assert_eq!(
                closed_stream_result(&id),
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "resultType": "complete",
                        "_meta": { "io.modelcontextprotocol/subscriptionId": id }
                    }
                })
            );
        }
    }

    #[test]
    fn a_delivered_notification_is_stamped_with_the_request_id() {
        let mut payload = json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": { "uri": "file:///x" }
        });
        inject_subscription_id_meta(&mut payload, &json!(1));
        assert_eq!(
            payload,
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/resources/updated",
                "params": {
                    "uri": "file:///x",
                    "_meta": { "io.modelcontextprotocol/subscriptionId": 1 }
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
