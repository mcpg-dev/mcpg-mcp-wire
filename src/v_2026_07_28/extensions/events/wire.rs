//! Wire shapes of the MCP Events extension (experimental; see the
//! [module docs](super)).

use std::fmt;

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::shared::caching::default_result_type_complete;
use crate::shared::jsonrpc::JsonRpcErrorBody;

// ---------------------------------------------------------------------------
// Identifiers.
// ---------------------------------------------------------------------------

/// SEP-2133 key of the extensions-map advertisement
/// (`capabilities.extensions["io.modelcontextprotocol/events"]`).
pub const EXTENSION_ID: &str = "io.modelcontextprotocol/events";

/// Key of the top-level advertisement (`capabilities.events`).
pub const CAPABILITY_KEY: &str = "events";

/// List the event types the caller may subscribe to.
pub const METHOD_LIST_EVENTS: &str = "events/list";
/// Create or refresh a webhook subscription (an upsert).
pub const METHOD_SUBSCRIBE_EVENTS: &str = "events/subscribe";
/// Drop a webhook subscription.
pub const METHOD_UNSUBSCRIBE_EVENTS: &str = "events/unsubscribe";
/// Poll one subscription.
pub const METHOD_POLL_EVENTS: &str = "events/poll";
/// Open a push stream for one subscription.
pub const METHOD_STREAM_EVENTS: &str = "events/stream";

/// The set of event types, or a descriptor, changed; the client re-lists.
pub const METHOD_NOTIFICATIONS_LIST_CHANGED: &str = "notifications/events/list_changed";

/// The one MCP-specific header on a webhook POST, beside the Standard
/// Webhooks `webhook-id` / `webhook-timestamp` / `webhook-signature`.
/// Lower case because `http::HeaderName` is; header names compare
/// case-insensitively on the wire.
pub const SUBSCRIPTION_ID_HEADER: &str = "x-mcp-subscription-id";

/// Prefix of a Standard Webhooks symmetric secret.
pub const WEBHOOK_SECRET_PREFIX: &str = mcpg_standard_webhooks::SECRET_PREFIX;
/// Fewest key bytes a `whsec_` secret may decode to.
pub const WEBHOOK_SECRET_MIN_BYTES: usize = mcpg_standard_webhooks::MIN_SECRET_BYTES;
/// Most key bytes a `whsec_` secret may decode to.
pub const WEBHOOK_SECRET_MAX_BYTES: usize = mcpg_standard_webhooks::MAX_SECRET_BYTES;

/// Largest webhook body a receiver is expected to accept (256 KiB); a
/// larger one draws `413`, which the sender must not retry.
pub const MAX_WEBHOOK_BODY_BYTES: usize = 262_144;

/// `webhook-id` prefix of a control envelope; the full id is
/// `msg_<type>_<random>`.
pub const CONTROL_WEBHOOK_ID_PREFIX: &str = "msg_";

/// The longest an `events/stream` may go without a data frame (an event or
/// a heartbeat), in milliseconds.
pub const MAX_STREAM_HEARTBEAT_MS: u64 = 30_000;

/// `true` for every method name in the `events/` namespace, routed or not.
pub fn is_events_method(method: &str) -> bool {
    method.starts_with("events/")
}

/// The body param an `Mcp-Name` header mirrors on an `events/*` method
/// that names an event type. SEP-2243 defines `Mcp-Name` for other
/// methods only, so on these it is optional: validated when a client
/// sends it, never required.
pub fn optional_name_header_field(method: &str) -> Option<&'static str> {
    match method {
        METHOD_SUBSCRIBE_EVENTS
        | METHOD_UNSUBSCRIBE_EVENTS
        | METHOD_POLL_EVENTS
        | METHOD_STREAM_EVENTS => Some("name"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Capability.
// ---------------------------------------------------------------------------

/// The capability object, advertised both as `capabilities.events` and
/// as the value of the [`EXTENSION_ID`] extensions entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsCapability {
    /// The server sends `notifications/events/list_changed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

impl EventsCapability {
    /// The capability as a JSON value, for the untyped extensions map.
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Map::new()))
    }

    /// What `capabilities`, a server's capabilities object, advertises for
    /// MCP Events: the [`EXTENSION_ID`] entry of its extensions map, else
    /// its top-level [`CAPABILITY_KEY`] object. `None` when it advertises
    /// neither; an entry this build cannot read counts as advertised, with
    /// nothing known about it.
    pub fn advertised_in(capabilities: &Value) -> Option<Self> {
        let advertised = capabilities
            .get("extensions")
            .and_then(|extensions| extensions.get(EXTENSION_ID))
            .filter(|entry| entry.is_object())
            .or_else(|| {
                capabilities
                    .get(CAPABILITY_KEY)
                    .filter(|entry| entry.is_object())
            })?;
        Some(serde_json::from_value(advertised.clone()).unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------
// Event types.
// ---------------------------------------------------------------------------

/// A delivery mode an event type offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryMode {
    Poll,
    Push,
    Webhook,
    /// A mode this build does not know. Parsed so a newer peer's
    /// descriptor still reads; never emitted.
    #[serde(other)]
    Unknown,
}

impl DeliveryMode {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Poll => "poll",
            Self::Push => "push",
            Self::Webhook => "webhook",
            Self::Unknown => "unknown",
        }
    }
}

/// One event type as `events/list` describes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventDescriptor {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Non-empty subset of the delivery modes.
    #[serde(default)]
    pub delivery: Vec<DeliveryMode>,
    /// JSON Schema of the subscription `arguments`.
    #[serde(default = "object_schema")]
    pub input_schema: Value,
    /// JSON Schema of each occurrence's `data`.
    #[serde(default = "object_schema")]
    pub payload_schema: Value,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

fn object_schema() -> Value {
    serde_json::json!({ "type": "object" })
}

/// `events/list` params.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEventsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// `events/list` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListEventsResult {
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    #[serde(default)]
    pub events: Vec<EventDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl ListEventsResult {
    pub fn new(events: Vec<EventDescriptor>, next_cursor: Option<String>) -> Self {
        Self {
            result_type: default_result_type_complete(),
            events,
            next_cursor,
            meta: None,
        }
    }
}

/// `notifications/events/list_changed` params.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EventsListChangedParams {
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl EventsListChangedParams {
    /// The `notifications/events/list_changed` notification carrying these
    /// params.
    pub fn to_notification(&self) -> Value {
        serde_json::json!({
            "jsonrpc": crate::shared::jsonrpc::JSONRPC_VERSION,
            "method": METHOD_NOTIFICATIONS_LIST_CHANGED,
            "params": self,
        })
    }
}

// ---------------------------------------------------------------------------
// The `whsec_` secret.
// ---------------------------------------------------------------------------

/// A client-supplied Standard Webhooks signing secret: `whsec_` followed
/// by standard base64 (padding optional) of 24 to 64 bytes.
///
/// Construction validates the format, so holding one means the value is
/// usable as an HMAC key. `Debug` never prints the value, and the bytes
/// are overwritten when the value is dropped. `Serialize` does emit the
/// value — a subscribe request has to carry it — so never serialize a
/// type that holds one into a log, an audit record or an error.
#[derive(Clone)]
pub struct WebhookSecret(String);

/// Why a value is not a usable `whsec_` secret. Carries no part of the
/// value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebhookSecretError {
    /// The value does not start with `whsec_`.
    MissingPrefix,
    /// The part after the prefix is not standard base64.
    NotBase64,
    /// The decoded key is shorter than 24 or longer than 64 bytes.
    BadLength,
}

