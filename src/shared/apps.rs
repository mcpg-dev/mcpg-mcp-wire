//! Version-agnostic core of the SEP-1865 **MCP Apps** extension.
//!
//! MCP Apps lets a server attach an interactive HTML UI to a tool: the
//! tool descriptor names a `ui://` resource, the host fetches it, and
//! renders the HTML in a sandboxed `<iframe>`. The bulk of the Apps
//! protocol (`ui/*` postMessage methods) runs host↔iframe and **never
//! touches MCPG**.
//!
//! From a gateway's perspective there are **no new MCP-wire methods**.
//! MCPG's job is:
//!
//! 1. carry `_meta.ui.*` on tool/resource descriptors and
//!    `tools/call` results unchanged (both wire versions);
//! 2. advertise `io.modelcontextprotocol/ui` downstream (to clients)
//!    and upstream (to federated servers, so they emit UI tools);
//! 3. keep `_meta.ui.resourceUri` pointing at the `ui://` resource the
//!    gateway actually serves after federation prefixing
//!    ([`rewrite_tool_resource_uri`]);
//! 4. clamp operator policy — intersect the upstream-declared CSP
//!    (both `_meta.ui.csp` and the proprietary snake_case
//!    `openai/widgetCSP` alias that ChatGPT-class hosts honour) and
//!    strip iframe permissions to an operator allow-list, on egress
//!    ([`AppsPolicy::apply_to_resource_meta`]).
//!
//! This module owns the pieces that are identical across wire versions
//! (constants, the `ui://` URI helpers, the policy engine, the
//! capability-object builder) so the modern handler, the legacy
//! runtime, and the federation engine all share one implementation.
//!
//! ## The one nuance that bites the typed-`_meta` code
//!
//! Every other SEP-2133 extension keys its `_meta` entry by the full
//! reverse-DNS namespace (`io.modelcontextprotocol/cacheToken`). MCP
//! Apps breaks the convention: its key is the bare string **`ui`**. So
//! none of MCPG's `io.modelcontextprotocol/*`-keyed `_meta` machinery
//! captures it — everything here keys on the literal `"ui"`.

use serde_json::{Map, Value};

/// SEP-2133 extension identifier advertised in
/// `capabilities.extensions`. Note: `/ui`, **not** `/apps`.
pub const EXTENSION_ID: &str = "io.modelcontextprotocol/ui";

/// The only content type Apps supports in the MVP. The
/// `;profile=mcp-app` parameter is load-bearing — it triggers UI
/// rendering host-side and MUST survive byte-exact through MCPG.
pub const UI_MIME_TYPE: &str = "text/html;profile=mcp-app";

/// URI scheme distinguishing UI resources from ordinary resources.
pub const UI_URI_SCHEME: &str = "ui://";

/// Default `mimeTypes` advertised when MCPG opts into the extension
/// without a more specific upstream-reflected set.
pub const DEFAULT_MIME_TYPES: &[&str] = &[UI_MIME_TYPE];

/// The four CSP axes carried on `_meta.ui.csp`, as their wire
/// (camelCase) keys. Order matches the SEP field order.
pub const CSP_AXES: [&str; 4] = [
    "connectDomains",
    "resourceDomains",
    "frameDomains",
    "baseUriDomains",
];

/// The four iframe permission keys carried on `_meta.ui.permissions`,
/// as their wire (camelCase) keys.
pub const PERMISSION_KEYS: [&str; 4] = ["camera", "microphone", "geolocation", "clipboardWrite"];

/// OpenAI's Apps SDK carries CSP as the proprietary `_meta` key
/// `openai/widgetCSP` — a top-level sibling of `ui` (not nested under
/// it) with **snake_case** axes. ChatGPT-class hosts honour this alias,
/// so a value left unclamped here would let an upstream's allow-list
/// exceed operator policy regardless of `_meta.ui.csp`. The egress
/// clamp intersects it in place.
pub const OPENAI_WIDGET_CSP_KEY: &str = "openai/widgetCSP";

