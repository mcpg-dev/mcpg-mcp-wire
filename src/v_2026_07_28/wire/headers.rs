//! SEP-2243 mirrored request headers for `2026-07-28`.
//!
//! One module for the whole contract: the header names, the
//! `=?base64?…?=` value codec, the server-side validation of `Mcp-Name`
//! and `Mcp-Param-{Name}` against the body, and the `tools/call`
//! param→header promotion the gateway performs when it relays to a
//! header-routing intermediary.
//!
//! The two halves used to sit in different files — the codec here in
//! `wire/`, the validation and promotion inside the server `handler` —
//! which is why the federation *client* had to reach into a server
//! handler to call `promote_param_headers`. Encoding and decoding the
//! same header belong together.

use http::HeaderMap;
use serde_json::Value;

use crate::shared::error::HEADER_MISMATCH_CODE;
use crate::shared::messages::TransportRejection;

/// HTTP header that carries the JSON-RPC `method` of the body. SEP-2243.
///
/// Server-side: the modern HTTP transport validates that this header
/// matches the body's `method` field and rejects with HTTP 400 on
/// mismatch.
pub const METHOD_HEADER: &str = "mcp-method";

/// HTTP header that carries the resource / tool / prompt name when
/// the body's params reference one. SEP-2243.
///
/// Used together with [`METHOD_HEADER`] so policy / rate-limit
/// middleware can inspect the target name without parsing the body.
pub const NAME_HEADER: &str = "mcp-name";

/// HTTP header *prefix* for per-parameter routing hints. SEP-2243.
///
/// Full header name is built as `mcp-param-{name}` (e.g.
/// `mcp-param-cursor` for the pagination cursor). The transport
/// records only the headers a deployment opts into so we don't
/// accidentally surface large param values via HTTP.
pub const PARAM_HEADER_PREFIX: &str = "mcp-param-";

/// JSON-Schema extension keyword that designates a `tools/call`
/// argument for promotion into an `Mcp-Param-{Name}` header. SEP-2243.
pub const X_MCP_HEADER_KEYWORD: &str = "x-mcp-header";

// ---------------------------------------------------------------------------
// SEP-2243 header-value encoding (the `=?base64?…?=` sentinel).
// ---------------------------------------------------------------------------

/// Encode a value into a SEP-2243 mirrored-header value, wrapping it in
/// the `=?base64?…?=` sentinel when it is not safe as a plain ASCII
/// header (empty, non-ASCII, control characters, leading/trailing
/// whitespace, or a value that itself looks like the sentinel). The
/// inverse of [`decode_header_value`].
///
/// Used by the modern server when mirroring params into routing headers
/// and by the federation client when emitting `Mcp-Name` /
/// `Mcp-Param-{Name}` on a `2026-07-28` upstream POST.
pub fn encode_header_value(value: &str) -> String {
    use base64::Engine as _;
    let needs_encoding = value.is_empty()
        || value.starts_with("=?base64?")
        || value.starts_with(char::is_whitespace)
        || value.ends_with(char::is_whitespace)
        || value.bytes().any(|b| !(0x20..=0x7e).contains(&b));
    if needs_encoding {
        let encoded = base64::engine::general_purpose::STANDARD.encode(value.as_bytes());
        format!("=?base64?{encoded}?=")
    } else {
        value.to_owned()
    }
}

