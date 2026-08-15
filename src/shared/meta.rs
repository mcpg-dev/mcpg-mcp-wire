//! `_meta` reserved-prefix policing rules shared across protocol
//! revisions.
//!
//! The MCP `_meta` key grammar is a `prefix` followed by a `name`:
//!
//! - **prefix** (optional) — a series of labels separated by dots
//!   (`.`), terminated by a single slash (`/`). Each label starts with
//!   a letter and ends with a letter or digit.
//! - **name** — the segment after the slash (or the whole key when no
//!   prefix is present).
//!
//! **Reservation rule (security boundary).** A prefix is reserved for
//! MCP use *iff* its **second dot-separated label** is
//! `modelcontextprotocol` or `mcp`. The spec's examples:
//! `io.modelcontextprotocol/`, `dev.mcp/`, `org.modelcontextprotocol.api/`,
//! and `com.mcp.tools/` are all reserved; `com.example.mcp/` is **not**
//! reserved (its second label is `example`).
//!
//! A key carrying a reserved prefix is accepted from untrusted request
//! `_meta` **only** when it is one of the keys the spec actually
//! defines ([`KNOWN_RESERVED_KEYS`]). Any other reserved-prefix key is
//! rejected (fail-closed): this stops a client forging a spec-owned
//! key the gateway would otherwise route or trust — e.g. an
//! undefined `io.modelcontextprotocol/somethingPrivileged` or a
//! squatted `com.mcp.tools/...` — while still letting a compliant
//! client carry the real SEP-defined keys (the SEP-2575 identity
//! triple, W3C trace context, progress/log/cache hints). Legitimate
//! third-party namespaces such as `com.example.mcp/*` are not reserved
//! and pass unconditionally.
//!
//! Spec-defined bare keys (`progressToken`, `traceparent`,
//! `tracestate`) carry no prefix (no second label), so they are never
//! reserved and remain allowed.

use serde_json::Value;

use crate::shared::error::ProtocolError;

/// The `io.modelcontextprotocol/*` keys the spec / SEP set actually
/// defines. A reserved-prefix key is accepted from untrusted input
/// only if it appears here; any other reserved key is rejected so a
/// client cannot squat the reserved namespace. Mirrors the
/// `META_KEY_*` constants in
/// [`v_2026_07_28::wire::meta`](crate::v_2026_07_28::wire::meta).
const KNOWN_RESERVED_KEYS: &[&str] = &[
    // SEP-2575 stateless identity triple.
    "io.modelcontextprotocol/protocolVersion",
    "io.modelcontextprotocol/clientInfo",
    "io.modelcontextprotocol/clientCapabilities",
    // SEP-414 W3C/OTel trace context.
    "io.modelcontextprotocol/traceparent",
    "io.modelcontextprotocol/tracestate",
    // Per-request hints.
    "io.modelcontextprotocol/progressToken",
    "io.modelcontextprotocol/logLevel",
    "io.modelcontextprotocol/cacheToken",
    "io.modelcontextprotocol/related-task",
    "io.modelcontextprotocol/idempotencyKey",
    "io.modelcontextprotocol/preserveContext",
    // SEP-2322 MRTR resume channel.
    "io.modelcontextprotocol/requestState",
    "io.modelcontextprotocol/inputResponses",
    // SEP-2575 subscriptions/listen correlation.
    "io.modelcontextprotocol/subscriptionId",
    // Tasks extension (SEP-2663) model-immediate-response hint.
    "io.modelcontextprotocol/model-immediate-response",
];

/// Whether a `_meta` key's prefix is reserved for MCP use.
///
/// The prefix is everything before the first `/`; a key with no `/`
/// carries no prefix and is therefore never reserved. A prefix is
/// reserved exactly when its **second** dot-separated label
/// (case-insensitively) is `modelcontextprotocol` or `mcp`.
fn is_reserved_meta_key(key: &str) -> bool {
    let Some((prefix, _name)) = key.split_once('/') else {
        // No prefix → just a bare name → never reserved.
        return false;
    };
    let mut labels = prefix.split('.');
    // The reservation hinges on the SECOND label only.
    let (_first, second) = (labels.next(), labels.next());
    match second {
        Some(label) => {
            label.eq_ignore_ascii_case("modelcontextprotocol") || label.eq_ignore_ascii_case("mcp")
        }
        None => false,
    }
}

