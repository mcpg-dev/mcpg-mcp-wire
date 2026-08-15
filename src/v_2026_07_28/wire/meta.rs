//! Modern per-request `_meta` namespace types for MCP revision `2026-07-28`.
//!
//! The stateless modern revision (SEP-2575 + SEP-2567) cannot rely
//! on session-scoped capability negotiation for per-request hints —
//! every request stands on its own. The spec consolidates these
//! hints under the reserved
//! [`META_NAMESPACE`](crate::v_2026_07_28::wire::meta::META_NAMESPACE)
//! prefix, so a single `_meta` object on any request body can carry:
//!
//! - trace context propagation (W3C `traceparent` / `tracestate`),
//! - progress tracking (`progressToken`),
//! - per-request log level (replaces the deprecated `logging/setLevel`
//!   subscribe-per-session model — SEP-2577),
//! - SEP-2549 cache validation tokens,
//! - SEP-2567 context-preservation hints (the client tells the
//!   server "associate this request with the result of request X"),
//! - per-request idempotency keys,
//! - task correlation (`related-task`).
//!
//! MRTR's `requestState` and `inputResponses` keys live in
//! `v_2026_07_28/wire/mrtr.rs`; the tasks-extension keys
//! ship under `v_2026_07_28/extensions/tasks/wire.rs`.
//!
//! Dispatch reads the individual keys it needs directly off the raw
//! `_meta` object by the `META_KEY_*` constants below. The body
//! parser in `shared::meta` enforces the `io.modelcontextprotocol/`
//! namespace rules before any handler inspects these keys.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Namespace + well-known key constants.
// ---------------------------------------------------------------------------

/// Reverse-DNS namespace reserved for spec-defined `_meta` keys.
/// Vendor extensions MUST use a different prefix.
pub const META_NAMESPACE: &str = "io.modelcontextprotocol";

/// SEP-2575 per-request protocol version. The stateless wire removes
/// the `initialize` handshake, so every request self-identifies the
/// revision the client is speaking via this key.
pub const META_KEY_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";

/// SEP-2575 per-request client identity (`{name, title?, version, …}`),
/// replacing the `initialize`-time `clientInfo`.
pub const META_KEY_CLIENT_INFO: &str = "io.modelcontextprotocol/clientInfo";

/// SEP-2575 per-request client capability advertisement, replacing the
/// `initialize`-time `clientCapabilities`.
pub const META_KEY_CLIENT_CAPABILITIES: &str = "io.modelcontextprotocol/clientCapabilities";

/// W3C trace-parent header propagation key.
pub const META_KEY_TRACEPARENT: &str = "io.modelcontextprotocol/traceparent";

/// Progress-token key — the value the server should echo on
/// `notifications/progress` events.
pub const META_KEY_PROGRESS_TOKEN: &str = "io.modelcontextprotocol/progressToken";

/// Per-request log level. Replaces the legacy
/// `logging/setLevel` subscribe-per-session model.
pub const META_KEY_LOG_LEVEL: &str = "io.modelcontextprotocol/logLevel";

/// SEP-2549 cache-validation token. Carried by clients on subsequent
/// `*/list` requests so the server can reply 304-style with the
/// cached snapshot.
pub const META_KEY_CACHE_TOKEN: &str = "io.modelcontextprotocol/cacheToken";

/// Per-request idempotency key. When set, the server SHOULD return
/// the cached response for a prior request that carried the same
/// key.
pub const META_KEY_IDEMPOTENCY: &str = "io.modelcontextprotocol/idempotencyKey";

/// SEP-2567 context-preservation hint. The value points at a prior
/// request whose context this one should re-use.
pub const META_KEY_PRESERVE_CONTEXT: &str = "io.modelcontextprotocol/preserveContext";

// ---------------------------------------------------------------------------
// Typed payload shapes for keys with non-scalar values.
// ---------------------------------------------------------------------------

/// Severity hint for [`RequestMeta::log_level`]. Same eight RFC-5424
/// levels the legacy `logging/setLevel` accepted, ordered so a more
/// permissive level (lower in the table) compares less-than a less
/// permissive one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

impl LogLevel {
    /// Parse one of the eight RFC-5424 / MCP logging vocabulary
    /// strings (`debug` … `emergency`) into a [`LogLevel`]. Case is
    /// significant per the spec (all-lowercase); returns `None` for
    /// any other token. Used to map a pipeline log step's configured
    /// `level` string onto the typed severity so it can be compared
    /// against a per-request `io.modelcontextprotocol/logLevel`
    /// threshold.
    pub fn parse_str(s: &str) -> Option<Self> {
        Some(match s {
            "debug" => Self::Debug,
            "info" => Self::Info,
            "notice" => Self::Notice,
            "warning" => Self::Warning,
            "error" => Self::Error,
            "critical" => Self::Critical,
            "alert" => Self::Alert,
            "emergency" => Self::Emergency,
            _ => return None,
        })
    }

    /// SEP-2575 per-request emission gate. A `notifications/message`
    /// at severity `self` is emitted iff `self >= minimum` — i.e. the
    /// message's level is at or above the client-requested floor. A
    /// message whose level string is unrecognised is treated as
    /// emitted (the configured level vocabulary is validated
    /// elsewhere; an unknown token is not silently dropped here).
    pub fn permits(self, minimum: LogLevel) -> bool {
        self >= minimum
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_ordering_is_rfc_5424() {
        assert!(LogLevel::Debug < LogLevel::Info);
        assert!(LogLevel::Info < LogLevel::Notice);
        assert!(LogLevel::Notice < LogLevel::Warning);
        assert!(LogLevel::Warning < LogLevel::Error);
        assert!(LogLevel::Error < LogLevel::Critical);
        assert!(LogLevel::Critical < LogLevel::Alert);
        assert!(LogLevel::Alert < LogLevel::Emergency);
    }

    #[test]
    fn well_known_constants_match_spec_strings() {
        // Compile-time pin: if a constant drifts off the spec
        // string, MCPG silently fails to parse the key. Regression
        // guard.
        assert_eq!(META_NAMESPACE, "io.modelcontextprotocol");
        assert_eq!(
            META_KEY_PROTOCOL_VERSION,
            "io.modelcontextprotocol/protocolVersion"
        );
        assert_eq!(META_KEY_CLIENT_INFO, "io.modelcontextprotocol/clientInfo");
        assert_eq!(
            META_KEY_CLIENT_CAPABILITIES,
            "io.modelcontextprotocol/clientCapabilities"
        );
        assert_eq!(META_KEY_TRACEPARENT, "io.modelcontextprotocol/traceparent");
        assert_eq!(
            META_KEY_PROGRESS_TOKEN,
            "io.modelcontextprotocol/progressToken"
        );
        assert_eq!(META_KEY_LOG_LEVEL, "io.modelcontextprotocol/logLevel");
        assert_eq!(META_KEY_CACHE_TOKEN, "io.modelcontextprotocol/cacheToken");
        assert_eq!(
            META_KEY_IDEMPOTENCY,
            "io.modelcontextprotocol/idempotencyKey"
        );
        assert_eq!(
            META_KEY_PRESERVE_CONTEXT,
            "io.modelcontextprotocol/preserveContext"
        );
    }
}