/// `openai/widgetCSP` snake_case axes, each paired with the operator
/// allow-list axis it clamps against. `redirectDomains` (bounds
/// `openExternal` link targets) has no SEP-1865 `_meta.ui.csp` axis.
const OPENAI_CSP_AXES: [(&str, &str); 4] = [
    ("connect_domains", "connectDomains"),
    ("resource_domains", "resourceDomains"),
    ("frame_domains", "frameDomains"),
    ("redirect_domains", "redirectDomains"),
];

/// OpenAI's proprietary dedicated-sandbox-origin alias — the snake-cased
/// equivalent of `_meta.ui.domain`; clamped against `allowed_domains`.
pub const OPENAI_WIDGET_DOMAIN_KEY: &str = "openai/widgetDomain";

/// True when `uri` is an Apps UI resource URI (`ui://…`).
pub fn is_ui_uri(uri: &str) -> bool {
    uri.starts_with(UI_URI_SCHEME)
}

/// Build the capability object advertised under
/// `capabilities.extensions["io.modelcontextprotocol/ui"]`.
///
/// `mime_types` is the (possibly unioned-across-upstreams) set of
/// content types; empty falls back to [`DEFAULT_MIME_TYPES`].
pub fn capability_value(mime_types: &[String]) -> Value {
    let list: Vec<Value> = if mime_types.is_empty() {
        DEFAULT_MIME_TYPES.iter().map(|s| Value::from(*s)).collect()
    } else {
        mime_types.iter().cloned().map(Value::from).collect()
    };
    serde_json::json!({ "mimeTypes": list })
}

// ───────────────────────────── resourceUri ─────────────────────────────

/// Read a tool's `_meta.ui.resourceUri`, accepting the deprecated flat
/// alias `_meta["ui/resourceUri"]` as a fallback.
pub fn tool_resource_uri(meta: &Value) -> Option<&str> {
    meta.get("ui")
        .and_then(|u| u.get("resourceUri"))
        .and_then(Value::as_str)
        .or_else(|| meta.get("ui/resourceUri").and_then(Value::as_str))
}

/// Rewrite a tool's `_meta.ui.resourceUri` through `map`, normalizing
/// the deprecated flat alias into the nested `ui.resourceUri` form.
///
/// Used by the federation engine: when a federated server's `ui://`
/// resources are re-served under a gateway-side URI, the tool's
/// `resourceUri` must be rewritten in lockstep or the host fetches a
/// URI MCPG doesn't serve. `map` returns the rewritten URI, or `None`
/// to leave it unchanged. No-op when no `resourceUri` is present.
///
/// Non-destructive: every sibling `_meta` key (and every sibling key
/// inside `ui`) is preserved.
pub fn rewrite_tool_resource_uri(meta: &mut Value, map: impl Fn(&str) -> Option<String>) {
    let Some(current) = tool_resource_uri(meta).map(str::to_owned) else {
        return;
    };
    let Some(rewritten) = map(&current) else {
        return;
    };
    let Value::Object(obj) = meta else {
        return;
    };
    // Normalize: the canonical home is `ui.resourceUri`; drop the
    // deprecated flat alias if present.
    obj.remove("ui/resourceUri");
    let ui = obj.entry("ui").or_insert_with(|| Value::Object(Map::new()));
    if let Value::Object(ui_obj) = ui {
        ui_obj.insert("resourceUri".to_owned(), Value::from(rewritten));
    }
}

// ───────────────────────────── policy ──────────────────────────────────

/// Compiled operator policy for `_meta.ui` on UI resources. Built from
/// `mcp.configurations.apps` config; see
/// [`crate::config::apps::AppsConfig::compiled_policy`].
///
/// All clamping is **tighten-only**: MCPG can narrow what a host will
/// honour, never widen it. CSP axes are set-intersected; permissions
/// outside the allow-list are stripped; out-of-list sandbox domains are
/// dropped (or, in `strict` mode, rejected by the caller).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppsPolicy {
    pub connect_domains: Vec<String>,
    pub resource_domains: Vec<String>,
    pub frame_domains: Vec<String>,
    pub base_uri_domains: Vec<String>,
    /// Allow-list for `openExternal` redirect targets — clamps the
    /// OpenAI `openai/widgetCSP.redirect_domains` axis. No SEP-1865
    /// `_meta.ui.csp` equivalent, so it only ever bounds the alias.
    pub redirect_domains: Vec<String>,
    /// Allowed iframe permissions, as wire (camelCase) keys.
    pub allowed_permissions: Vec<String>,
    /// Allowed sandbox `domain` values. `None` ⇒ any domain allowed.
    pub allowed_domains: Option<Vec<String>>,
    /// When true, the caller rejects (rather than sanitizes) a response
    /// whose `_meta.ui` escaped the policy (see [`PolicyReport`]).
    pub strict: bool,
}