/// Validate `_meta` keys per the MCP reserved-prefix grammar. A key
/// whose prefix's second dot-label is `modelcontextprotocol` / `mcp`
/// is reserved: it is accepted only when it is a spec-defined key
/// ([`KNOWN_RESERVED_KEYS`]) and otherwise rejected from untrusted
/// input (fail-closed). Every non-reserved key — including legitimate
/// third-party namespaces such as `com.example.mcp/...` — is allowed.
pub fn validate_meta_keys(meta: &Value) -> Result<(), ProtocolError> {
    let Some(obj) = meta.as_object() else {
        return Ok(());
    };
    for key in obj.keys() {
        if is_reserved_meta_key(key) && !KNOWN_RESERVED_KEYS.contains(&key.as_str()) {
            return Err(ProtocolError::invalid_request(format!(
                "_meta key '{key}' uses a reserved MCP namespace but is not \
                 a spec-defined key (a prefix whose second label is \
                 `modelcontextprotocol` or `mcp` is reserved for MCP use)"
            )));
        }
        // Descend into nested `_meta` carried inside a structured
        // value so a forged reserved key cannot hide one level down.
        if let Some(nested) = obj.get(key).and_then(|v| v.get("_meta")) {
            validate_meta_keys(nested)?;
        }
    }
    Ok(())
}

/// Walk a request body and validate every `_meta` object in its
/// `params` subtree — the top-level `params._meta`, every content
/// block's `_meta`, and any nested `_meta` reachable from there.
pub fn validate_request_meta(body: &Value) -> Result<(), ProtocolError> {
    let Some(params) = body.get("params") else {
        return Ok(());
    };
    if let Some(meta) = params.get("_meta") {
        validate_meta_keys(meta)?;
    }
    // SEP content carries per-block `_meta`; descend so a reserved key
    // tucked inside a `content[]` / `messages[]` / `contents[]` entry
    // is policed with the same grammar as the top-level block.
    validate_content_meta(params)?;
    Ok(())
}

/// Validate `_meta` on content blocks reachable from a `params`
/// subtree. Walks the well-known content arrays (`content`,
/// `messages`, `contents`) and any nested `_meta` they carry.
fn validate_content_meta(params: &Value) -> Result<(), ProtocolError> {
    for field in ["content", "messages", "contents"] {
        if let Some(items) = params.get(field).and_then(Value::as_array) {
            for item in items {
                validate_value_meta(item)?;
            }
        }
    }
    Ok(())
}

