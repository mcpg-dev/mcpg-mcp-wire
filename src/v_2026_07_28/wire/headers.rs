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

use http::{HeaderMap, HeaderName};
use serde_json::Value;

use crate::shared::error::HEADER_MISMATCH_CODE;
use crate::shared::messages::TransportRejection;

/// HTTP header that carries the JSON-RPC `method` of the body. SEP-2243.
///
/// Server-side: the HTTP transport validates that this header
/// matches the body's `method` field and rejects with HTTP 400 on
/// mismatch.
pub const METHOD_HEADER: &str = "mcp-method";

/// HTTP header that carries the resource / tool / prompt name when
/// the body's params reference one. SEP-2243.
///
/// Used together with [`METHOD_HEADER`] so policy / rate-limit
/// middleware can inspect the target name without parsing the body.
pub const NAME_HEADER: &str = "mcp-name";

/// [`NAME_HEADER`] as a header name: a lookup by it skips parsing the name.
const NAME_HEADER_NAME: HeaderName = HeaderName::from_static(NAME_HEADER);

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
/// Used by the server when mirroring params into routing headers and
/// by the federation client when emitting `Mcp-Name` /
/// `Mcp-Param-{Name}` on an upstream POST.
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

    let raw_header = headers.get(&NAME_HEADER_NAME).and_then(|v| v.to_str().ok());

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
    annotated_params(input_schema, arguments)
        .into_iter()
        .filter_map(|param| {
            let value = param.value?.header_string();
            Some((
                format!("{PARAM_HEADER_PREFIX}{}", param.header),
                encode_header_value(&value),
            ))
        })
        .collect()
}

/// One valid `x-mcp-header` annotation of a schema, with the value the
/// arguments give it.
struct AnnotatedParam {
    /// The header name after `Mcp-Param-`, lower case.
    header: String,
    /// The argument; `None` when the arguments carry no value (or `null`)
    /// for it.
    value: Option<ParamValue>,
}

/// The value of an annotated argument, as a header is compared with it.
enum ParamValue {
    /// A string or boolean argument, compared as its header string.
    Text(String),
    /// A whole-number argument of an `integer` property, compared by number.
    Integer(i128),
}

impl ParamValue {
    /// The value of `value` for a property of schema `prop_schema`.
    fn of(prop_schema: &Value, value: &Value) -> Option<Self> {
        if prop_schema.get("type").and_then(Value::as_str) == Some("integer")
            && let Some(number) = value.as_number()
        {
            return integral(number).map(Self::Integer);
        }
        json_scalar_to_header_string(value).map(Self::Text)
    }

    /// The header string a client sends for this value.
    fn header_string(self) -> String {
        match self {
            Self::Text(text) => text,
            Self::Integer(n) => n.to_string(),
        }
    }

    /// Whether the decoded header value `decoded` carries this value. An
    /// integer compares by number, so `42` and `42.0` are the same.
    fn matches(&self, decoded: &str) -> bool {
        match self {
            Self::Text(text) => text == decoded,
            Self::Integer(n) => decimal_integer(decoded) == Some(*n),
        }
    }
}

/// Largest magnitude a JSON float holds as an exact whole number (2^53 - 1).
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// The whole number `number` holds: any integer, or a float with no
/// fraction in the range a float holds exactly.
fn integral(number: &serde_json::Number) -> Option<i128> {
    if let Some(n) = number.as_i64() {
        return Some(i128::from(n));
    }
    if let Some(n) = number.as_u64() {
        return Some(i128::from(n));
    }
    let f = number.as_f64()?;
    (f.fract() == 0.0 && f.abs() <= MAX_SAFE_INTEGER).then_some(f as i128)
}

/// The whole number a decimal header value names: an optional `-`, digits,
/// and an optional fraction of zeros (`42`, `-7`, `42.0`). Other notation
/// (`+42`, `1e2`, ` 42`) names none.
fn decimal_integer(text: &str) -> Option<i128> {
    let (negative, unsigned) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (whole, fraction) = match unsigned.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (unsigned, None),
    };
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if let Some(fraction) = fraction
        && (fraction.is_empty() || !fraction.bytes().all(|b| b == b'0'))
    {
        return None;
    }
    let digits = whole.trim_start_matches('0');
    if digits.len() > 20 {
        return None;
    }
    let magnitude: i128 = if digits.is_empty() {
        0
    } else {
        digits.parse().ok()?
    };
    Some(if negative { -magnitude } else { magnitude })
}