/// What [`AppsPolicy::apply_to_resource_meta`] changed, for metrics,
/// audit, and strict-mode rejection.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PolicyReport {
    /// CSP axes whose domain set was narrowed.
    pub csp_axes_narrowed: Vec<&'static str>,
    /// Permission keys removed (outside the allow-list).
    pub permissions_stripped: Vec<String>,
    /// `domain` was outside the allow-list and dropped.
    pub domain_dropped: bool,
    /// Human-readable descriptions of each escape, for strict mode +
    /// audit. Non-empty ⇒ the upstream asked for more than policy
    /// permits.
    pub violations: Vec<String>,
}

impl PolicyReport {
    /// True when the upstream `_meta.ui` requested something the policy
    /// does not permit (a domain/permission/CSP entry was removed).
    pub fn has_violations(&self) -> bool {
        !self.violations.is_empty()
    }

    /// True when nothing changed.
    pub fn is_noop(&self) -> bool {
        self.csp_axes_narrowed.is_empty()
            && self.permissions_stripped.is_empty()
            && !self.domain_dropped
    }
}

/// Intersect an upstream CSP domain list with the operator's allow-list
/// for that axis.
///
/// - operator `["*"]` ⇒ no bound on this axis: pass the upstream list
///   through unchanged.
/// - upstream `["*"]` ∩ a concrete operator list ⇒ the operator list
///   (the wildcard is *narrowed*, never widened).
/// - otherwise a plain set intersection, treating `'self'` / `'none'`
///   as opaque atoms.
fn intersect_axis(upstream: &[String], operator: &[String]) -> Vec<String> {
    if operator.iter().any(|d| d == "*") {
        return upstream.to_vec();
    }
    if upstream.iter().any(|d| d == "*") {
        return operator.to_vec();
    }
    upstream
        .iter()
        .filter(|d| operator.iter().any(|o| o == *d))
        .cloned()
        .collect()
}

impl AppsPolicy {
    fn operator_axis(&self, axis: &str) -> &[String] {
        match axis {
            "connectDomains" => &self.connect_domains,
            "resourceDomains" => &self.resource_domains,
            "frameDomains" => &self.frame_domains,
            "baseUriDomains" => &self.base_uri_domains,
            "redirectDomains" => &self.redirect_domains,
            _ => &[],
        }
    }