impl WebhookSecretError {
    /// The message every rejection carries. It names the rule, never the
    /// value.
    pub const MESSAGE: &'static str =
        "delivery.secret must be `whsec_` followed by standard base64 of 24 to 64 bytes";
}

impl fmt::Display for WebhookSecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(Self::MESSAGE)
    }
}

impl std::error::Error for WebhookSecretError {}

impl WebhookSecret {
    /// Validate and wrap a `whsec_` secret, by the rules of
    /// [`mcpg_standard_webhooks::WhsecSecret::parse`].
    pub fn parse(value: &str) -> Result<Self, WebhookSecretError> {
        use mcpg_standard_webhooks::SecretError;
        mcpg_standard_webhooks::WhsecSecret::parse(value).map_err(|error| match error {
            SecretError::MissingPrefix => WebhookSecretError::MissingPrefix,
            SecretError::NotBase64 => WebhookSecretError::NotBase64,
            SecretError::KeyLength => WebhookSecretError::BadLength,
        })?;
        Ok(Self(value.to_owned()))
    }

    /// The signing key, for signing or verifying deliveries.
    pub fn signing_secret(&self) -> mcpg_standard_webhooks::WhsecSecret {
        mcpg_standard_webhooks::WhsecSecret::parse(&self.0)
            .expect("a WebhookSecret is validated when it is made")
    }

    /// The same secret, for a subscription request that names it.
    pub fn from_signing_secret(secret: &mcpg_standard_webhooks::WhsecSecret) -> Self {
        Self(secret.to_whsec_string().to_string())
    }

    /// The full `whsec_…` string, for a client sending it upstream. Treat
    /// the result as secret.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl PartialEq for WebhookSecret {
    /// Constant-time in the length of the shorter value, so a comparison
    /// against a stored secret does not leak how many leading bytes match.
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.0.as_bytes(), other.0.as_bytes());
        let mut diff = u8::from(a.len() != b.len());
        for (x, y) in a.iter().zip(b) {
            diff |= x ^ y;
        }
        diff == 0
    }
}

impl Eq for WebhookSecret {}

impl fmt::Debug for WebhookSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WebhookSecret(<redacted>)")
    }
}

impl Drop for WebhookSecret {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        overwrite(&mut bytes);
    }
}

/// Zero a buffer with writes the optimizer may not elide, even though the
/// buffer is freed right after.
fn overwrite(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() {
        // SAFETY: `byte` is a valid, aligned, exclusive reference.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

impl Serialize for WebhookSecret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WebhookSecret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SecretVisitor;
        impl de::Visitor<'_> for SecretVisitor {
            type Value = WebhookSecret;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a `whsec_` secret string")
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<WebhookSecret, E> {
                WebhookSecret::parse(value).map_err(|_| E::custom(WebhookSecretError::MESSAGE))
            }
        }
        deserializer.deserialize_str(SecretVisitor)
    }
}

// ---------------------------------------------------------------------------
// Lenient field helpers.
// ---------------------------------------------------------------------------

/// The `ttlMs` a subscribe request carries: absent, `null`, or a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TtlRequest {
    /// Field absent: the server picks its default lifetime.
    #[default]
    ServerDefault,
    /// `ttlMs: null`: the client asks for a subscription with no expiry.
    NoExpiry,
    /// A suggested lifetime in milliseconds.
    Millis(u64),
}

impl TtlRequest {
    pub fn is_server_default(&self) -> bool {
        matches!(self, Self::ServerDefault)
    }

    /// The request a `ttlMs` field makes: `None` when the field is absent.
    pub fn from_field(field: Option<&Value>) -> Result<Self, &'static str> {
        match field {
            None => Ok(Self::ServerDefault),
            Some(Value::Null) => Ok(Self::NoExpiry),
            Some(value) => millis_from_value(value).map(Self::Millis),
        }
    }
}

/// Subscription `arguments` as the wire reads them: an object, where an
/// absent field or `null` is the empty object.
pub fn arguments_from(field: Option<&Value>) -> Result<Map<String, Value>, &'static str> {
    match field {
        None | Some(Value::Null) => Ok(Map::new()),
        Some(Value::Object(map)) => Ok(map.clone()),
        Some(_) => Err("`arguments` must be a JSON object"),
    }
}

/// Field-level (de)serializer for [`TtlRequest`]. Paired with
/// `#[serde(default)]`, an absent field stays [`TtlRequest::ServerDefault`],
/// which is the one case the tri-state needs the field-level hook for.
mod ttl_request {
    use super::*;

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<TtlRequest, D::Error> {
        let value = Value::deserialize(deserializer)?;
        TtlRequest::from_field(Some(&value)).map_err(de::Error::custom)
    }

    pub fn serialize<S: Serializer>(ttl: &TtlRequest, serializer: S) -> Result<S::Ok, S::Error> {
        match ttl {
            TtlRequest::Millis(ms) => serializer.serialize_u64(*ms),
            TtlRequest::NoExpiry | TtlRequest::ServerDefault => serializer.serialize_none(),
        }
    }
}

/// A duration in whole milliseconds. Accepts a JSON integer, or a float
/// with no fractional part (some clients send every number as a double).
fn millis_from_value(value: &Value) -> Result<u64, &'static str> {
    const MESSAGE: &str = "a duration must be a non-negative whole number of milliseconds";
    if let Some(ms) = value.as_u64() {
        return Ok(ms);
    }
    match value.as_f64() {
        Some(f) if f.is_finite() && f >= 0.0 && f.fract() == 0.0 && f <= u64::MAX as f64 => {
            Ok(f as u64)
        }
        _ => Err(MESSAGE),
    }
}

fn optional_millis<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<u64>, D::Error> {
    match Option::<Value>::deserialize(deserializer)? {
        None | Some(Value::Null) => Ok(None),
        Some(value) => millis_from_value(&value)
            .map(Some)
            .map_err(de::Error::custom),
    }
}

/// Seconds from an older draft's field, converted to milliseconds.
fn optional_seconds_as_millis<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u64>, D::Error> {
    Ok(optional_millis(deserializer)?.map(|s| s.saturating_mul(1000)))
}

/// `arguments` by [`arguments_from`], so absent and `null` address the
/// same subscription.
fn arguments_object<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Map<String, Value>, D::Error> {
    match Value::deserialize(deserializer)? {
        Value::Object(map) => Ok(map),
        other => arguments_from(Some(&other)).map_err(de::Error::custom),
    }
}

fn default_webhook_mode() -> String {
    DeliveryMode::Webhook.as_str().to_owned()
}

// ---------------------------------------------------------------------------
// Webhook subscriptions.
// ---------------------------------------------------------------------------

/// `events/subscribe` `delivery`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookDelivery {
    /// Always `"webhook"` for a well-formed subscribe; absent reads as
    /// `"webhook"`. Any other value is a mode `events/subscribe` does not
    /// serve.
    #[serde(default = "default_webhook_mode")]
    pub mode: String,
    /// The `https` callback URL, as the client sent it.
    pub url: String,
    pub secret: WebhookSecret,
}

impl WebhookDelivery {
    pub fn is_webhook(&self) -> bool {
        self.mode == DeliveryMode::Webhook.as_str()
    }
}