/// Decode a SEP-2243 mirrored-header value to its source string.
///
/// Header values that cannot be carried as plain ASCII (non-ASCII,
/// control characters, or leading/trailing whitespace) are wrapped in
/// the sentinel `=?base64?{Base64EncodedValue}?=`. A value that is not
/// wrapped is returned verbatim. The RFC-2047-style four-token form
/// `=?{charset}?B?{Base64EncodedValue}?=` is also accepted (the
/// charset is honored only when UTF-8; otherwise the bytes are decoded
/// lossily) so intermediaries that re-encode in the classic encoded-word
/// shape still validate.
///
/// Returns `None` when a sentinel-wrapped value is malformed (bad
/// base64 or an unsupported token shape) so the caller can reject the
/// request rather than compare against garbage.
pub fn decode_header_value(raw: &str) -> Option<String> {
    use base64::Engine as _;

    // Two-token canonical form: `=?base64?{value}?=`.
    if let Some(inner) = raw
        .strip_prefix("=?base64?")
        .and_then(|s| s.strip_suffix("?="))
    {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(inner)
            .ok()?;
        return Some(String::from_utf8_lossy(&bytes).into_owned());
    }

    // RFC-2047 encoded-word form: `=?{charset}?B?{value}?=` (base64
    // encoding marker `B`/`b`; the quoted-printable `Q` marker is not
    // part of the SEP contract and is rejected).
    if let Some(body) = raw.strip_prefix("=?").and_then(|s| s.strip_suffix("?=")) {
        let mut parts = body.splitn(3, '?');
        let _charset = parts.next()?;
        let encoding = parts.next()?;
        let payload = parts.next()?;
        if !encoding.eq_ignore_ascii_case("B") {
            return None;
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(payload)
            .ok()?;
        return Some(String::from_utf8_lossy(&bytes).into_owned());
    }

    Some(raw.to_owned())
}

#[cfg(test)]
mod header_value_tests {
    use super::decode_header_value;

    #[test]
    fn plain_ascii_passes_through() {
        assert_eq!(
            decode_header_value("get_weather").as_deref(),
            Some("get_weather")
        );
        assert_eq!(
            decode_header_value("file:///projects/myapp/config.json").as_deref(),
            Some("file:///projects/myapp/config.json"),
        );
    }

    #[test]
    fn base64_sentinel_decodes_non_ascii() {
        // "Hello, 世界" from the spec's encoding-examples table.
        assert_eq!(
            decode_header_value("=?base64?SGVsbG8sIOS4lueVjA==?=").as_deref(),
            Some("Hello, 世界"),
        );
    }

    #[test]
    fn base64_sentinel_preserves_padding_whitespace() {
        // " padded " — leading/trailing spaces force sentinel encoding.
        assert_eq!(
            decode_header_value("=?base64?IHBhZGRlZCA=?=").as_deref(),
            Some(" padded "),
        );
    }

    #[test]
    fn rfc2047_four_token_form_decodes() {
        assert_eq!(
            decode_header_value("=?utf-8?B?SGVsbG8sIOS4lueVjA==?=").as_deref(),
            Some("Hello, 世界"),
        );
    }

    #[test]
    fn malformed_sentinel_is_rejected() {
        assert!(decode_header_value("=?base64?not valid base64!!?=").is_none());
        // Quoted-printable encoding marker is outside the SEP contract.
        assert!(decode_header_value("=?utf-8?Q?Hello?=").is_none());
    }
}

/// Build the SEP-2243 `HeaderMismatch` rejection (HTTP 400 +
/// JSON-RPC `-32020`).
pub fn header_mismatch(message: &str, jsonrpc_id: Option<Value>) -> TransportRejection {
    TransportRejection {
        status: 400,
        error_code: HEADER_MISMATCH_CODE,
        message: message.to_owned(),
        data: None,
        jsonrpc_id,
    }
}

/// Validate the SEP-2243 `Mcp-Name` header against the body value
/// at `source_field` (`name` or `uri`). The header is required for
/// the methods that carry an identifying param; the value is
/// sentinel-decoded before comparison.
pub fn validate_name_header(
    headers: &HeaderMap,
    body: &Value,
    source_field: &str,
    jsonrpc_id: Option<Value>,
) -> Result<(), TransportRejection> {
    let body_value = body
        .as_object()
        .and_then(|obj| obj.get("params"))
        .and_then(|p| p.get(source_field))
        .and_then(Value::as_str);

    let raw_header = headers.get(NAME_HEADER).and_then(|v| v.to_str().ok());

    let Some(raw_header) = raw_header else {
        return Err(header_mismatch(
            &format!(
                "SEP-2243 requires the `Mcp-Name` header (mirroring \
                 `params.{source_field}`) on this method; it is absent"
            ),
            jsonrpc_id,
        ));
    };

    let Some(decoded) = decode_header_value(raw_header) else {
        return Err(header_mismatch(
            "`Mcp-Name` header carries a malformed `=?base64?…?=` value",
            jsonrpc_id,
        ));
    };

    match body_value {
        Some(body_value) if body_value == decoded => Ok(()),
        Some(body_value) => Err(header_mismatch(
            &format!(
                "Mcp-Name header (`{decoded}`) does not match body \
                 `params.{source_field}` (`{body_value}`); SEP-2243 \
                 requires the two to agree"
            ),
            jsonrpc_id,
        )),
        None => Err(header_mismatch(
            &format!(
                "Mcp-Name header is present but the body carries no \
                 `params.{source_field}` to validate it against"
            ),
            jsonrpc_id,
        )),
    }
}

/// Validate every recognized `Mcp-Param-{Name}` header against the
/// matching `params.arguments.{name}` value in the body (SEP-2243).
/// Header values are sentinel-decoded; integer / boolean body
/// values are compared by their canonical string form. A header
/// that disagrees with the body is rejected (`-32020`).
pub fn validate_param_headers(
    headers: &HeaderMap,
    body: &Value,
    jsonrpc_id: Option<Value>,
) -> Result<(), TransportRejection> {
    let arguments = body
        .as_object()
        .and_then(|obj| obj.get("params"))
        .and_then(|p| p.get("arguments"));

    for (name, value) in headers.iter() {
        let Some(param) = name.as_str().strip_prefix(PARAM_HEADER_PREFIX) else {
            continue;
        };
        let Some(raw) = value.to_str().ok() else {
            return Err(header_mismatch(
                &format!("`Mcp-Param-{param}` header carries non-ASCII bytes"),
                jsonrpc_id,
            ));
        };
        let Some(decoded) = decode_header_value(raw) else {
            return Err(header_mismatch(
                &format!("`Mcp-Param-{param}` header carries a malformed `=?base64?…?=` value"),
                jsonrpc_id,
            ));
        };
        let body_value = arguments.and_then(|a| a.get(param));
        match body_value.map(json_scalar_to_header_string) {
            Some(Some(body_str)) if body_str == decoded => {}
            _ => {
                return Err(header_mismatch(
                    &format!(
                        "Mcp-Param-{param} header (`{decoded}`) does not match \
                         body `params.arguments.{param}`; SEP-2243 requires \
                         the two to agree"
                    ),
                    jsonrpc_id,
                ));
            }
        }
    }
    Ok(())
}

/// The body param a method's `Mcp-Name` header mirrors, or `None` for
/// methods that do not carry the header (SEP-2243).
pub fn name_source_field(method: &str) -> Option<&'static str> {
    match method {
        "tools/call" | "prompts/get" => Some("name"),
        "resources/read" => Some("uri"),
        _ => None,
    }
}

