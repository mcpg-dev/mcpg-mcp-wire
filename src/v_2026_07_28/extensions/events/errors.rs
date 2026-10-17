//! Error codes of the MCP Events extension and their typed `data`.
//!
//! The constants below are the codes as the gateway reasons about them.
//! The wire has two numberings ([`CodeNumbering`]): `-32011` to `-32015`,
//! which the ChatGPT profile documents, and `-32023` to `-32027`, which
//! the SEP-3415 sketch uses. A server renumbers at its output; a reader
//! accepts both.
//!
//! Each code carries a family of conditions; the `data` object tells them
//! apart (`{kind}`, `{reason}`, `{limit, max?}`, `{feature, value?,
//! reason?}`, `{reason}`).

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::shared::error::{INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE};
use crate::shared::jsonrpc::{JSONRPC_VERSION, JsonRpcError, JsonRpcErrorBody};
use crate::v_2026_07_28::extensions::events::wire::DeliveryErrorCategory;

/// An unknown event name, or no subscription for the key on unsubscribe.
pub const NOT_FOUND_CODE: i32 = -32011;
/// The principal may not use this event with these arguments, its access
/// was revoked, or it is not authenticated at all.
pub const FORBIDDEN_CODE: i32 = -32012;
/// A server limit or quota was reached.
pub const RESOURCE_EXHAUSTED_CODE: i32 = -32013;
/// Well-formed, but asks for something this server does not offer.
pub const UNSUPPORTED_CODE: i32 = -32014;
/// The callback endpoint failed verification or could not be reached.
pub const CALLBACK_ENDPOINT_ERROR_CODE: i32 = -32015;

/// How far the SEP-3415 numbering moves each code.
const SEP_3415_SHIFT: i32 = 12;

/// How the extension's five codes are numbered on the wire.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeNumbering {
    /// `-32011` to `-32015`: the numbers the ChatGPT profile documents.
    #[default]
    Draft,
    /// `-32023` to `-32027`: the numbers the SEP-3415 sketch uses.
    #[serde(rename = "sep_3415")]
    Sep3415,
}

impl CodeNumbering {
    /// `code` as this numbering writes it. Any other code is unchanged.
    pub fn wire_code(self, code: i32) -> i32 {
        match self {
            Self::Sep3415 if (CALLBACK_ENDPOINT_ERROR_CODE..=NOT_FOUND_CODE).contains(&code) => {
                code - SEP_3415_SHIFT
            }
            _ => code,
        }
    }

    /// The code a reader means by `code` under either numbering.
    pub fn normalize(code: i32) -> i32 {
        let sep_3415 =
            (CALLBACK_ENDPOINT_ERROR_CODE - SEP_3415_SHIFT)..=(NOT_FOUND_CODE - SEP_3415_SHIFT);
        if sep_3415.contains(&code) {
            code + SEP_3415_SHIFT
        } else {
            code
        }
    }
}

/// `data.feature` of an unsupported delivery mode.
pub const FEATURE_DELIVERY_MODE: &str = "deliveryMode";
/// `data.feature` of a changed `inputSchema`.
pub const FEATURE_INPUT_SCHEMA: &str = "inputSchema";
/// `data.feature` of a changed `payloadSchema`.
pub const FEATURE_PAYLOAD_SCHEMA: &str = "payloadSchema";
/// `data.reason` of an in-place, non-additive schema change.
pub const REASON_SCHEMA_CHANGED: &str = "schema_changed";

/// What a `-32011` did not find.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotFoundKind {
    Event,
    Subscription,
    #[serde(other)]
    Unknown,
}

/// `data` of `-32011`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotFoundData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<NotFoundKind>,
}