/// `events/subscribe` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "RawSubscribeEventsParams")]
pub struct SubscribeEventsParams {
    pub name: String,
    pub arguments: Map<String, Value>,
    pub delivery: WebhookDelivery,
    /// Where to start delivery; `None` is "from now".
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_ms: Option<u64>,
    #[serde(
        skip_serializing_if = "TtlRequest::is_server_default",
        serialize_with = "ttl_request::serialize"
    )]
    pub ttl_ms: TtlRequest,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl SubscribeEventsParams {
    /// Parse `events/subscribe` params with errors that never quote a
    /// value from the request.
    ///
    /// A type error deep in the body makes serde quote the offending
    /// string, and the string may be the client's secret. The message
    /// keeps only what cannot carry a value: a missing field's name, or
    /// the fixed secret-format rule.
    pub fn from_value_redacted(params: Value) -> Result<Self, String> {
        serde_json::from_value(params).map_err(|error| redacted_params_error(&error))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSubscribeEventsParams {
    name: String,
    #[serde(default, deserialize_with = "arguments_object")]
    arguments: Map<String, Value>,
    delivery: WebhookDelivery,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default, deserialize_with = "optional_millis")]
    max_age_ms: Option<u64>,
    #[serde(
        default,
        rename = "maxAge",
        deserialize_with = "optional_seconds_as_millis"
    )]
    max_age_seconds_as_ms: Option<u64>,
    #[serde(default, deserialize_with = "ttl_request::deserialize")]
    ttl_ms: TtlRequest,
    #[serde(rename = "_meta", default)]
    meta: Option<Value>,
}

impl From<RawSubscribeEventsParams> for SubscribeEventsParams {
    fn from(raw: RawSubscribeEventsParams) -> Self {
        Self {
            name: raw.name,
            arguments: raw.arguments,
            delivery: raw.delivery,
            cursor: raw.cursor,
            max_age_ms: raw.max_age_ms.or(raw.max_age_seconds_as_ms),
            ttl_ms: raw.ttl_ms,
            meta: raw.meta,
        }
    }
}

/// `events/unsubscribe` `delivery`: the callback URL only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsubscribeDelivery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub url: String,
}

/// `events/unsubscribe` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeEventsParams {
    pub name: String,
    #[serde(default, deserialize_with = "arguments_object")]
    pub arguments: Map<String, Value>,
    pub delivery: UnsubscribeDelivery,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl UnsubscribeEventsParams {
    /// As [`SubscribeEventsParams::from_value_redacted`]: a client that
    /// sends its secret along on unsubscribe must not see it quoted back.
    pub fn from_value_redacted(params: Value) -> Result<Self, String> {
        serde_json::from_value(params).map_err(|error| redacted_params_error(&error))
    }
}

/// A serde error reduced to text that holds no request value.
fn redacted_params_error(error: &serde_json::Error) -> String {
    let text = error.to_string();
    if text.starts_with(WebhookSecretError::MESSAGE) {
        return WebhookSecretError::MESSAGE.to_owned();
    }
    if let Some(rest) = text.strip_prefix("missing field `")
        && let Some(end) = rest.find('`')
    {
        return format!("missing field `{}`", &rest[..end]);
    }
    "a field has the wrong type or an invalid value".to_owned()
}

/// `events/unsubscribe` result: an empty result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeEventsResult {
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl Default for UnsubscribeEventsResult {
    fn default() -> Self {
        Self {
            result_type: default_result_type_complete(),
            meta: None,
        }
    }
}

/// A delivery-failure category. The only failure detail that crosses the
/// wire: never a status line, header or response body from the endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DeliveryErrorCategory {
    #[serde(rename = "connection_refused")]
    ConnectionRefused,
    #[serde(rename = "timeout")]
    Timeout,
    #[serde(rename = "tls_error")]
    TlsError,
    #[serde(rename = "http_4xx")]
    Http4xx,
    #[serde(rename = "http_5xx")]
    Http5xx,
    #[serde(rename = "challenge_failed")]
    ChallengeFailed,
    /// Delivery is paused by the operator. An mcpg category, reported in
    /// `deliveryStatus.lastError` only, never as a `-32015` reason.
    #[serde(rename = "paused")]
    Paused,
    /// A category this build does not know. Parsed, never emitted.
    #[serde(other, rename = "unknown")]
    Unknown,
}

impl DeliveryErrorCategory {
    /// The wire spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ConnectionRefused => "connection_refused",
            Self::Timeout => "timeout",
            Self::TlsError => "tls_error",
            Self::Http4xx => "http_4xx",
            Self::Http5xx => "http_5xx",
            Self::ChallengeFailed => "challenge_failed",
            Self::Paused => "paused",
            Self::Unknown => "unknown",
        }
    }
}

/// `deliveryStatus` on a subscribe or refresh result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryStatus {
    /// `false` while delivery is suspended or paused.
    pub active: bool,
    /// RFC 3339 time of the last acknowledged delivery; `null` before the
    /// first.
    #[serde(default)]
    pub last_delivery_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<DeliveryErrorCategory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_since: Option<String>,
    /// Deliveries are being delayed by the server, not failing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throttled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

/// `events/subscribe` result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeEventsResult {
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    /// Server-derived routing handle, deterministic over the subscription
    /// key. Never accepted as input.
    pub id: String,
    /// RFC 3339 expiry of the grant; `null` only for a no-expiry grant.
    /// Always serialized.
    #[serde(default)]
    pub refresh_before: Option<String>,
    /// The safe-to-persist watermark; `null` for a type without replay.
    /// Always serialized.
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_status: Option<DeliveryStatus>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl SubscribeEventsResult {
    pub fn new(
        id: String,
        refresh_before: Option<String>,
        cursor: Option<String>,
        truncated: bool,
    ) -> Self {
        Self {
            result_type: default_result_type_complete(),
            id,
            refresh_before,
            cursor,
            truncated,
            delivery_status: None,
            meta: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Poll.
// ---------------------------------------------------------------------------

/// `events/poll` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "RawPollEventsParams")]
pub struct PollEventsParams {
    pub name: String,
    pub arguments: Map<String, Value>,
    /// `None` is "from now".
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_events: Option<u32>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPollEventsParams {
    name: String,
    #[serde(default, deserialize_with = "arguments_object")]
    arguments: Map<String, Value>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default, deserialize_with = "optional_millis")]
    max_age_ms: Option<u64>,
    #[serde(
        default,
        rename = "maxAge",
        deserialize_with = "optional_seconds_as_millis"
    )]
    max_age_seconds_as_ms: Option<u64>,
    #[serde(default)]
    max_events: Option<u32>,
    #[serde(rename = "_meta", default)]
    meta: Option<Value>,
}

impl From<RawPollEventsParams> for PollEventsParams {
    fn from(raw: RawPollEventsParams) -> Self {
        Self {
            name: raw.name,
            arguments: raw.arguments,
            cursor: raw.cursor,
            max_age_ms: raw.max_age_ms.or(raw.max_age_seconds_as_ms),
            max_events: raw.max_events,
            meta: raw.meta,
        }
    }
}

/// `events/poll` result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "RawPollEventsResult")]
pub struct PollEventsResult {
    pub result_type: String,
    pub events: Vec<EventOccurrence>,
    /// Always serialized; `null` for a type without replay.
    pub cursor: Option<String>,
    pub truncated: bool,
    pub has_more: bool,
    pub next_poll_ms: u64,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawPollEventsResult {
    #[serde(default = "default_result_type_complete")]
    result_type: String,
    #[serde(default)]
    events: Vec<EventOccurrence>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default)]
    truncated: bool,
    #[serde(default)]
    has_more: bool,
    #[serde(default, deserialize_with = "optional_millis")]
    next_poll_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_seconds_as_millis")]
    next_poll_seconds: Option<u64>,
    #[serde(rename = "_meta", default)]
    meta: Option<Value>,
}

