//! SEP-2575 per-request `_meta` identity triple.
//!
//! The stateless wire has no `initialize` handshake, so every id-bearing
//! request carries the client identity it would otherwise have negotiated:
//! `params._meta.io.modelcontextprotocol/{protocolVersion, clientInfo,
//! clientCapabilities}`. `server/discover` requires it unconditionally;
//! the rest of the method surface requires it only when the operator
//! opts in via `server.enforce_modern_request_meta`.

use serde_json::Value;

use crate::shared::messages::TransportRejection;

/// Return the first SEP-2575 per-request `_meta` identity key missing
/// from a request body, or `None` when all three are present. The
/// triple is `params._meta.io.modelcontextprotocol/{protocolVersion,
/// clientInfo, clientCapabilities}` — the stateless replacement for
/// the `initialize` handshake.
pub fn missing_request_meta_key(body: &Value) -> Option<&'static str> {
    let params = body.as_object().and_then(|obj| obj.get("params"));
    let meta = params.and_then(|p| p.get("_meta"));
    match meta {
        None => Some("params._meta"),
        Some(m) => {
            let mo = m.as_object();
            if mo
                .and_then(|o| o.get("io.modelcontextprotocol/protocolVersion"))
                .is_none()
            {
                Some("io.modelcontextprotocol/protocolVersion")
            } else if mo
                .and_then(|o| o.get("io.modelcontextprotocol/clientInfo"))
                .is_none()
            {
                Some("io.modelcontextprotocol/clientInfo")
            } else if mo
                .and_then(|o| o.get("io.modelcontextprotocol/clientCapabilities"))
                .is_none()
            {
                Some("io.modelcontextprotocol/clientCapabilities")
            } else {
                None
            }
        }
    }
}

/// Build the rejection for a missing SEP-2575 `_meta` identity key.
pub fn missing_meta_rejection(missing: &str, jsonrpc_id: Option<Value>) -> TransportRejection {
    TransportRejection {
        status: 200,
        error_code: -32602,
        message: format!(
            "SEP-2575 stateless request is missing required `_meta` \
             key: `{missing}`. Modern requests MUST carry \
             `params._meta.io.modelcontextprotocol/{{protocolVersion, \
             clientInfo, clientCapabilities}}`."
        ),
        data: None,
        jsonrpc_id,
    }
}

/// TOOLS-09 opt-in enforcement. When `server.enforce_modern_request_meta`
/// is on, the SEP-2575 per-request `_meta` identity triple is required
/// on EVERY id-bearing modern method (not just `server/discover`).
/// Notifications (id-less bodies) and `server/discover` (already
/// enforced unconditionally in `validate_transport_headers`) are
/// skipped here. Returns the rejection to send, or `None` to accept.
pub fn enforce_request_meta_triple(body: &Value) -> Option<TransportRejection> {
    let obj = body.as_object()?;
    let body_id = obj.get("id").cloned();
    body_id.as_ref()?;
    let method = obj.get("method").and_then(Value::as_str).unwrap_or("");
    if method == "server/discover" {
        // Already enforced unconditionally by the handler.
        return None;
    }
    missing_request_meta_key(body).map(|missing| missing_meta_rejection(missing, body_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enforce_request_meta_triple_accepts_full_triple() {
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list",
            "params": { "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                "io.modelcontextprotocol/clientInfo": { "name": "c", "version": "1" },
                "io.modelcontextprotocol/clientCapabilities": {}
            }}
        });
        assert!(enforce_request_meta_triple(&body).is_none());
    }

    #[test]
    fn enforce_request_meta_triple_rejects_missing_key_on_id_bearing_method() {
        // TOOLS-09: with the flag on, a non-discover id-bearing method
        // missing the triple is rejected.
        let body = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "x", "_meta": {
                "io.modelcontextprotocol/protocolVersion": "2026-07-28"
            }}
        });
        let rej = enforce_request_meta_triple(&body).expect("rejected");
        assert_eq!(rej.error_code, -32602);
        assert!(rej.message.contains("clientInfo"));
    }

    #[test]
    fn enforce_request_meta_triple_skips_notifications_and_discover() {
        // Notifications (no id) are not subject to the contract.
        let notif = serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/cancelled", "params": {}
        });
        assert!(enforce_request_meta_triple(&notif).is_none());
        // server/discover is enforced unconditionally elsewhere, so the
        // opt-in path skips it (no double-rejection).
        let discover = serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": {}
        });
        assert!(enforce_request_meta_triple(&discover).is_none());
    }
}