/// `data.reason` of `-32012`: why the principal may not have the events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForbiddenReason {
    /// No authenticated principal, or one below the mode's trust floor.
    Unauthenticated,
    /// The credential the subscription stands on expired.
    Expired,
    /// The principal's credential was revoked, or the subscription ended
    /// for good.
    Revoked,
    /// An operator ended the subscription.
    Admin,
    /// The trust floor refuses the principal.
    Trust,
    /// A policy rule refuses the principal.
    Policy,
    /// A policy engine refuses the principal.
    PolicyEngine,
    /// The request needs an approval first.
    ApprovalRequired,
    /// The token lacks a scope the request needs.
    InsufficientScope,
    /// Another gate refuses the request.
    Gate,
    /// The licence does not cover the delivery mode.
    Licence,
    /// The upstream server of an imported type refuses the principal.
    UpstreamForbidden,
    /// A reason this reader does not know.
    #[serde(other)]
    Unknown,
}

impl ForbiddenReason {
    /// The wire string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unauthenticated => "unauthenticated",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
            Self::Admin => "admin",
            Self::Trust => "trust",
            Self::Policy => "policy",
            Self::PolicyEngine => "policy_engine",
            Self::ApprovalRequired => "approval_required",
            Self::InsufficientScope => "insufficient_scope",
            Self::Gate => "gate",
            Self::Licence => "licence",
            Self::UpstreamForbidden => "upstream_forbidden",
            Self::Unknown => "unknown",
        }
    }
}

/// `data` of `-32012`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForbiddenData {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ForbiddenReason>,
}

/// `data` of `-32013`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceExhaustedData {
    /// The limit that was reached, e.g. `"subscriptions"`.
    pub limit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
}

/// `data` of `-32014`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnsupportedData {
    pub feature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// `data` of `-32015`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallbackEndpointErrorData {
    pub reason: DeliveryErrorCategory,
}

/// The typed `data` of an events error, recovered from a JSON-RPC error.
#[derive(Debug, Clone, PartialEq)]
pub enum EventsErrorData {
    NotFound(NotFoundData),
    Forbidden(ForbiddenData),
    ResourceExhausted(ResourceExhaustedData),
    Unsupported(UnsupportedData),
    CallbackEndpoint(CallbackEndpointErrorData),
}

impl EventsErrorData {
    /// Parse `data` by `code`, in either numbering. `None` for a code
    /// outside the events family, or `data` that does not fit the code's
    /// shape. A not-found or forbidden code without `data` parses as the
    /// empty shape (its fields are optional).
    pub fn parse(code: i32, data: Option<&Value>) -> Option<Self> {
        let empty = Value::Object(Map::new());
        let data = data.unwrap_or(&empty).clone();
        match CodeNumbering::normalize(code) {
            NOT_FOUND_CODE => serde_json::from_value(data).ok().map(Self::NotFound),
            FORBIDDEN_CODE => serde_json::from_value(data).ok().map(Self::Forbidden),
            RESOURCE_EXHAUSTED_CODE => serde_json::from_value(data)
                .ok()
                .map(Self::ResourceExhausted),
            UNSUPPORTED_CODE => serde_json::from_value(data).ok().map(Self::Unsupported),
            CALLBACK_ENDPOINT_ERROR_CODE => serde_json::from_value(data)
                .ok()
                .map(Self::CallbackEndpoint),
            _ => None,
        }
    }
}

/// An `events/*` failure: a JSON-RPC code, a message and optional typed
/// `data`. The constructors are the only way to build one, so each code
/// always carries the `data` shape the extension defines for it.
#[derive(Debug, Clone, PartialEq)]
pub struct EventsError {
    code: i32,
    message: String,
    data: Option<Value>,
}

fn data_value<T: Serialize>(data: &T) -> Option<Value> {
    serde_json::to_value(data).ok()
}