impl PollEventsResult {
    pub fn new(
        events: Vec<EventOccurrence>,
        cursor: Option<String>,
        truncated: bool,
        has_more: bool,
        next_poll_ms: u64,
    ) -> Self {
        Self {
            result_type: default_result_type_complete(),
            events,
            cursor,
            truncated,
            has_more,
            next_poll_ms,
            meta: None,
        }
    }
}

impl From<RawPollEventsResult> for PollEventsResult {
    fn from(raw: RawPollEventsResult) -> Self {
        Self {
            result_type: raw.result_type,
            events: raw.events,
            cursor: raw.cursor,
            truncated: raw.truncated,
            has_more: raw.has_more,
            next_poll_ms: raw.next_poll_ms.or(raw.next_poll_seconds).unwrap_or(0),
            meta: raw.meta,
        }
    }
}

// ---------------------------------------------------------------------------
// Push.
// ---------------------------------------------------------------------------

/// The stream is open: `{cursor, truncated}`. Sent first, and again with
/// `truncated: true` when events were skipped mid-stream.
pub const METHOD_NOTIFICATIONS_ACTIVE: &str = "notifications/events/active";
/// One occurrence; the params are an [`EventOccurrence`].
pub const METHOD_NOTIFICATIONS_EVENT: &str = "notifications/events/event";
/// Nothing new up to `{cursor}`; the stream is alive.
pub const METHOD_NOTIFICATIONS_HEARTBEAT: &str = "notifications/events/heartbeat";
/// A transient failure, `{error}`; the stream stays open.
pub const METHOD_NOTIFICATIONS_ERROR: &str = "notifications/events/error";
/// The subscription ended, `{error}`; the stream closes after it.
pub const METHOD_NOTIFICATIONS_TERMINATED: &str = "notifications/events/terminated";

/// `events/stream` params.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "RawStreamEventsParams")]
pub struct StreamEventsParams {
    pub name: String,
    pub arguments: Map<String, Value>,
    /// `None` is "from now".
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_ms: Option<u64>,
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStreamEventsParams {
    name: String,
    #[serde(default, deserialize_with = "arguments_object")]
    arguments: Map<String, Value>,
    #[serde(default)]
    cursor: Option<String>,
    #[serde(default, deserialize_with = "optional_millis")]
    max_age_ms: Option<u64>,
    #[serde(
        default,
        rename = "maxAge",
        deserialize_with = "optional_seconds_as_millis"
    )]
    max_age_seconds_as_ms: Option<u64>,
    #[serde(rename = "_meta", default)]
    meta: Option<Value>,
}

impl From<RawStreamEventsParams> for StreamEventsParams {
    fn from(raw: RawStreamEventsParams) -> Self {
        Self {
            name: raw.name,
            arguments: raw.arguments,
            cursor: raw.cursor,
            max_age_ms: raw.max_age_ms.or(raw.max_age_seconds_as_ms),
            meta: raw.meta,
        }
    }
}

/// One frame of an `events/stream` response.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamFrame {
    Active {
        cursor: Option<String>,
        truncated: bool,
    },
    Event(EventOccurrence),
    Heartbeat {
        cursor: Option<String>,
    },
    Error(JsonRpcErrorBody),
    Terminated(JsonRpcErrorBody),
}

impl StreamFrame {
    /// The notification method of this frame.
    pub fn method(&self) -> &'static str {
        match self {
            Self::Active { .. } => METHOD_NOTIFICATIONS_ACTIVE,
            Self::Event(_) => METHOD_NOTIFICATIONS_EVENT,
            Self::Heartbeat { .. } => METHOD_NOTIFICATIONS_HEARTBEAT,
            Self::Error(_) => METHOD_NOTIFICATIONS_ERROR,
            Self::Terminated(_) => METHOD_NOTIFICATIONS_TERMINATED,
        }
    }

    /// The JSON-RPC notification, with `subscription_id` (the stream
    /// request's id) added to `params._meta`; an occurrence's own `_meta`
    /// entries stay.
    pub fn to_notification(&self, subscription_id: &Value) -> Value {
        let params = match self {
            Self::Active { cursor, truncated } => {
                serde_json::json!({ "cursor": cursor, "truncated": truncated })
            }
            Self::Event(occurrence) => serde_json::to_value(occurrence).unwrap_or_default(),
            Self::Heartbeat { cursor } => serde_json::json!({ "cursor": cursor }),
            Self::Error(error) | Self::Terminated(error) => {
                serde_json::json!({ "error": error })
            }
        };
        let mut notification = serde_json::json!({
            "jsonrpc": crate::shared::jsonrpc::JSONRPC_VERSION,
            "method": self.method(),
            "params": params,
        });
        crate::v_2026_07_28::wire::subscriptions::inject_subscription_id_meta(
            &mut notification,
            subscription_id,
        );
        notification
    }

    /// The frame a stream notification carries, without the stream's
    /// `subscriptionId`; `None` for any other message or a malformed one.
    pub fn from_notification(notification: &Value) -> Option<Self> {
        #[derive(Deserialize)]
        struct Positioned {
            #[serde(default)]
            cursor: Option<String>,
            #[serde(default)]
            truncated: bool,
        }
        #[derive(Deserialize)]
        struct Failed {
            error: JsonRpcErrorBody,
        }
        let method = notification.get("method")?.as_str()?;
        let mut params = notification
            .get("params")
            .cloned()
            .unwrap_or_else(|| Value::Object(Map::new()));
        if let Some(meta) = params.get_mut("_meta").and_then(Value::as_object_mut) {
            meta.remove(crate::v_2026_07_28::wire::subscriptions::META_KEY_SUBSCRIPTION_ID);
            if meta.is_empty()
                && let Some(object) = params.as_object_mut()
            {
                object.remove("_meta");
            }
        }
        match method {
            METHOD_NOTIFICATIONS_ACTIVE => {
                let p: Positioned = serde_json::from_value(params).ok()?;
                Some(Self::Active {
                    cursor: p.cursor,
                    truncated: p.truncated,
                })
            }
            METHOD_NOTIFICATIONS_EVENT => serde_json::from_value(params).ok().map(Self::Event),
            METHOD_NOTIFICATIONS_HEARTBEAT => {
                let p: Positioned = serde_json::from_value(params).ok()?;
                Some(Self::Heartbeat { cursor: p.cursor })
            }
            METHOD_NOTIFICATIONS_ERROR => serde_json::from_value::<Failed>(params)
                .ok()
                .map(|f| Self::Error(f.error)),
            METHOD_NOTIFICATIONS_TERMINATED => serde_json::from_value::<Failed>(params)
                .ok()
                .map(|f| Self::Terminated(f.error)),
            _ => None,
        }
    }
}

/// The `StreamEventsResult` frame that ends a stream the server closes:
/// an empty result answering request `id`, shaped as every long-lived
/// stream of this wire closes.
pub fn stream_events_result(id: &Value) -> Value {
    crate::v_2026_07_28::wire::subscriptions::closed_stream_result(id)
}

// ---------------------------------------------------------------------------
// Occurrences and webhook bodies.
// ---------------------------------------------------------------------------