/// The valid `x-mcp-header` annotations of `schema`, each with its value
/// in `instance`. Annotations that break a constraint are left out.
fn annotated_params(schema: &Value, instance: &Value) -> Vec<AnnotatedParam> {
    let mut out = Vec::new();
    let mut seen_lower = std::collections::HashSet::new();
    collect_promotions(schema, Some(instance), &mut out, &mut seen_lower);
    out
}

/// Walk a schema's `properties` chain (and only that chain) collecting
/// valid `x-mcp-header` annotations. Recurses into nested object
/// `properties` but never through `items` / composition / `$ref`,
/// mirroring the SEP "statically reachable" rule.
fn collect_promotions(
    schema: &Value,
    instance: Option<&Value>,
    out: &mut Vec<AnnotatedParam>,
    seen_lower: &mut std::collections::HashSet<String>,
) {
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return;
    };
    for (prop_name, prop_schema) in properties {
        let instance_value = instance.and_then(|instance| instance.get(prop_name));

        if let Some(header_name) = prop_schema
            .get(X_MCP_HEADER_KEYWORD)
            .and_then(Value::as_str)
            && is_valid_header_token(header_name)
            && is_promotable_primitive(prop_schema)
        {
            let lower = header_name.to_ascii_lowercase();
            // Drop (exclude) on duplicate header name rather than fail.
            if seen_lower.insert(lower.clone()) {
                out.push(AnnotatedParam {
                    header: lower,
                    value: instance_value.and_then(|value| ParamValue::of(prop_schema, value)),
                });
            }
        }

        // Statically reachable nested object properties only.
        if prop_schema
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|t| t == "object")
        {
            collect_promotions(prop_schema, instance_value, out, seen_lower);
        }
    }
}

/// Why the `x-mcp-header` annotations of `input_schema` break the
/// constraints the transport places on them, or `None` when they keep
/// every one: each value is a non-empty HTTP token, unique among them
/// regardless of case, on a `string`, `integer` or `boolean` property
/// reached through `properties` only. A client drops a definition whose
/// annotations break a constraint.
pub fn x_mcp_header_violation(input_schema: &Value) -> Option<String> {
    let mut seen_lower = std::collections::HashSet::new();
    annotation_violation(input_schema, "", &mut seen_lower)
}

fn annotation_violation(
    schema: &Value,
    path: &str,
    seen_lower: &mut std::collections::HashSet<String>,
) -> Option<String> {
    let properties = schema.get("properties").and_then(Value::as_object)?;
    for (prop_name, prop_schema) in properties {
        let at = if path.is_empty() {
            prop_name.clone()
        } else {
            format!("{path}.{prop_name}")
        };
        if let Some(annotation) = prop_schema.get(X_MCP_HEADER_KEYWORD) {
            let Some(header_name) = annotation.as_str() else {
                return Some(format!("`{at}`: {X_MCP_HEADER_KEYWORD} is not a string"));
            };
            if !is_valid_header_token(header_name) {
                return Some(format!(
                    "`{at}`: {X_MCP_HEADER_KEYWORD} `{header_name}` is not an HTTP token"
                ));
            }
            if !is_promotable_primitive(prop_schema) {
                return Some(format!(
                    "`{at}`: {X_MCP_HEADER_KEYWORD} is only allowed on a string, integer or \
                     boolean property"
                ));
            }
            if !seen_lower.insert(header_name.to_ascii_lowercase()) {
                return Some(format!(
                    "`{at}`: {X_MCP_HEADER_KEYWORD} `{header_name}` is used more than once"
                ));
            }
        }
        if let Some(violation) = annotation_violation(prop_schema, &at, seen_lower) {
            return Some(violation);
        }
    }
    None
}