impl EventsError {
    /// `-32602`: arguments fail `inputSchema`, a malformed or non-`https`
    /// callback URL, a bad `whsec_` secret. `message` must not quote the
    /// rejected value.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: INVALID_PARAMS_CODE,
            message: message.into(),
            data: None,
        }
    }

    /// `-32011` with `data.kind`.
    pub fn not_found(kind: NotFoundKind) -> Self {
        let message = match kind {
            NotFoundKind::Event => "event type not found",
            NotFoundKind::Subscription => "subscription not found",
            NotFoundKind::Unknown => "not found",
        };
        Self {
            code: NOT_FOUND_CODE,
            message: message.to_owned(),
            data: data_value(&NotFoundData { kind: Some(kind) }),
        }
    }

    /// `-32012 {reason}`.
    pub fn forbidden(reason: ForbiddenReason) -> Self {
        Self {
            code: FORBIDDEN_CODE,
            message: "forbidden".to_owned(),
            data: data_value(&ForbiddenData {
                reason: Some(reason),
            }),
        }
    }

    /// `-32013` with `data.limit` and, when known, `data.max`.
    pub fn resource_exhausted(limit: &str, max: Option<u64>) -> Self {
        Self {
            code: RESOURCE_EXHAUSTED_CODE,
            message: format!("limit reached: {limit}"),
            data: data_value(&ResourceExhaustedData {
                limit: limit.to_owned(),
                max,
            }),
        }
    }

    /// `-32014` for a feature this server does not offer.
    pub fn unsupported(feature: &str, value: Option<Value>) -> Self {
        Self {
            code: UNSUPPORTED_CODE,
            message: format!("unsupported: {feature}"),
            data: data_value(&UnsupportedData {
                feature: feature.to_owned(),
                value,
                reason: None,
            }),
        }
    }

    /// `-32014 {feature: "deliveryMode", value: mode}`.
    pub fn unsupported_delivery_mode(mode: &str) -> Self {
        Self::unsupported(FEATURE_DELIVERY_MODE, Some(Value::String(mode.to_owned())))
    }

    /// `-32014 {feature, reason: "schema_changed"}`: the type's schema
    /// changed in place; the client re-lists and resubscribes.
    pub fn schema_changed(feature: &str) -> Self {
        Self {
            code: UNSUPPORTED_CODE,
            message: format!("{feature} changed"),
            data: data_value(&UnsupportedData {
                feature: feature.to_owned(),
                value: None,
                reason: Some(REASON_SCHEMA_CHANGED.to_owned()),
            }),
        }
    }

    /// `-32015 {reason}`. `reason` is one of the six endpoint categories;
    /// [`DeliveryErrorCategory::Paused`] describes the server, not the
    /// endpoint, and does not belong here.
    pub fn callback_endpoint(reason: DeliveryErrorCategory) -> Self {
        Self {
            code: CALLBACK_ENDPOINT_ERROR_CODE,
            message: format!("callback endpoint error: {}", reason.as_str()),
            data: data_value(&CallbackEndpointErrorData { reason }),
        }
    }

    /// `-32603`: a store or coordinator fault. Retryable; the request left
    /// no state behind.
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: INTERNAL_ERROR_CODE,
            message: message.into(),
            data: None,
        }
    }

    /// Add one entry to `data`, creating the object when absent. For a
    /// transport marker such as a missing-scope list; the marker must not
    /// collide with the code's own `data` fields.
    pub fn with_data_entry(mut self, key: &str, value: Value) -> Self {
        let mut object = match self.data.take() {
            Some(Value::Object(object)) => object,
            _ => Map::new(),
        };
        object.insert(key.to_owned(), value);
        self.data = Some(Value::Object(object));
        self
    }

    pub fn code(&self) -> i32 {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn data(&self) -> Option<&Value> {
        self.data.as_ref()
    }

    /// The JSON-RPC `error` object.
    pub fn to_error_body(&self) -> JsonRpcErrorBody {
        JsonRpcErrorBody {
            code: self.code,
            message: self.message.clone(),
            data: self.data.clone(),
        }
    }

    /// This error with its code as `numbering` writes it.
    pub fn numbered(mut self, numbering: CodeNumbering) -> Self {
        self.code = numbering.wire_code(self.code);
        self
    }

    /// The JSON-RPC error envelope answering request `id`.
    pub fn into_jsonrpc_error(self, id: Option<Value>) -> JsonRpcError {
        JsonRpcError {
            jsonrpc: JSONRPC_VERSION,
            id,
            error: JsonRpcErrorBody {
                code: self.code,
                message: self.message,
                data: self.data,
            },
        }
    }
}