/// One occurrence: an entry of a poll result, a push notification's
/// params, or a webhook event body. Serializes to a fixed field set, so
/// `data` can never put a top-level `type` key into a webhook body and
/// pass for a control envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventOccurrence {
    /// Stable id for client-side dedup; the `webhook-id` of its POST.
    pub event_id: String,
    pub name: String,
    /// RFC 3339 time the event occurred.
    pub timestamp: String,
    #[serde(default = "empty_object")]
    pub data: Value,
    /// Subscription position after this event; `null` for a type without
    /// replay. Always serialized.
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

fn empty_object() -> Value {
    Value::Object(Map::new())
}

/// A webhook body that is not an occurrence, discriminated by `type`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ControlEnvelope {
    /// Events were skipped; the client persists `cursor` and treats the
    /// gap as `truncated: true`.
    Gap {
        #[serde(default)]
        cursor: Option<String>,
    },
    /// The subscription has ended server-side.
    Terminated { error: JsonRpcErrorBody },
    /// Proof-of-intent challenge sent before the first delivery; the
    /// endpoint echoes `challenge` in a 2xx body.
    Verification { challenge: String },
}

impl ControlEnvelope {
    /// The `type` discriminator.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Gap { .. } => "gap",
            Self::Terminated { .. } => "terminated",
            Self::Verification { .. } => "verification",
        }
    }

    /// The `webhook-id` of this envelope's POST: `msg_<type>_<random>`.
    /// The caller mints `random` once per envelope and reuses it on every
    /// retry, so the receiver can dedup.
    pub fn webhook_id(&self, random: &str) -> String {
        format!("{CONTROL_WEBHOOK_ID_PREFIX}{}_{random}", self.kind())
    }
}

/// The 2xx body an endpoint answers a `verification` envelope with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResponse {
    pub challenge: String,
}

/// A parsed webhook body.
#[derive(Debug, Clone, PartialEq)]
pub enum WebhookBody {
    Event(EventOccurrence),
    Control(ControlEnvelope),
}