    /// Clamp `_meta.ui` on a resource descriptor or `resources/read`
    /// content block in place, returning what changed.
    ///
    /// Honors the SEP's omitted-means-restrictive defaults: an *absent*
    /// CSP axis (host default `frame-src 'none'` / `base-uri 'self'`)
    /// is left absent — policy never *materializes* an axis, which
    /// would loosen by replacing a restrictive default with an explicit
    /// permissive value. It only ever narrows axes the upstream
    /// actually declared.
    ///
    /// Non-destructive outside the policy fields: sibling `_meta` keys,
    /// `ui.resourceUri`, `ui.prefersBorder`, any unknown future `ui.*`
    /// sub-keys, and every non-render `openai/*` / `mcpui.dev/*` alias
    /// are untouched.
    ///
    /// The proprietary `openai/widgetCSP` / `openai/widgetDomain`
    /// aliases (which ChatGPT-class hosts honour) are clamped in place
    /// with the same tighten-only semantics as their `_meta.ui.*`
    /// counterparts — so a resource that declares its policy *only* via
    /// the alias cannot escape the operator allow-list. These run even
    /// when no `ui` object is present.
    pub fn apply_to_resource_meta(&self, meta: &mut Value) -> PolicyReport {
        let mut report = PolicyReport::default();
        let Some(obj) = meta.as_object_mut() else {
            return report;
        };

        // ── OpenAI proprietary aliases (clamp before / independent of
        //    `ui`, since a host may honour the alias on its own) ──
        self.clamp_openai_widget_csp(obj, &mut report);
        self.clamp_openai_widget_domain(obj, &mut report);

        // ── standard `_meta.ui.*` ──
        let Some(ui) = obj.get_mut("ui").and_then(Value::as_object_mut) else {
            return report;
        };

        // ── CSP intersection ──
        if let Some(csp) = ui.get_mut("csp").and_then(Value::as_object_mut) {
            for axis in CSP_AXES {
                let Some(upstream) = csp.get(axis).and_then(Value::as_array) else {
                    continue; // absent axis: leave the host's restrictive default
                };
                let upstream: Vec<String> = upstream
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect();
                let narrowed = intersect_axis(&upstream, self.operator_axis(axis));
                if narrowed != upstream {
                    report.csp_axes_narrowed.push(axis);
                    let dropped: Vec<&String> =
                        upstream.iter().filter(|d| !narrowed.contains(d)).collect();
                    if !dropped.is_empty() {
                        report.violations.push(format!(
                            "csp.{axis} dropped {dropped:?} (outside operator allow-list)"
                        ));
                    }
                    csp.insert(
                        axis.to_owned(),
                        Value::Array(narrowed.into_iter().map(Value::from).collect()),
                    );
                }
            }
        }

        // ── permission stripping ──
        if let Some(perms) = ui.get_mut("permissions").and_then(Value::as_object_mut) {
            let to_remove: Vec<String> = perms
                .keys()
                .filter(|k| !self.allowed_permissions.iter().any(|a| a == *k))
                .cloned()
                .collect();
            for key in to_remove {
                perms.remove(&key);
                report
                    .violations
                    .push(format!("permission '{key}' stripped (not in allow-list)"));
                report.permissions_stripped.push(key);
            }
        }

        // ── sandbox domain allow-list ──
        if let Some(allowed) = &self.allowed_domains {
            let declared = ui.get("domain").and_then(Value::as_str).map(str::to_owned);
            if let Some(domain) = declared
                && !allowed.contains(&domain)
            {
                report.domain_dropped = true;
                report
                    .violations
                    .push(format!("domain '{domain}' dropped (outside allow-list)"));
                ui.remove("domain");
            }
        }

        report
    }

    /// Intersect the snake_case axes of an `openai/widgetCSP` alias in
    /// place, mirroring the `_meta.ui.csp` clamp. Tighten-only: an
    /// absent axis is left absent; a present axis is narrowed to the
    /// operator allow-list (`redirect_domains` against the dedicated
    /// `redirect_domains` axis).
    fn clamp_openai_widget_csp(&self, obj: &mut Map<String, Value>, report: &mut PolicyReport) {
        let Some(csp) = obj
            .get_mut(OPENAI_WIDGET_CSP_KEY)
            .and_then(Value::as_object_mut)
        else {
            return;
        };
        for (alias_axis, operator_axis) in OPENAI_CSP_AXES {
            let Some(upstream) = csp.get(alias_axis).and_then(Value::as_array) else {
                continue;
            };
            let upstream: Vec<String> = upstream
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            let narrowed = intersect_axis(&upstream, self.operator_axis(operator_axis));
            if narrowed != upstream {
                report.csp_axes_narrowed.push(operator_axis);
                let dropped: Vec<&String> =
                    upstream.iter().filter(|d| !narrowed.contains(d)).collect();
                if !dropped.is_empty() {
                    report.violations.push(format!(
                        "openai/widgetCSP.{alias_axis} dropped {dropped:?} \
                         (outside operator allow-list)"
                    ));
                }
                csp.insert(
                    alias_axis.to_owned(),
                    Value::Array(narrowed.into_iter().map(Value::from).collect()),
                );
            }
        }
    }