/// Canonical header-string form of a scalar JSON value per the
/// SEP-2243 type-conversion rules (`string` as-is, `integer` as a
/// decimal string, `boolean` as lowercase `true`/`false`). Returns
/// `None` for non-scalar or `null` values (which are never promoted).
fn json_scalar_to_header_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) if n.is_i64() || n.is_u64() => Some(n.to_string()),
        _ => None,
    }
}

/// SEP-2243 server-side param→header promotion. Given a tool's
/// `inputSchema` and a `tools/call` arguments object, produce the
/// `(Mcp-Param-{Name}, value)` header pairs the gateway emits when it
/// relays a call to a header-routing intermediary.
///
/// Each promotion is gated on the schema constraints SEP-2243 places
/// on `x-mcp-header`: a non-empty HTTP-token name, a primitive
/// (`string`/`integer`/`boolean`, never `number`) annotated property
/// statically reachable through `properties` chains only, and a
/// case-insensitively unique header name. A property that violates any
/// constraint is **excluded** (its promotion is dropped) rather than
/// failing the whole call — matching the SEP's per-tool exclusion
/// posture. Values that need it are wrapped in the `=?base64?…?=`
/// sentinel.
pub fn promote_param_headers(input_schema: &Value, arguments: &Value) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut seen_lower: std::collections::HashSet<String> = std::collections::HashSet::new();
    collect_promotions(input_schema, arguments, &mut out, &mut seen_lower);
    out
}