impl WebhookBody {
    /// Classify and parse a webhook body: a top-level `type` makes it a
    /// control envelope, anything else an occurrence.
    pub fn from_value(body: Value) -> Result<Self, serde_json::Error> {
        if body.get("type").is_some() {
            serde_json::from_value(body).map(Self::Control)
        } else {
            serde_json::from_value(body).map(Self::Event)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 24, 32 and 64 random-looking bytes, standard base64.
    const SECRET_24: &str = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";
    const SECRET_32: &str = "whsec_AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
    const SECRET_64: &str = "whsec_AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+Pw==";

    fn subscribe_body() -> Value {
        json!({
            "name": "incident.created",
            "arguments": { "severity": "P1" },
            "delivery": {
                "mode": "webhook",
                "url": "https://proxy.example.com/hooks/client123",
                "secret": SECRET_24,
            },
            "cursor": null,
            "maxAgeMs": 300000,
            "ttlMs": 3600000
        })
    }

    #[test]
    fn identifiers_match_the_sketch() {
        assert_eq!(EXTENSION_ID, "io.modelcontextprotocol/events");
        assert_eq!(CAPABILITY_KEY, "events");
        assert_eq!(METHOD_LIST_EVENTS, "events/list");
        assert_eq!(METHOD_SUBSCRIBE_EVENTS, "events/subscribe");
        assert_eq!(METHOD_UNSUBSCRIBE_EVENTS, "events/unsubscribe");
        assert_eq!(METHOD_POLL_EVENTS, "events/poll");
        assert_eq!(
            METHOD_NOTIFICATIONS_LIST_CHANGED,
            "notifications/events/list_changed"
        );
        assert_eq!(MAX_WEBHOOK_BODY_BYTES, 256 * 1024);
        assert!(
            http::HeaderName::from_static(SUBSCRIPTION_ID_HEADER)
                .as_str()
                .eq_ignore_ascii_case("X-MCP-Subscription-Id")
        );
    }

    #[test]
    fn events_methods_are_the_events_namespace() {
        for method in [
            "events/list",
            "events/subscribe",
            "events/unsubscribe",
            "events/poll",
            "events/stream",
        ] {
            assert!(is_events_method(method), "{method}");
        }
        for method in [
            "tools/call",
            "events",
            "",
            "notifications/events/list_changed",
        ] {
            assert!(!is_events_method(method), "{method}");
        }
    }

    #[test]
    fn methods_that_name_an_event_type_mirror_it_in_an_optional_mcp_name() {
        for method in [
            "events/subscribe",
            "events/unsubscribe",
            "events/poll",
            "events/stream",
        ] {
            assert_eq!(optional_name_header_field(method), Some("name"), "{method}");
        }
        assert_eq!(optional_name_header_field("events/list"), None);
        assert_eq!(optional_name_header_field("tools/call"), None);
    }

    #[test]
    fn the_capability_is_read_from_either_location() {
        let flagged = EventsCapability {
            list_changed: Some(true),
        };
        let extension = json!({ "extensions": { EXTENSION_ID: { "listChanged": true } } });
        assert_eq!(
            EventsCapability::advertised_in(&extension),
            Some(flagged.clone())
        );
        let top_level = json!({ CAPABILITY_KEY: {} });
        assert_eq!(
            EventsCapability::advertised_in(&top_level),
            Some(EventsCapability::default())
        );
        let both = json!({
            CAPABILITY_KEY: {},
            "extensions": { EXTENSION_ID: { "listChanged": true } }
        });
        assert_eq!(
            EventsCapability::advertised_in(&both),
            Some(flagged),
            "the extensions entry wins"
        );
        for absent in [
            json!({}),
            json!({ "tools": {} }),
            json!({ CAPABILITY_KEY: true }),
            json!({ "extensions": { EXTENSION_ID: null } }),
        ] {
            assert_eq!(EventsCapability::advertised_in(&absent), None, "{absent}");
        }
    }

    #[test]
    fn capability_value_is_an_empty_object_without_list_changed() {
        assert_eq!(EventsCapability::default().to_value(), json!({}));
        assert_eq!(
            serde_json::to_value(EventsCapability::default()).unwrap(),
            json!({})
        );
        let with_flag = EventsCapability {
            list_changed: Some(true),
        };
        assert_eq!(with_flag.to_value(), json!({ "listChanged": true }));
        assert_eq!(
            serde_json::to_value(&with_flag).unwrap(),
            with_flag.to_value()
        );
        let parsed: EventsCapability =
            serde_json::from_value(json!({ "listChanged": true, "future": 1 })).unwrap();
        assert_eq!(parsed, with_flag);
    }

    #[test]
    fn webhook_secret_accepts_24_to_64_bytes_with_or_without_padding() {
        for secret in [SECRET_24, SECRET_32, SECRET_64] {
            let parsed = WebhookSecret::parse(secret).expect(secret);
            assert_eq!(parsed.expose_secret(), secret);
        }
        let unpadded = SECRET_32.trim_end_matches('=');
        assert!(WebhookSecret::parse(unpadded).is_ok());
    }

    #[test]
    fn webhook_secret_rejects_everything_else() {
        // 23 and 65 bytes.
        let short = format!(
            "whsec_{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [7u8; 23])
        );
        let long = format!(
            "whsec_{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, [7u8; 65])
        );
        assert_eq!(
            WebhookSecret::parse(&short).unwrap_err(),
            WebhookSecretError::BadLength
        );
        assert_eq!(
            WebhookSecret::parse(&long).unwrap_err(),
            WebhookSecretError::BadLength
        );
        assert_eq!(
            WebhookSecret::parse("MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw").unwrap_err(),
            WebhookSecretError::MissingPrefix
        );
        assert_eq!(
            WebhookSecret::parse("WHSEC_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw").unwrap_err(),
            WebhookSecretError::MissingPrefix
        );
        for not_base64 in [
            "whsec_not base64 at all!!",
            "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaS-",
            "whsec_",
        ] {
            assert!(WebhookSecret::parse(not_base64).is_err(), "{not_base64}");
        }
    }

    #[test]
    fn webhook_secret_never_appears_in_debug_or_errors() {
        let parsed = WebhookSecret::parse(SECRET_24).unwrap();
        let debug = format!("{parsed:?}");
        assert!(!debug.contains("MfKQ9r8"), "{debug}");

        let params: SubscribeEventsParams = serde_json::from_value(subscribe_body()).unwrap();
        let debug = format!("{params:?}");
        assert!(!debug.contains("MfKQ9r8"), "{debug}");

        let bad = "whsec_c2hvcnQ=";
        let mut body = subscribe_body();
        body["delivery"]["secret"] = json!(bad);
        let error = SubscribeEventsParams::from_value_redacted(body.clone()).unwrap_err();
        assert_eq!(error, WebhookSecretError::MESSAGE);
        let serde_error = serde_json::from_value::<SubscribeEventsParams>(body)
            .unwrap_err()
            .to_string();
        assert!(!serde_error.contains("c2hvcnQ"), "{serde_error}");
    }

    #[test]
    fn redacted_parse_never_quotes_a_misplaced_secret() {
        // The secret sits where an object belongs, so serde quotes it.
        let mut body = subscribe_body();
        body["delivery"] = json!(SECRET_24);
        let raw = serde_json::from_value::<SubscribeEventsParams>(body.clone())
            .unwrap_err()
            .to_string();
        assert!(raw.contains("MfKQ9r8"), "serde quotes the value: {raw}");
        let redacted = SubscribeEventsParams::from_value_redacted(body).unwrap_err();
        assert!(!redacted.contains("MfKQ9r8"), "{redacted}");

        let mut body = subscribe_body();
        body.as_object_mut().unwrap().remove("name");
        assert_eq!(
            SubscribeEventsParams::from_value_redacted(body).unwrap_err(),
            "missing field `name`"
        );

        let unsubscribe = json!({ "name": "x", "delivery": SECRET_24 });
        let redacted = UnsubscribeEventsParams::from_value_redacted(unsubscribe).unwrap_err();
        assert!(!redacted.contains("MfKQ9r8"), "{redacted}");
    }

    #[test]
    fn webhook_secret_compares_by_value() {
        let a = WebhookSecret::parse(SECRET_24).unwrap();
        let b = WebhookSecret::parse(SECRET_24).unwrap();
        let c = WebhookSecret::parse(SECRET_32).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn subscribe_params_parse_the_sketch_example() {
        let params: SubscribeEventsParams = serde_json::from_value(subscribe_body()).unwrap();
        assert_eq!(params.name, "incident.created");
        assert_eq!(params.arguments["severity"], "P1");
        assert!(params.delivery.is_webhook());
        assert_eq!(
            params.delivery.url,
            "https://proxy.example.com/hooks/client123"
        );
        assert_eq!(params.delivery.secret.expose_secret(), SECRET_24);
        assert_eq!(params.cursor, None);
        assert_eq!(params.max_age_ms, Some(300_000));
        assert_eq!(params.ttl_ms, TtlRequest::Millis(3_600_000));
    }

    #[test]
    fn ttl_ms_is_tri_state() {
        let mut body = subscribe_body();
        body.as_object_mut().unwrap().remove("ttlMs");
        let absent: SubscribeEventsParams = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(absent.ttl_ms, TtlRequest::ServerDefault);

        body["ttlMs"] = Value::Null;
        let null: SubscribeEventsParams = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(null.ttl_ms, TtlRequest::NoExpiry);

        body["ttlMs"] = json!(600000.0);
        let float: SubscribeEventsParams = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(float.ttl_ms, TtlRequest::Millis(600_000));

        for bad in [json!(-1), json!(1.5), json!("3600000")] {
            body["ttlMs"] = bad.clone();
            assert!(
                serde_json::from_value::<SubscribeEventsParams>(body.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn ttl_ms_serializes_back_to_the_same_tri_state() {
        let mut params: SubscribeEventsParams = serde_json::from_value(subscribe_body()).unwrap();
        for ttl in [
            TtlRequest::ServerDefault,
            TtlRequest::NoExpiry,
            TtlRequest::Millis(5),
        ] {
            params.ttl_ms = ttl;
            let value = serde_json::to_value(&params).unwrap();
            match ttl {
                TtlRequest::ServerDefault => assert!(value.get("ttlMs").is_none()),
                TtlRequest::NoExpiry => assert_eq!(value["ttlMs"], Value::Null),
                TtlRequest::Millis(ms) => assert_eq!(value["ttlMs"], ms),
            }
            let back: SubscribeEventsParams = serde_json::from_value(value).unwrap();
            assert_eq!(back, params);
        }
    }

    #[test]
    fn subscribe_params_are_lenient() {
        let body = json!({
            "name": "x",
            "delivery": { "url": "https://h.example/a", "secret": SECRET_24, "extra": 1 },
            "somethingNew": true,
        });
        let params: SubscribeEventsParams = serde_json::from_value(body).unwrap();
        assert!(params.arguments.is_empty(), "absent arguments read as {{}}");
        assert_eq!(params.cursor, None, "absent cursor reads as null");
        assert!(params.delivery.is_webhook(), "absent mode reads as webhook");
        assert_eq!(params.max_age_ms, None);

        let null_arguments = json!({
            "name": "x",
            "arguments": null,
            "delivery": { "url": "https://h.example/a", "secret": SECRET_24 },
        });
        let params: SubscribeEventsParams = serde_json::from_value(null_arguments).unwrap();
        assert!(params.arguments.is_empty());

        let other_mode = json!({
            "name": "x",
            "delivery": { "mode": "push", "url": "https://h.example/a", "secret": SECRET_24 },
        });
        let params: SubscribeEventsParams = serde_json::from_value(other_mode).unwrap();
        assert!(!params.delivery.is_webhook());
        assert_eq!(params.delivery.mode, "push");
    }

    #[test]
    fn older_max_age_spelling_is_seconds() {
        let mut body = subscribe_body();
        let obj = body.as_object_mut().unwrap();
        obj.remove("maxAgeMs");
        obj.insert("maxAge".to_owned(), json!(300));
        let params: SubscribeEventsParams = serde_json::from_value(body.clone()).unwrap();
        assert_eq!(params.max_age_ms, Some(300_000));

        // The current spelling wins when both are present.
        body["maxAgeMs"] = json!(1000);
        let params: SubscribeEventsParams = serde_json::from_value(body).unwrap();
        assert_eq!(params.max_age_ms, Some(1000));
        let emitted = serde_json::to_value(&params).unwrap();
        assert_eq!(emitted["maxAgeMs"], 1000);
        assert!(emitted.get("maxAge").is_none());
    }

    #[test]
    fn subscribe_params_round_trip_with_the_secret() {
        let params: SubscribeEventsParams = serde_json::from_value(subscribe_body()).unwrap();
        let value = serde_json::to_value(&params).unwrap();
        assert_eq!(value["delivery"]["secret"], SECRET_24);
        assert_eq!(value["delivery"]["mode"], "webhook");
        assert_eq!(value["cursor"], Value::Null);
        let back: SubscribeEventsParams = serde_json::from_value(value).unwrap();
        assert_eq!(back, params);
    }

    #[test]
    fn unsubscribe_params_take_the_url_and_ignore_a_secret() {
        let params: UnsubscribeEventsParams = serde_json::from_value(json!({
            "name": "incident.created",
            "arguments": { "severity": "P1" },
            "delivery": { "url": "https://proxy.example.com/hooks/client123", "secret": SECRET_24 }
        }))
        .unwrap();
        assert_eq!(
            params.delivery.url,
            "https://proxy.example.com/hooks/client123"
        );
        let emitted = serde_json::to_value(&params).unwrap();
        assert!(emitted["delivery"].get("secret").is_none());
        assert!(emitted["delivery"].get("mode").is_none());

        let no_arguments: UnsubscribeEventsParams = serde_json::from_value(json!({
            "name": "x", "delivery": { "url": "https://h.example/a" }
        }))
        .unwrap();
        assert!(no_arguments.arguments.is_empty());
    }

    #[test]
    fn unsubscribe_result_is_an_empty_complete_result() {
        assert_eq!(
            serde_json::to_value(UnsubscribeEventsResult::default()).unwrap(),
            json!({ "resultType": "complete" })
        );
        let parsed: UnsubscribeEventsResult = serde_json::from_value(json!({})).unwrap();
        assert_eq!(parsed, UnsubscribeEventsResult::default());
    }

    #[test]
    fn subscribe_result_always_carries_refresh_before_and_cursor() {
        let result = SubscribeEventsResult::new("sub_a3f1".to_owned(), None, None, false);
        let value = serde_json::to_value(&result).unwrap();
        assert_eq!(
            value,
            json!({
                "resultType": "complete",
                "id": "sub_a3f1",
                "refreshBefore": null,
                "cursor": null,
                "truncated": false
            })
        );

        let mut refreshed = SubscribeEventsResult::new(
            "sub_a3f1".to_owned(),
            Some("2026-02-19T17:00:00Z".to_owned()),
            Some("ev1.x".to_owned()),
            true,
        );
        refreshed.delivery_status = Some(DeliveryStatus {
            active: false,
            last_delivery_at: Some("2026-02-19T15:45:00Z".to_owned()),
            last_error: Some(DeliveryErrorCategory::Http4xx),
            failed_since: Some("2026-02-19T15:50:00Z".to_owned()),
            throttled: None,
            retry_after_ms: None,
        });
        let value = serde_json::to_value(&refreshed).unwrap();
        assert_eq!(value["refreshBefore"], "2026-02-19T17:00:00Z");
        assert_eq!(value["truncated"], true);
        assert_eq!(
            value["deliveryStatus"],
            json!({
                "active": false,
                "lastDeliveryAt": "2026-02-19T15:45:00Z",
                "lastError": "http_4xx",
                "failedSince": "2026-02-19T15:50:00Z"
            })
        );
        let back: SubscribeEventsResult = serde_json::from_value(value).unwrap();
        assert_eq!(back, refreshed);
    }

    #[test]
    fn healthy_delivery_status_reports_null_last_error() {
        let status = DeliveryStatus {
            active: true,
            last_delivery_at: None,
            last_error: None,
            failed_since: None,
            throttled: Some(true),
            retry_after_ms: Some(60_000),
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            json!({
                "active": true,
                "lastDeliveryAt": null,
                "lastError": null,
                "throttled": true,
                "retryAfterMs": 60000
            })
        );
    }

    #[test]
    fn error_categories_use_the_sketch_spellings() {
        for (category, text) in [
            (
                DeliveryErrorCategory::ConnectionRefused,
                "connection_refused",
            ),
            (DeliveryErrorCategory::Timeout, "timeout"),
            (DeliveryErrorCategory::TlsError, "tls_error"),
            (DeliveryErrorCategory::Http4xx, "http_4xx"),
            (DeliveryErrorCategory::Http5xx, "http_5xx"),
            (DeliveryErrorCategory::ChallengeFailed, "challenge_failed"),
            (DeliveryErrorCategory::Paused, "paused"),
        ] {
            assert_eq!(serde_json::to_value(category).unwrap(), json!(text));
            assert_eq!(category.as_str(), text);
            let back: DeliveryErrorCategory = serde_json::from_value(json!(text)).unwrap();
            assert_eq!(back, category);
        }
        let unknown: DeliveryErrorCategory = serde_json::from_value(json!("dns_wobble")).unwrap();
        assert_eq!(unknown, DeliveryErrorCategory::Unknown);
    }

    #[test]
    fn descriptor_round_trips_and_tolerates_unknown_modes() {
        let value = json!({
            "name": "incident.created",
            "description": "Fires when a new PagerDuty incident is created",
            "delivery": ["webhook", "push", "poll"],
            "inputSchema": { "type": "object", "properties": { "severity": { "type": "string" } } },
            "payloadSchema": { "type": "object" },
            "_meta": { "x": 1 }
        });
        let descriptor: EventDescriptor = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            descriptor.delivery,
            vec![
                DeliveryMode::Webhook,
                DeliveryMode::Push,
                DeliveryMode::Poll
            ]
        );
        assert_eq!(serde_json::to_value(&descriptor).unwrap(), value);

        let lenient: EventDescriptor = serde_json::from_value(json!({
            "name": "x", "delivery": ["carrier-pigeon", "webhook"], "title": "ignored"
        }))
        .unwrap();
        assert_eq!(
            lenient.delivery,
            vec![DeliveryMode::Unknown, DeliveryMode::Webhook]
        );
        assert_eq!(lenient.input_schema, json!({ "type": "object" }));
        assert_eq!(lenient.payload_schema, json!({ "type": "object" }));
    }

    #[test]
    fn list_params_and_result_shapes() {
        let params: ListEventsParams = serde_json::from_value(json!({ "cursor": "c2" })).unwrap();
        assert_eq!(params.cursor.as_deref(), Some("c2"));
        let null_cursor: ListEventsParams =
            serde_json::from_value(json!({ "cursor": null })).unwrap();
        assert_eq!(null_cursor.cursor, None);

        let result = ListEventsResult::new(Vec::new(), None);
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            json!({ "resultType": "complete", "events": [] })
        );
        let paged = ListEventsResult::new(Vec::new(), Some("next".to_owned()));
        assert_eq!(serde_json::to_value(&paged).unwrap()["nextCursor"], "next");
        let parsed: ListEventsResult =
            serde_json::from_value(json!({ "events": [{ "name": "a" }] })).unwrap();
        assert_eq!(parsed.events[0].name, "a");
        assert_eq!(parsed.result_type, "complete");
    }

    #[test]
    fn poll_params_and_result_accept_older_spellings() {
        let params: PollEventsParams = serde_json::from_value(json!({
            "name": "email.received",
            "arguments": { "from": "*@example.com" },
            "cursor": null,
            "maxAge": 300,
            "maxEvents": 50
        }))
        .unwrap();
        assert_eq!(params.max_age_ms, Some(300_000));
        assert_eq!(params.max_events, Some(50));
        assert_eq!(params.cursor, None);

        let result: PollEventsResult = serde_json::from_value(json!({
            "events": [],
            "cursor": "historyId_99842",
            "truncated": false,
            "hasMore": false,
            "nextPollSeconds": 30
        }))
        .unwrap();
        assert_eq!(result.next_poll_ms, 30_000);
        let emitted = serde_json::to_value(&result).unwrap();
        assert_eq!(emitted["nextPollMs"], 30_000);
        assert!(emitted.get("nextPollSeconds").is_none());
        assert_eq!(emitted["resultType"], "complete");

        let current: PollEventsResult = serde_json::from_value(json!({
            "events": [], "cursor": null, "truncated": true, "hasMore": true, "nextPollMs": 1500
        }))
        .unwrap();
        assert_eq!(current.next_poll_ms, 1500);
        assert_eq!(
            serde_json::to_value(&current).unwrap()["cursor"],
            Value::Null
        );
    }

    #[test]
    fn occurrence_has_a_fixed_field_set() {
        let occurrence = EventOccurrence {
            event_id: "evt_789".to_owned(),
            name: "incident.created".to_owned(),
            timestamp: "2026-02-19T16:00:00Z".to_owned(),
            data: json!({ "type": "gap", "incidentId": "INC-1234" }),
            cursor: None,
            meta: None,
        };
        let value = serde_json::to_value(&occurrence).unwrap();
        assert_eq!(
            value,
            json!({
                "eventId": "evt_789",
                "name": "incident.created",
                "timestamp": "2026-02-19T16:00:00Z",
                "data": { "type": "gap", "incidentId": "INC-1234" },
                "cursor": null
            })
        );
        // A `type` inside `data` never makes the body a control envelope.
        assert!(matches!(
            WebhookBody::from_value(value).unwrap(),
            WebhookBody::Event(_)
        ));
    }

    #[test]
    fn control_envelopes_use_the_type_discriminator() {
        let gap = ControlEnvelope::Gap {
            cursor: Some("fresh".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(&gap).unwrap(),
            json!({ "type": "gap", "cursor": "fresh" })
        );
        assert_eq!(gap.webhook_id("r4nd"), "msg_gap_r4nd");

        let terminated = ControlEnvelope::Terminated {
            error: JsonRpcErrorBody {
                code: -32012,
                message: "Forbidden".to_owned(),
                data: Some(json!({ "reason": "revoked" })),
            },
        };
        assert_eq!(
            serde_json::to_value(&terminated).unwrap(),
            json!({
                "type": "terminated",
                "error": { "code": -32012, "message": "Forbidden", "data": { "reason": "revoked" } }
            })
        );
        assert_eq!(terminated.webhook_id("x"), "msg_terminated_x");

        let verification = ControlEnvelope::Verification {
            challenge: "n0nce".to_owned(),
        };
        assert_eq!(
            serde_json::to_value(&verification).unwrap(),
            json!({ "type": "verification", "challenge": "n0nce" })
        );
        assert_eq!(verification.kind(), "verification");

        for envelope in [gap, terminated, verification] {
            let value = serde_json::to_value(&envelope).unwrap();
            assert_eq!(
                WebhookBody::from_value(value).unwrap(),
                WebhookBody::Control(envelope)
            );
        }

        let echo: VerificationResponse =
            serde_json::from_value(json!({ "challenge": "n0nce", "extra": true })).unwrap();
        assert_eq!(echo.challenge, "n0nce");
    }

    #[test]
    fn stream_params_parse_leniently() {
        let params: StreamEventsParams = serde_json::from_value(json!({
            "name": "email.received",
            "arguments": { "from": "*@example.com" },
            "cursor": null,
            "maxAge": 300
        }))
        .unwrap();
        assert_eq!(params.cursor, None);
        assert_eq!(params.max_age_ms, Some(300_000));
        let bare: StreamEventsParams = serde_json::from_value(json!({ "name": "x" })).unwrap();
        assert!(bare.arguments.is_empty());
    }

    #[test]
    fn stream_frames_match_the_sketch() {
        let id = json!(1);
        assert_eq!(
            StreamFrame::Active {
                cursor: Some("c1".to_owned()),
                truncated: false
            }
            .to_notification(&id),
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/events/active",
                "params": {
                    "cursor": "c1",
                    "truncated": false,
                    "_meta": { "io.modelcontextprotocol/subscriptionId": 1 }
                }
            })
        );
        assert_eq!(
            StreamFrame::Heartbeat { cursor: None }.to_notification(&json!("s-1"))["params"],
            json!({ "cursor": null, "_meta": { "io.modelcontextprotocol/subscriptionId": "s-1" } })
        );
        let event = StreamFrame::Event(EventOccurrence {
            event_id: "evt_001".to_owned(),
            name: "email.received".to_owned(),
            timestamp: "2026-02-19T15:30:00Z".to_owned(),
            data: json!({ "subject": "hi" }),
            cursor: Some("c2".to_owned()),
            meta: None,
        })
        .to_notification(&id);
        assert_eq!(event["method"], "notifications/events/event");
        assert_eq!(event["params"]["eventId"], "evt_001");
        assert_eq!(event["params"]["cursor"], "c2");
        assert_eq!(
            event["params"]["_meta"]["io.modelcontextprotocol/subscriptionId"],
            1
        );
        let terminated = StreamFrame::Terminated(JsonRpcErrorBody {
            code: -32012,
            message: "forbidden".to_owned(),
            data: Some(json!({ "reason": "revoked" })),
        })
        .to_notification(&id);
        assert_eq!(terminated["method"], "notifications/events/terminated");
        assert_eq!(terminated["params"]["error"]["code"], -32012);
        assert_eq!(
            stream_events_result(&id),
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "result": {
                    "resultType": "complete",
                    "_meta": { "io.modelcontextprotocol/subscriptionId": 1 }
                }
            })
        );
    }

    #[test]
    fn a_stream_notification_reads_back_as_its_frame() {
        let frames = [
            StreamFrame::Active {
                cursor: Some("c1".to_owned()),
                truncated: true,
            },
            StreamFrame::Heartbeat { cursor: None },
            StreamFrame::Event(EventOccurrence {
                event_id: "evt_001".to_owned(),
                name: "email.received".to_owned(),
                timestamp: "2026-02-19T15:30:00Z".to_owned(),
                data: json!({ "subject": "hi" }),
                cursor: Some("c2".to_owned()),
                meta: None,
            }),
            StreamFrame::Error(JsonRpcErrorBody {
                code: -32603,
                message: "busy".to_owned(),
                data: None,
            }),
            StreamFrame::Terminated(JsonRpcErrorBody {
                code: -32012,
                message: "forbidden".to_owned(),
                data: Some(json!({ "reason": "revoked" })),
            }),
        ];
        for frame in frames {
            let notification = frame.to_notification(&json!(7));
            assert_eq!(StreamFrame::from_notification(&notification), Some(frame));
        }

        // An occurrence's own `_meta` travels beside the stream's id.
        let annotated = StreamFrame::Event(EventOccurrence {
            event_id: "e3".to_owned(),
            name: "issues.opened".to_owned(),
            timestamp: "2026-02-19T15:30:00Z".to_owned(),
            data: json!({}),
            cursor: None,
            meta: Some(json!({ "dev.mcpg/source": "github" })),
        });
        let notification = annotated.to_notification(&json!(7));
        assert_eq!(notification["params"]["_meta"]["dev.mcpg/source"], "github");
        assert_eq!(
            notification["params"]["_meta"]
                [crate::v_2026_07_28::wire::subscriptions::META_KEY_SUBSCRIPTION_ID],
            7
        );
        assert_eq!(
            StreamFrame::from_notification(&notification),
            Some(annotated)
        );

        assert_eq!(
            StreamFrame::from_notification(
                &json!({ "jsonrpc": "2.0", "method": "notifications/tools/list_changed" })
            ),
            None
        );
        assert_eq!(
            StreamFrame::from_notification(&stream_events_result(&json!(7))),
            None
        );
    }
}