impl std::fmt::Display for EventsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code {})", self.message, self.code)
    }
}

impl std::error::Error for EventsError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn body(error: &EventsError) -> Value {
        serde_json::to_value(error.to_error_body()).unwrap()
    }

    /// The `data` a reader recovers from the error on the wire.
    fn typed(error: &EventsError) -> Option<EventsErrorData> {
        EventsErrorData::parse(error.code(), error.data())
    }

    #[test]
    fn codes_are_the_sketch_numbers() {
        assert_eq!(NOT_FOUND_CODE, -32011);
        assert_eq!(FORBIDDEN_CODE, -32012);
        assert_eq!(RESOURCE_EXHAUSTED_CODE, -32013);
        assert_eq!(UNSUPPORTED_CODE, -32014);
        assert_eq!(CALLBACK_ENDPOINT_ERROR_CODE, -32015);
    }

    #[test]
    fn not_found_names_its_kind() {
        let event = EventsError::not_found(NotFoundKind::Event);
        assert_eq!(event.code(), -32011);
        assert_eq!(body(&event)["data"], json!({ "kind": "event" }));
        let subscription = EventsError::not_found(NotFoundKind::Subscription);
        assert_eq!(
            body(&subscription)["data"],
            json!({ "kind": "subscription" })
        );
        assert_eq!(
            typed(&subscription),
            Some(EventsErrorData::NotFound(NotFoundData {
                kind: Some(NotFoundKind::Subscription)
            }))
        );
    }

    #[test]
    fn forbidden_names_its_reason() {
        let revoked = EventsError::forbidden(ForbiddenReason::Revoked);
        assert_eq!(revoked.code(), -32012);
        assert_eq!(body(&revoked)["data"], json!({ "reason": "revoked" }));
        let policy_engine = EventsError::forbidden(ForbiddenReason::PolicyEngine);
        assert_eq!(
            body(&policy_engine)["data"],
            json!({ "reason": "policy_engine" })
        );
        assert_eq!(
            EventsErrorData::parse(-32012, None),
            Some(EventsErrorData::Forbidden(ForbiddenData { reason: None }))
        );
    }

    #[test]
    fn forbidden_reasons_read_their_wire_strings_and_tolerate_others() {
        for reason in [
            ForbiddenReason::Unauthenticated,
            ForbiddenReason::Expired,
            ForbiddenReason::Revoked,
            ForbiddenReason::Admin,
            ForbiddenReason::Trust,
            ForbiddenReason::Policy,
            ForbiddenReason::PolicyEngine,
            ForbiddenReason::ApprovalRequired,
            ForbiddenReason::InsufficientScope,
            ForbiddenReason::Gate,
            ForbiddenReason::Licence,
            ForbiddenReason::UpstreamForbidden,
        ] {
            assert_eq!(serde_json::to_value(reason).unwrap(), reason.as_str());
            assert_eq!(
                serde_json::from_value::<ForbiddenReason>(json!(reason.as_str())).unwrap(),
                reason
            );
        }
        assert_eq!(
            EventsErrorData::parse(-32012, Some(&json!({ "reason": "quarantined" }))),
            Some(EventsErrorData::Forbidden(ForbiddenData {
                reason: Some(ForbiddenReason::Unknown)
            }))
        );
    }

    #[test]
    fn resource_exhausted_names_the_limit() {
        let error = EventsError::resource_exhausted("subscriptions", Some(100));
        assert_eq!(error.code(), -32013);
        assert_eq!(
            body(&error)["data"],
            json!({ "limit": "subscriptions", "max": 100 })
        );
        let no_max = EventsError::resource_exhausted("callback_verifications", None);
        assert_eq!(
            body(&no_max)["data"],
            json!({ "limit": "callback_verifications" })
        );
    }

    #[test]
    fn unsupported_shapes() {
        let mode = EventsError::unsupported_delivery_mode("push");
        assert_eq!(mode.code(), -32014);
        assert_eq!(
            body(&mode)["data"],
            json!({ "feature": "deliveryMode", "value": "push" })
        );
        let schema = EventsError::schema_changed(FEATURE_PAYLOAD_SCHEMA);
        assert_eq!(
            body(&schema)["data"],
            json!({ "feature": "payloadSchema", "reason": "schema_changed" })
        );
    }

    #[test]
    fn callback_endpoint_error_carries_the_category() {
        let error = EventsError::callback_endpoint(DeliveryErrorCategory::ChallengeFailed);
        assert_eq!(error.code(), -32015);
        assert_eq!(
            body(&error)["data"],
            json!({ "reason": "challenge_failed" })
        );
        assert_eq!(
            typed(&error),
            Some(EventsErrorData::CallbackEndpoint(
                CallbackEndpointErrorData {
                    reason: DeliveryErrorCategory::ChallengeFailed
                }
            ))
        );
    }

    #[test]
    fn invalid_params_and_internal_use_the_base_codes() {
        assert_eq!(EventsError::invalid_params("bad").code(), -32602);
        assert_eq!(EventsError::internal("store down").code(), -32603);
        assert_eq!(typed(&EventsError::invalid_params("bad")), None);
    }

    #[test]
    fn with_data_entry_extends_the_typed_data() {
        let error = EventsError::forbidden(ForbiddenReason::InsufficientScope)
            .with_data_entry("marker", json!("a b"));
        assert_eq!(
            body(&error)["data"],
            json!({ "reason": "insufficient_scope", "marker": "a b" })
        );
        let from_none = EventsError::internal("x").with_data_entry("marker", json!(1));
        assert_eq!(body(&from_none)["data"], json!({ "marker": 1 }));
    }

    #[test]
    fn jsonrpc_envelope_carries_the_error_body() {
        let error = EventsError::forbidden(ForbiddenReason::Revoked);
        let error_body = body(&error);
        let envelope = serde_json::to_value(error.into_jsonrpc_error(Some(json!(7)))).unwrap();
        assert_eq!(envelope["jsonrpc"], "2.0");
        assert_eq!(envelope["id"], 7);
        assert_eq!(envelope["error"], error_body);
    }

    #[test]
    fn parse_tolerates_foreign_codes_and_shapes() {
        assert_eq!(EventsErrorData::parse(-32601, None), None);
        assert_eq!(EventsErrorData::parse(-32013, Some(&json!({}))), None);
        assert_eq!(
            EventsErrorData::parse(-32011, Some(&json!({ "kind": "galaxy" }))),
            Some(EventsErrorData::NotFound(NotFoundData {
                kind: Some(NotFoundKind::Unknown)
            }))
        );
    }

    #[test]
    fn each_numbering_writes_its_own_codes_and_a_reader_takes_both() {
        let draft = [-32011, -32012, -32013, -32014, -32015];
        let sep_3415 = [-32023, -32024, -32025, -32026, -32027];
        for (old, new) in draft.into_iter().zip(sep_3415) {
            assert_eq!(CodeNumbering::Draft.wire_code(old), old);
            assert_eq!(CodeNumbering::Sep3415.wire_code(old), new);
            assert_eq!(CodeNumbering::normalize(new), old);
            assert_eq!(CodeNumbering::normalize(old), old);
        }
        for other in [
            -32602, -32603, -32601, -32022, -32029, -32010, -32016, -32028,
        ] {
            assert_eq!(CodeNumbering::Sep3415.wire_code(other), other);
            assert_eq!(CodeNumbering::normalize(other), other);
        }
        let renumbered =
            EventsError::resource_exhausted("streams", Some(16)).numbered(CodeNumbering::Sep3415);
        assert_eq!(renumbered.code(), -32025);
        assert_eq!(
            typed(&renumbered),
            Some(EventsErrorData::ResourceExhausted(ResourceExhaustedData {
                limit: "streams".to_owned(),
                max: Some(16),
            }))
        );
        assert_eq!(
            serde_json::to_value(CodeNumbering::Sep3415).unwrap(),
            json!("sep_3415")
        );
    }
}