    /// Drop an `openai/widgetDomain` alias outside the sandbox-domain
    /// allow-list, mirroring the `_meta.ui.domain` clamp.
    fn clamp_openai_widget_domain(&self, obj: &mut Map<String, Value>, report: &mut PolicyReport) {
        let Some(allowed) = &self.allowed_domains else {
            return;
        };
        let declared = obj
            .get(OPENAI_WIDGET_DOMAIN_KEY)
            .and_then(Value::as_str)
            .map(str::to_owned);
        if let Some(domain) = declared
            && !allowed.contains(&domain)
        {
            report.domain_dropped = true;
            report.violations.push(format!(
                "openai/widgetDomain '{domain}' dropped (outside allow-list)"
            ));
            obj.remove(OPENAI_WIDGET_DOMAIN_KEY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn policy() -> AppsPolicy {
        AppsPolicy {
            connect_domains: vec!["api.example.com".to_owned()],
            resource_domains: vec!["*".to_owned()],
            frame_domains: vec!["self".to_owned()],
            base_uri_domains: vec!["self".to_owned()],
            redirect_domains: vec!["chat.example.com".to_owned()],
            allowed_permissions: vec!["camera".to_owned(), "geolocation".to_owned()],
            allowed_domains: Some(vec!["trusted.example.com".to_owned()]),
            strict: false,
        }
    }

    #[test]
    fn constants_match_spec() {
        assert_eq!(EXTENSION_ID, "io.modelcontextprotocol/ui");
        assert_eq!(UI_MIME_TYPE, "text/html;profile=mcp-app");
        assert_eq!(UI_URI_SCHEME, "ui://");
        assert!(is_ui_uri("ui://srv/widget"));
        assert!(!is_ui_uri("https://srv/widget"));
    }

    #[test]
    fn capability_value_defaults_and_override() {
        assert_eq!(
            capability_value(&[]),
            json!({ "mimeTypes": ["text/html;profile=mcp-app"] })
        );
        assert_eq!(
            capability_value(&["text/html;profile=mcp-app".to_owned(), "x/y".to_owned()]),
            json!({ "mimeTypes": ["text/html;profile=mcp-app", "x/y"] })
        );
    }

    #[test]
    fn intersect_operator_wildcard_passes_upstream_through() {
        let up = vec!["a.com".to_owned(), "b.com".to_owned()];
        assert_eq!(intersect_axis(&up, &["*".to_owned()]), up);
    }

    #[test]
    fn intersect_upstream_wildcard_narrows_to_operator() {
        let up = vec!["*".to_owned()];
        let op = vec!["api.example.com".to_owned()];
        assert_eq!(intersect_axis(&up, &op), op);
    }

    #[test]
    fn intersect_plain_sets_with_self_atom() {
        let up = vec![
            "self".to_owned(),
            "evil.com".to_owned(),
            "ok.com".to_owned(),
        ];
        let op = vec!["self".to_owned(), "ok.com".to_owned()];
        assert_eq!(
            intersect_axis(&up, &op),
            vec!["self".to_owned(), "ok.com".to_owned()]
        );
    }

    #[test]
    fn intersect_empty_when_disjoint() {
        let up = vec!["evil.com".to_owned()];
        let op = vec!["ok.com".to_owned()];
        assert!(intersect_axis(&up, &op).is_empty());
    }

    #[test]
    fn apply_narrows_present_axis_and_leaves_absent_axis_untouched() {
        let p = policy();
        let mut meta = json!({
            "ui": { "csp": { "connectDomains": ["*"] } }  // frameDomains absent
        });
        let report = p.apply_to_resource_meta(&mut meta);
        // operator connect_domains is concrete ⇒ upstream "*" narrowed.
        assert_eq!(
            meta["ui"]["csp"]["connectDomains"],
            json!(["api.example.com"])
        );
        // absent frameDomains stays absent (host default 'none' preserved).
        assert!(meta["ui"]["csp"].get("frameDomains").is_none());
        assert!(report.csp_axes_narrowed.contains(&"connectDomains"));
    }

    #[test]
    fn apply_resource_domains_wildcard_operator_is_noop() {
        let p = policy();
        let mut meta = json!({ "ui": { "csp": { "resourceDomains": ["cdn.example.com"] } } });
        let report = p.apply_to_resource_meta(&mut meta);
        assert_eq!(
            meta["ui"]["csp"]["resourceDomains"],
            json!(["cdn.example.com"])
        );
        assert!(!report.csp_axes_narrowed.contains(&"resourceDomains"));
    }

    #[test]
    fn apply_strips_disallowed_permissions_only() {
        let p = policy();
        let mut meta = json!({
            "ui": { "permissions": { "camera": {}, "microphone": {}, "geolocation": {} } }
        });
        let report = p.apply_to_resource_meta(&mut meta);
        let perms = meta["ui"]["permissions"].as_object().unwrap();
        assert!(perms.contains_key("camera"));
        assert!(perms.contains_key("geolocation"));
        assert!(!perms.contains_key("microphone"));
        assert_eq!(report.permissions_stripped, vec!["microphone".to_owned()]);
    }

    #[test]
    fn apply_drops_domain_outside_allow_list() {
        let p = policy();
        let mut meta = json!({ "ui": { "domain": "evil.example.com" } });
        let report = p.apply_to_resource_meta(&mut meta);
        assert!(meta["ui"].get("domain").is_none());
        assert!(report.domain_dropped);
        assert!(report.has_violations());
    }

    #[test]
    fn apply_keeps_domain_inside_allow_list() {
        let p = policy();
        let mut meta = json!({ "ui": { "domain": "trusted.example.com" } });
        let report = p.apply_to_resource_meta(&mut meta);
        assert_eq!(meta["ui"]["domain"], "trusted.example.com");
        assert!(!report.domain_dropped);
    }

    #[test]
    fn apply_preserves_sibling_meta_and_unknown_ui_subkeys() {
        let p = policy();
        let mut meta = json!({
            "mcpg": { "source": { "federatedFrom": "notion" } },
            "ui": {
                "resourceUri": "ui://srv/x",
                "prefersBorder": true,
                "futureKey": { "x": 1 },
                "permissions": { "microphone": {} }
            }
        });
        p.apply_to_resource_meta(&mut meta);
        // sibling namespace untouched
        assert_eq!(meta["mcpg"]["source"]["federatedFrom"], "notion");
        // unknown ui sub-keys untouched
        assert_eq!(meta["ui"]["resourceUri"], "ui://srv/x");
        assert_eq!(meta["ui"]["prefersBorder"], true);
        assert_eq!(meta["ui"]["futureKey"]["x"], 1);
        // disallowed permission stripped
        assert!(meta["ui"]["permissions"].as_object().unwrap().is_empty());
    }

    #[test]
    fn apply_is_noop_when_no_ui_object() {
        let p = policy();
        let mut meta = json!({ "mcpg": { "source": "x" } });
        let report = p.apply_to_resource_meta(&mut meta);
        assert!(report.is_noop());
        assert_eq!(meta, json!({ "mcpg": { "source": "x" } }));
    }

    #[test]
    fn apply_clamps_openai_widget_csp_alias_in_place() {
        let p = policy();
        let mut meta = json!({
            "openai/widgetCSP": {
                "connect_domains": ["api.example.com", "evil.com"],
                "resource_domains": ["*"],          // operator resource = ["*"] ⇒ passthrough
                "frame_domains": ["self", "evil.com"]
            }
        });
        let report = p.apply_to_resource_meta(&mut meta);
        // connect narrowed to the operator allow-list
        assert_eq!(
            meta["openai/widgetCSP"]["connect_domains"],
            json!(["api.example.com"])
        );
        // resource untouched (operator "*")
        assert_eq!(meta["openai/widgetCSP"]["resource_domains"], json!(["*"]));
        // frame narrowed to "self" (evil.com dropped)
        assert_eq!(meta["openai/widgetCSP"]["frame_domains"], json!(["self"]));
        assert!(report.csp_axes_narrowed.contains(&"connectDomains"));
        assert!(report.csp_axes_narrowed.contains(&"frameDomains"));
        assert!(report.has_violations());
    }

    #[test]
    fn apply_clamps_openai_widget_csp_with_no_ui_object() {
        // The core regression: a resource whose ONLY CSP declaration is
        // the snake_case OpenAI alias (no `_meta.ui` at all) must still
        // be clamped — pre-fix, the function early-returned and a host
        // honouring `openai/widgetCSP` saw the operator policy bypassed.
        let p = policy();
        let mut meta = json!({
            "openai/widgetCSP": { "connect_domains": ["*"] }
        });
        let report = p.apply_to_resource_meta(&mut meta);
        assert_eq!(
            meta["openai/widgetCSP"]["connect_domains"],
            json!(["api.example.com"])
        );
        assert!(report.has_violations());
    }

    #[test]
    fn apply_clamps_redirect_domains_axis() {
        let p = policy(); // redirect_domains = ["chat.example.com"]
        let mut meta = json!({
            "openai/widgetCSP": { "redirect_domains": ["chat.example.com", "evil.com"] }
        });
        let report = p.apply_to_resource_meta(&mut meta);
        assert_eq!(
            meta["openai/widgetCSP"]["redirect_domains"],
            json!(["chat.example.com"])
        );
        assert!(report.csp_axes_narrowed.contains(&"redirectDomains"));
    }

    #[test]
    fn apply_drops_openai_widget_domain_outside_allow_list() {
        let p = policy(); // allowed_domains = ["trusted.example.com"]
        let mut meta = json!({ "openai/widgetDomain": "evil.example.com" });
        let report = p.apply_to_resource_meta(&mut meta);
        assert!(meta.get("openai/widgetDomain").is_none());
        assert!(report.domain_dropped);

        let mut ok = json!({ "openai/widgetDomain": "trusted.example.com" });
        p.apply_to_resource_meta(&mut ok);
        assert_eq!(ok["openai/widgetDomain"], "trusted.example.com");
    }

    #[test]
    fn apply_preserves_non_render_openai_and_mcpui_aliases() {
        let p = policy();
        let mut meta = json!({
            "openai/widgetDescription": "A board view",
            "openai/toolInvocation/invoking": "Preparing…",
            "mcpui.dev/ui-preferred-frame-size": [800, 600],
            "ui": { "csp": { "connectDomains": ["*"] } }
        });
        p.apply_to_resource_meta(&mut meta);
        // non-render aliases pass through byte-faithfully
        assert_eq!(meta["openai/widgetDescription"], "A board view");
        assert_eq!(meta["openai/toolInvocation/invoking"], "Preparing…");
        assert_eq!(meta["mcpui.dev/ui-preferred-frame-size"], json!([800, 600]));
        // the standard ui.csp axis is still clamped
        assert_eq!(
            meta["ui"]["csp"]["connectDomains"],
            json!(["api.example.com"])
        );
    }

    #[test]
    fn rewrite_resource_uri_nested_form() {
        let mut meta = json!({ "ui": { "resourceUri": "ui://srv/x", "prefersBorder": true } });
        rewrite_tool_resource_uri(&mut meta, |u| Some(format!("ui://notion/{}", &u[5..])));
        assert_eq!(meta["ui"]["resourceUri"], "ui://notion/srv/x");
        assert_eq!(meta["ui"]["prefersBorder"], true);
    }

    #[test]
    fn rewrite_resource_uri_normalizes_deprecated_alias() {
        let mut meta = json!({ "ui/resourceUri": "ui://srv/x" });
        rewrite_tool_resource_uri(&mut meta, |u| Some(format!("ui://notion/{}", &u[5..])));
        assert_eq!(meta["ui"]["resourceUri"], "ui://notion/srv/x");
        assert!(meta.get("ui/resourceUri").is_none());
    }

    #[test]
    fn rewrite_resource_uri_noop_when_absent_or_unmapped() {
        let mut meta = json!({ "ui": { "prefersBorder": true } });
        rewrite_tool_resource_uri(&mut meta, |_| Some("x".to_owned()));
        assert!(meta["ui"].get("resourceUri").is_none());

        let mut meta2 = json!({ "ui": { "resourceUri": "ui://srv/x" } });
        rewrite_tool_resource_uri(&mut meta2, |_| None);
        assert_eq!(meta2["ui"]["resourceUri"], "ui://srv/x");
    }

    #[test]
    fn tool_resource_uri_reads_both_forms() {
        assert_eq!(
            tool_resource_uri(&json!({ "ui": { "resourceUri": "ui://a" } })),
            Some("ui://a")
        );
        assert_eq!(
            tool_resource_uri(&json!({ "ui/resourceUri": "ui://b" })),
            Some("ui://b")
        );
        assert_eq!(tool_resource_uri(&json!({ "ui": {} })), None);
    }
}