/// Walk a schema's `properties` chain (and only that chain) collecting
/// valid `x-mcp-header` promotions. Recurses into nested object
/// `properties` but never through `items` / composition / `$ref`,
/// mirroring the SEP "statically reachable" rule.
fn collect_promotions(
    schema: &Value,
    instance: &Value,
    out: &mut Vec<(String, String)>,
    seen_lower: &mut std::collections::HashSet<String>,
) {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (prop_name, prop_schema) in properties {
        let instance_value = instance.get(prop_name);

        if let Some(header_name) = prop_schema
            .get(X_MCP_HEADER_KEYWORD)
            .and_then(Value::as_str)
            && is_valid_header_token(header_name)
            && is_promotable_primitive(prop_schema)
        {
            let lower = header_name.to_ascii_lowercase();
            // Drop (exclude) on duplicate header name rather than fail.
            if seen_lower.insert(lower)
                && let Some(value) = instance_value.and_then(json_scalar_to_header_string)
            {
                out.push((
                    format!("{PARAM_HEADER_PREFIX}{}", header_name.to_ascii_lowercase()),
                    encode_header_value(&value),
                ));
            }
        }

        // Statically reachable nested object properties only.
        if prop_schema
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "object")
            && let Some(nested_instance) = instance_value
        {
            collect_promotions(prop_schema, nested_instance, out, seen_lower);
        }
    }
}

/// Whether a property schema is a primitive type eligible for header
/// promotion (`string`/`integer`/`boolean`). `number` is explicitly
/// excluded by SEP-2243.
fn is_promotable_primitive(prop_schema: &Value) -> bool {
    matches!(
        prop_schema.get("type").and_then(Value::as_str),
        Some("string") | Some("integer") | Some("boolean")
    )
}

/// Whether `name` is a valid HTTP field-name token (`1*tchar`,
/// RFC 9110 §5.1) — non-empty, no control / separator characters.
fn is_valid_header_token(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| {
            matches!(b,
                b'!' | b'#' | b'$' | b'%' | b'&' | b'\'' | b'*' | b'+' | b'-' | b'.'
                | b'^' | b'_' | b'`' | b'|' | b'~'
                | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promote_param_headers_emits_annotated_string() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "region": { "type": "string", "x-mcp-header": "Region" },
                "query": { "type": "string" },
            }
        });
        let args = serde_json::json!({ "region": "us-west1", "query": "SELECT 1" });
        let headers = promote_param_headers(&schema, &args);
        assert_eq!(
            headers,
            vec![("mcp-param-region".to_owned(), "us-west1".to_owned())]
        );
    }

    #[test]
    fn promote_param_headers_encodes_non_ascii() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "greeting": { "type": "string", "x-mcp-header": "Greeting" },
            }
        });
        let args = serde_json::json!({ "greeting": "Hello, 世界" });
        let headers = promote_param_headers(&schema, &args);
        assert_eq!(
            headers,
            vec![(
                "mcp-param-greeting".to_owned(),
                "=?base64?SGVsbG8sIOS4lueVjA==?=".to_owned()
            )]
        );
    }

    #[test]
    fn promote_param_headers_excludes_constraint_violations() {
        // `number` is forbidden; an empty header name is forbidden; a
        // duplicate header name is dropped; an annotation under `items`
        // (array) is not statically reachable. Each violator is
        // EXCLUDED, the whole promotion does not fail, and the one
        // valid string promotion survives.
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "ok": { "type": "string", "x-mcp-header": "Ok" },
                "ratio": { "type": "number", "x-mcp-header": "Ratio" },
                "empty": { "type": "string", "x-mcp-header": "" },
                "dup": { "type": "string", "x-mcp-header": "Ok" },
                "list": {
                    "type": "array",
                    "items": { "type": "string", "x-mcp-header": "Nested" }
                }
            }
        });
        let args = serde_json::json!({
            "ok": "yes",
            "ratio": 1.5,
            "empty": "x",
            "dup": "z",
            "list": ["a"],
        });
        let headers = promote_param_headers(&schema, &args);
        assert_eq!(headers, vec![("mcp-param-ok".to_owned(), "yes".to_owned())]);
    }
}