/// Validate the `Mcp-Param-{Name}` headers of a request against the
/// `x-mcp-header` annotations of `input_schema` and the request's
/// `arguments`. `headers` holds each `Mcp-Param-` header as its lower-case
/// name after the prefix and its raw value.
///
/// Every annotated argument with a value needs its header, equal to the
/// value after sentinel decode (by number for an `integer` property, so
/// `42` and `42.0` are the same); an annotated argument without a value
/// must not have one. A header no annotation names is not checked here.
/// The error names the header, never its value.
pub fn validate_annotated_param_headers(
    input_schema: &Value,
    arguments: &Value,
    headers: &[(String, String)],
) -> Result<(), String> {
    for param in annotated_params(input_schema, arguments) {
        let sent = headers
            .iter()
            .find(|(name, _)| *name == param.header)
            .map(|(_, raw)| raw.as_str());
        match (param.value, sent) {
            (None, None) => {}
            (Some(_), None) => {
                return Err(format!(
                    "the `Mcp-Param-{}` header is required: the arguments carry the value \
                     it mirrors",
                    param.header
                ));
            }
            (None, Some(_)) => {
                return Err(format!(
                    "`Mcp-Param-{}` is sent, but the arguments carry no value for it",
                    param.header
                ));
            }
            (Some(value), Some(raw)) => match decode_header_value(raw) {
                Some(decoded) if value.matches(&decoded) => {}
                Some(_) => {
                    return Err(format!(
                        "the `Mcp-Param-{}` header does not match the argument it mirrors",
                        param.header
                    ));
                }
                None => {
                    return Err(format!(
                        "the `Mcp-Param-{}` header carries a malformed `=?base64?…?=` value",
                        param.header
                    ));
                }
            },
        }
    }
    Ok(())
}

