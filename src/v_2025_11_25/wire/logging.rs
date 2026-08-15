//! Logging wire types for MCP revision `2025-11-25`.
//!
//! Two surfaces:
//!
//! - [`LoggingSetLevelParams`] — body of a `logging/setLevel`
//!   request. Client tells the server the minimum severity it wants
//!   to receive over `notifications/message`.
//! - [`LoggingMessageNotification`] (+ [`LoggingMessageParams`]) —
//!   server-pushed structured-log event.
//!
//! [`LoggingLevel`] is the syslog-ordered severity enum used by both
//! surfaces.
//!
//! `DRAFT-2026-v1` deprecates this whole feature in favour of stderr
//! / OpenTelemetry; the per-request `_meta.logLevel` key replaces
//! `logging/setLevel` for any client that still wants per-call log
//! filtering. The 2025-11-25 shapes stay intact during the
//! deprecation window (see SEP-2577).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Parameters for `logging/setLevel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSetLevelParams {
    pub level: LoggingLevel,
}

/// Syslog-ordered severity levels (RFC 5424 ordering).
///
/// `PartialOrd` / `Ord` derives reflect the syslog ordinal so the
/// runtime can filter `notifications/message` events by comparing the
/// session's set level against the event's level — `Debug < Info < …
/// < Emergency`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum LoggingLevel {
    Debug,
    Info,
    Notice,
    Warning,
    Error,
    Critical,
    Alert,
    Emergency,
}

/// JSON-RPC notification for `notifications/message`. Carries one
/// structured log event from server to client over SSE (or stdio's
/// shared bidirectional channel).
#[derive(Debug, Clone, Serialize)]
pub struct LoggingMessageNotification {
    pub jsonrpc: &'static str,
    pub method: &'static str,
    pub params: LoggingMessageParams,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoggingMessageParams {
    pub level: LoggingLevel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logger: Option<String>,
    pub data: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn logging_level_serializes_as_snake_case() {
        assert_eq!(serde_json::to_value(LoggingLevel::Debug).unwrap(), "debug");
        assert_eq!(serde_json::to_value(LoggingLevel::Info).unwrap(), "info");
        assert_eq!(
            serde_json::to_value(LoggingLevel::Notice).unwrap(),
            "notice"
        );
        assert_eq!(
            serde_json::to_value(LoggingLevel::Emergency).unwrap(),
            "emergency"
        );
    }

    #[test]
    fn logging_level_syslog_ordering() {
        // Comparison must follow syslog severity, not enum declaration
        // order (the two happen to match here, but the test pins it).
        assert!(LoggingLevel::Debug < LoggingLevel::Info);
        assert!(LoggingLevel::Info < LoggingLevel::Warning);
        assert!(LoggingLevel::Warning < LoggingLevel::Error);
        assert!(LoggingLevel::Error < LoggingLevel::Critical);
        assert!(LoggingLevel::Critical < LoggingLevel::Alert);
        assert!(LoggingLevel::Alert < LoggingLevel::Emergency);
    }

    #[test]
    fn logging_set_level_params_round_trip() {
        let p = LoggingSetLevelParams {
            level: LoggingLevel::Warning,
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["level"], "warning");
        let back: LoggingSetLevelParams = serde_json::from_value(v).unwrap();
        assert_eq!(back.level, LoggingLevel::Warning);
    }

    #[test]
    fn logging_message_notification_carries_data() {
        let notif = LoggingMessageNotification {
            jsonrpc: "2.0",
            method: "notifications/message",
            params: LoggingMessageParams {
                level: LoggingLevel::Error,
                logger: Some("db".to_owned()),
                data: json!({ "error": "connection refused" }),
            },
        };
        let v = serde_json::to_value(&notif).unwrap();
        assert_eq!(v["jsonrpc"], "2.0");
        assert_eq!(v["method"], "notifications/message");
        assert_eq!(v["params"]["level"], "error");
        assert_eq!(v["params"]["logger"], "db");
        assert_eq!(v["params"]["data"]["error"], "connection refused");
    }

    #[test]
    fn logging_message_params_omits_logger_when_absent() {
        let v = serde_json::to_value(&LoggingMessageParams {
            level: LoggingLevel::Info,
            logger: None,
            data: json!("hello"),
        })
        .unwrap();
        assert!(v.get("logger").is_none());
        assert_eq!(v["data"], "hello");
    }
}