/// Validate a value's own `_meta` and recurse into a nested `content`
/// / `resource` payload the value may carry (e.g. a message wrapping a
/// content block, or a content block wrapping an embedded resource).
fn validate_value_meta(value: &Value) -> Result<(), ProtocolError> {
    if let Some(meta) = value.get("_meta") {
        validate_meta_keys(meta)?;
    }
    // A `PromptMessage` wraps its block under `content` (an object,
    // not an array); descend so the block's own `_meta` is policed.
    match value.get("content") {
        Some(Value::Object(_)) => validate_value_meta(&value["content"])?,
        Some(Value::Array(items)) => {
            for item in items.iter() {
                validate_value_meta(item)?;
            }
        }
        _ => {}
    }
    if let Some(resource) = value.get("resource") {
        validate_value_meta(resource)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn accepts_known_top_level_keys() {
        let meta = json!({
            "progressToken": "p1",
            "traceparent": "00-...-...-00",
            "tracestate": "vendor=value"
        });
        validate_meta_keys(&meta).unwrap();
    }

    #[test]
    fn accepts_domain_qualified_keys() {
        // Third-party prefix whose second label is not a reserved one.
        let meta = json!({ "com.acme.audit/actor": "x" });
        validate_meta_keys(&meta).unwrap();
    }

    // --- Security boundary: forged reserved keys MUST be rejected. ---

    #[test]
    fn rejects_forged_unknown_io_modelcontextprotocol_key() {
        // The canonical attack: a client invents a reserved-namespace
        // key (not spec-defined) hoping downstream code trusts/routes
        // it.
        let meta = json!({ "io.modelcontextprotocol/somethingPrivileged": { "x": 1 } });
        let err = validate_meta_keys(&meta).unwrap_err();
        assert!(err.message().contains("reserved"));
    }

    #[test]
    fn rejects_dev_mcp_second_label() {
        // `dev.mcp/` — second label is `mcp`; not a spec-defined key.
        let meta = json!({ "dev.mcp/foo": "x" });
        let err = validate_meta_keys(&meta).unwrap_err();
        assert!(err.message().contains("reserved"));
    }

    #[test]
    fn rejects_com_mcp_tools_second_label() {
        // `com.mcp.tools/` — second label is `mcp`.
        let meta = json!({ "com.mcp.tools/run": "x" });
        let err = validate_meta_keys(&meta).unwrap_err();
        assert!(err.message().contains("reserved"));
    }

    #[test]
    fn rejects_org_modelcontextprotocol_api_second_label() {
        // `org.modelcontextprotocol.api/` — second label is
        // `modelcontextprotocol`.
        let meta = json!({ "org.modelcontextprotocol.api/x": "y" });
        let err = validate_meta_keys(&meta).unwrap_err();
        assert!(err.message().contains("reserved"));
    }

    #[test]
    fn reservation_is_case_insensitive() {
        // A reserved namespace recognized case-insensitively, but the
        // key itself is unknown ⇒ rejected.
        let meta = json!({ "io.ModelContextProtocol/notARealKey": "y" });
        assert!(validate_meta_keys(&meta).is_err());
        let meta = json!({ "dev.MCP/x": "y" });
        assert!(validate_meta_keys(&meta).is_err());
    }

    // --- Legitimate third-party keys MUST be allowed. ---

    #[test]
    fn allows_com_example_mcp_second_label_example() {
        // `com.example.mcp/` — the second label is `example`, NOT a
        // reserved one, so it is allowed even though a later label is
        // `mcp`.
        let meta = json!({ "com.example.mcp/foo": "x" });
        validate_meta_keys(&meta).unwrap();
    }

    #[test]
    fn allows_legacy_dotted_bare_keys() {
        // A key with no `/` carries no prefix → never reserved.
        validate_meta_keys(&json!({ "mcp.foo": "x" })).unwrap();
        validate_meta_keys(&json!({ "modelcontextprotocol.foo": "x" })).unwrap();
        validate_meta_keys(&json!({ "mcp": {} })).unwrap();
    }

    #[test]
    fn allows_spec_defined_reserved_keys() {
        // The real SEP-2575 identity triple + the other spec-defined
        // reserved keys are accepted — a compliant modern client MUST
        // carry these.
        validate_meta_keys(&json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientInfo": { "name": "c", "version": "1" },
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/traceparent": "00-a-b-01",
            "io.modelcontextprotocol/progressToken": "p",
            "io.modelcontextprotocol/logLevel": "info"
        }))
        .unwrap();
    }

    // --- Descent into nested `_meta`. ---

    #[test]
    fn descends_into_nested_meta() {
        let meta = json!({
            "com.acme.audit/actor": {
                "_meta": { "io.modelcontextprotocol/forged": 1 }
            }
        });
        let err = validate_meta_keys(&meta).unwrap_err();
        assert!(err.message().contains("reserved"));
    }

    #[test]
    fn validate_request_meta_skips_when_no_params() {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
        validate_request_meta(&body).unwrap();
    }

    #[test]
    fn validate_request_meta_walks_into_params_meta() {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/list",
            "params": { "_meta": { "io.modelcontextprotocol/forged": "x" } }
        });
        let err = validate_request_meta(&body).unwrap_err();
        assert!(err.message().contains("reserved"));
    }

    #[test]
    fn validate_request_meta_descends_into_content_blocks() {
        // A forged reserved key hidden inside a content block's `_meta`
        // must still be policed.
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "echo",
                "content": [
                    { "type": "text", "text": "hi",
                      "_meta": { "io.modelcontextprotocol/forged": "x" } }
                ]
            }
        });
        let err = validate_request_meta(&body).unwrap_err();
        assert!(err.message().contains("reserved"));
    }

    #[test]
    fn validate_request_meta_descends_into_embedded_resource() {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "prompts/get",
            "params": {
                "name": "p",
                "messages": [
                    { "role": "user", "content": {
                        "type": "resource",
                        "resource": {
                            "uri": "x", "text": "y",
                            "_meta": { "dev.mcp/forged": 1 }
                        }
                    }}
                ]
            }
        });
        let err = validate_request_meta(&body).unwrap_err();
        assert!(err.message().contains("reserved"));
    }
}