/// Each `Mcp-Param-{Name}` header of `headers`, as its lower-case name
/// after the prefix and its value. A header whose value is not visible
/// ASCII, or whose `=?base64?…?=` value is malformed, is a
/// `HeaderMismatch` rejection.
pub fn param_headers(
    headers: &HeaderMap,
    jsonrpc_id: Option<Value>,
) -> Result<Vec<(String, String)>, TransportRejection> {
    let mut out = Vec::new();
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
        if decode_header_value(raw).is_none() {
            return Err(header_mismatch(
                &format!("`Mcp-Param-{param}` header carries a malformed `=?base64?…?=` value"),
                jsonrpc_id,
            ));
        }
        out.push((param.to_owned(), raw.to_owned()));
    }
    Ok(out)
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

    #[test]
    fn x_mcp_header_violations_are_named() {
        let valid = serde_json::json!({
            "type": "object",
            "properties": {
                "region": { "type": "string", "x-mcp-header": "Region" },
                "limits": {
                    "type": "object",
                    "properties": { "tier": { "type": "integer", "x-mcp-header": "Tier" } }
                },
                "plain": { "type": "string" }
            }
        });
        assert_eq!(x_mcp_header_violation(&valid), None);
        for bad in [
            serde_json::json!({ "properties": { "a": { "type": "string", "x-mcp-header": "" } } }),
            serde_json::json!({ "properties": { "a": { "type": "string", "x-mcp-header": "A B" } } }),
            serde_json::json!({ "properties": { "a": { "type": "number", "x-mcp-header": "A" } } }),
            serde_json::json!({ "properties": { "a": { "type": "string", "x-mcp-header": 7 } } }),
            serde_json::json!({ "properties": {
                "a": { "type": "string", "x-mcp-header": "Dup" },
                "b": { "type": "string", "x-mcp-header": "dup" }
            } }),
            serde_json::json!({ "properties": { "o": { "type": "object", "properties": {
                "a": { "type": "array", "x-mcp-header": "A" }
            } } } }),
        ] {
            assert!(x_mcp_header_violation(&bad).is_some(), "{bad}");
        }
    }

    #[test]
    fn annotated_param_headers_must_mirror_the_arguments() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "region": { "type": "string", "x-mcp-header": "Region" },
                "tier": { "type": "integer", "x-mcp-header": "TierLevel" },
                "note": { "type": "string" }
            }
        });
        let args = serde_json::json!({ "region": "Zürich", "tier": 2, "note": "x" });
        let good = vec![
            ("region".to_owned(), encode_header_value("Zürich")),
            ("tierlevel".to_owned(), "2".to_owned()),
            ("unrelated".to_owned(), "anything".to_owned()),
        ];
        assert_eq!(
            validate_annotated_param_headers(&schema, &args, &good),
            Ok(())
        );

        let missing = vec![("region".to_owned(), encode_header_value("Zürich"))];
        let error = validate_annotated_param_headers(&schema, &args, &missing).unwrap_err();
        assert!(error.contains("Mcp-Param-tierlevel"), "{error}");

        let wrong = vec![
            ("region".to_owned(), "Bern".to_owned()),
            ("tierlevel".to_owned(), "2".to_owned()),
        ];
        let error = validate_annotated_param_headers(&schema, &args, &wrong).unwrap_err();
        assert!(
            error.contains("Mcp-Param-region") && !error.contains("Bern"),
            "{error}"
        );

        let no_value = serde_json::json!({ "region": null });
        let stray = vec![("region".to_owned(), "x".to_owned())];
        assert!(validate_annotated_param_headers(&schema, &no_value, &stray).is_err());
        assert_eq!(
            validate_annotated_param_headers(&schema, &no_value, &[]),
            Ok(())
        );
    }

    /// An integer argument matches its header by number; a string or a
    /// boolean matches only its exact header string.
    #[test]
    fn integer_param_headers_compare_by_number() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "tier": { "type": "integer", "x-mcp-header": "Tier" },
                "code": { "type": "string", "x-mcp-header": "Code" },
                "dry": { "type": "boolean", "x-mcp-header": "Dry" }
            }
        });
        let check = |args: serde_json::Value, tier: &str| {
            validate_annotated_param_headers(
                &schema,
                &args,
                &[("tier".to_owned(), tier.to_owned())],
            )
        };
        let tier = |n: serde_json::Value| serde_json::json!({ "tier": n });
        let forty_two = || tier(serde_json::json!(42));

        for header in ["42", "42.0", "42.000", "042", "=?base64?NDIuMA==?="] {
            assert_eq!(check(forty_two(), header), Ok(()), "{header}");
        }
        assert_eq!(check(tier(serde_json::json!(42.0)), "42"), Ok(()));
        assert_eq!(check(tier(serde_json::json!(-7)), "-7.0"), Ok(()));
        assert_eq!(check(tier(serde_json::json!(0)), "-0"), Ok(()));
        let safe_max = tier(serde_json::json!(9_007_199_254_740_991_i64));
        assert_eq!(check(safe_max, "9007199254740991"), Ok(()));
        let refused = [
            "43", "42.5", "1e2", "4.2e1", "+42", " 42", "42.", ".0", "0x2a", "",
        ];
        for header in refused {
            assert!(check(forty_two(), header).is_err(), "{header:?}");
        }
        assert!(check(tier(serde_json::json!(100)), "1e2").is_err());

        // A string argument that looks like a number stays a string.
        let code = |header: &str| {
            validate_annotated_param_headers(
                &schema,
                &serde_json::json!({ "code": "42" }),
                &[("code".to_owned(), header.to_owned())],
            )
        };
        assert_eq!(code("42"), Ok(()));
        assert!(code("42.0").is_err());
        let dry = |header: &str| {
            validate_annotated_param_headers(
                &schema,
                &serde_json::json!({ "dry": true }),
                &[("dry".to_owned(), header.to_owned())],
            )
        };
        assert_eq!(dry("true"), Ok(()));
        assert!(dry("True").is_err());
        assert!(dry("1").is_err());

        // Promotion sends the decimal form of a whole number.
        assert_eq!(
            promote_param_headers(&schema, &tier(serde_json::json!(42.0))),
            vec![("mcp-param-tier".to_owned(), "42".to_owned())]
        );
    }

    #[test]
    fn param_headers_are_collected_and_checked_for_shape() {
        let mut headers = HeaderMap::new();
        headers.insert("mcp-param-region", "us-west1".parse().unwrap());
        headers.insert("mcp-name", "x".parse().unwrap());
        let Ok(collected) = param_headers(&headers, None) else {
            panic!("well-formed headers are collected");
        };
        assert_eq!(
            collected,
            vec![("region".to_owned(), "us-west1".to_owned())]
        );
        headers.insert("mcp-param-bad", "=?base64?not base64!?=".parse().unwrap());
        let Err(rejection) = param_headers(&headers, None) else {
            panic!("a malformed sentinel is refused");
        };
        assert_eq!(rejection.error_code, HEADER_MISMATCH_CODE);
    }
}
