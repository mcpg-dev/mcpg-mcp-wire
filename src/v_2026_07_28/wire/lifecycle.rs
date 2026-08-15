//! Modern lifecycle wire types — `server/discover`.
//!
//! In 2026-07-28 the legacy three-step handshake
//! (`initialize` request → `InitializeResult` response →
//! `notifications/initialized` notification → session usable) is
//! replaced by a single stateless request: `server/discover`.
//!
//! Properties:
//!
//! - **Single round-trip.** The client posts `server/discover`,
//!   the server returns capabilities, and the conversation
//!   continues without a follow-up notification. SEP-2575 +
//!   SEP-2567.
//! - **No session.** Modern requests are stateless; the response
//!   does not mint an `Mcp-Session-Id`. Subsequent calls do not
//!   carry one.
//! - **Capability-only.** Server response shape carries
//!   `serverInfo`, `supportedVersions`, capabilities, and an
//!   optional `instructions` string. It is a `CacheableResult`
//!   (SEP-2549/2322): required `resultType` + `ttlMs` +
//!   `cacheScope`. The modern
//!   `ServerCapabilities` is a slimmer surface than 2025-11-25:
//!   `subscribe` is no longer a `resources` sub-flag (it moves to
//!   `subscriptions/listen` per SEP-2575); `roots`, `sampling`,
//!   and `logging` are deprecated per SEP-2577 (12-month grace
//!   runway) and intentionally absent from this file.
//! - **Extensions.** SEP-2133 lands as the typed `extensions`
//!   maps on both `ClientCapabilities` and `ServerCapabilities`;
//!   keys are reverse-DNS strings (`io.modelcontextprotocol/...`
//!   or vendor-prefixed).
//!
//! The `inputs`, `subscriptions`, and `tasks` capabilities the
//! modern revision adds for MRTR / `subscriptions/listen` / the
//! tasks extension are introduced in Phases 4 and 5; this file
//! ships the discovery surface that's reachable today.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::shared::caching::{CacheScope, default_result_type_complete};
use crate::shared::content::Icon;

/// JSON-RPC method name for the modern discovery request.
pub const METHOD_SERVER_DISCOVER: &str = "server/discover";

// ---------------------------------------------------------------------------
// Request / response.
// ---------------------------------------------------------------------------

/// Parameters for `server/discover`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverParams {
    /// Protocol version the client is pinning. Servers MAY downgrade
    /// to a version they support; the set the server can speak is
    /// returned in [`DiscoverResult::supported_versions`].
    ///
    /// SEP-2575 lets stateless clients carry this on the
    /// `_meta.io.modelcontextprotocol/protocolVersion` slot
    /// instead of the top-level params field; either placement is
    /// accepted, so the typed field is `Option<String>` and the
    /// handler falls back to reading `_meta` when missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<String>,
    /// Identity + version of the client. Optional for the same
    /// SEP-2575 reason as `protocol_version` above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ImplementationInfo>,
    /// Client-side capability advertisement.
    #[serde(default)]
    pub capabilities: ClientCapabilities,
    /// `_meta` namespace — typically empty on discovery, but SEP-2133
    /// keeps the slot reserved.
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// Result body for `server/discover`. A `CacheableResult`
/// (SEP-2549/2322): `resultType` + `ttlMs` + `cacheScope` are
/// required on the modern wire. The final schema dropped the
/// singular `protocolVersion` field — discovery advertises the
/// `supportedVersions` set instead, and the negotiated revision
/// rides on the `MCP-Protocol-Version` response header.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverResult {
    /// SEP-2322 result-type discriminator. Always `"complete"`.
    #[serde(default = "default_result_type_complete")]
    pub result_type: String,
    /// All protocol revisions this server can serve. Clients use
    /// this to negotiate down when their preferred revision is
    /// unavailable.
    pub supported_versions: Vec<String>,
    /// Identity + version of the server (the gateway plus a
    /// `_meta` block carrying the upstream backend names).
    pub server_info: ImplementationInfo,
    /// Server-side capability advertisement.
    #[serde(default)]
    pub capabilities: ServerCapabilities,
    /// Free-form prose the client SHOULD surface to the user, e.g.
    /// "This gateway exposes 23 tools, 4 prompts, and a JSON-RPC
    /// resource catalog from your team's deployments."
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// SEP-2549 cache lifetime (ms).
    pub ttl_ms: u64,
    /// SEP-2549 cache bucket. Discovery is identical for every
    /// client, so it is `public`.
    pub cache_scope: CacheScope,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

// ---------------------------------------------------------------------------
// Implementation identity (used by both sides).
// ---------------------------------------------------------------------------

/// Self-description of an MCP participant.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImplementationInfo {
    /// Machine-readable identifier — usually a reverse-DNS string.
    pub name: String,
    /// Human-readable display name (`title`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Semver string. Servers SHOULD bump on every backwards-
    /// incompatible capability change.
    pub version: String,
    /// One-paragraph self-description shown next to `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Branding icons (same shape as 2025-11-25: `src` + `mimeType`
    /// + `sizes` + `theme`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icons: Option<Vec<Icon>>,
    /// Canonical project / vendor URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website_url: Option<String>,
}

// ---------------------------------------------------------------------------
// Server capability advertisement.
// ---------------------------------------------------------------------------

/// Modern server-side capabilities.
///
/// Each `Option<...>` field is `None` when the server does not
/// advertise that capability. Capabilities the server *does*
/// support are present even when all their sub-flags would
/// serialize to `null` — clients distinguish "feature present, no
/// extra flags" from "feature absent" by whether the key appears at
/// all.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// Tool-call surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    /// Prompt catalog surface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
    /// Resource read / template surface. The `subscribe` sub-flag
    /// here advertises per-resource update delivery; unlike
    /// 2025-11-25, the subscribe/unsubscribe *methods* are removed and
    /// subscriptions are established over `subscriptions/listen`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    /// `completion/complete` argument-completion surface. The wire key
    /// is the plural `completions` (the request method is singular
    /// `completion/complete`, but the capability is `completions` per
    /// the schema's `ServerCapabilities`).
    #[serde(
        rename = "completions",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub completions: Option<CompletionCapability>,
    /// SEP-2133 extensions — reverse-DNS-keyed opaque advertisement
    /// map. MCPG uses this to surface
    /// `io.modelcontextprotocol/tasks` and any
    /// operator-configured `dev.mcpg/*` capabilities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, Value>>,
}

/// Tools advertisement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    /// Server emits `notifications/tools/list_changed` when the
    /// catalog changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
    /// SEP-2549 cache advertisement. `Some(...)` indicates the
    /// server emits `ttlMs` / `cacheScope` on `tools/list` results
    /// and supports cache-validation flows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheCapability>,
}

/// Prompts advertisement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptsCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheCapability>,
}

/// Resources advertisement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
    /// Server honors per-resource `resources/updated` subscriptions
    /// established over `subscriptions/listen`. Distinct from the
    /// 2025-11-25 `resources/subscribe` method (removed on this wire);
    /// the flag advertises that the long-lived listen stream will
    /// deliver `notifications/resources/updated` for subscribed URIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheCapability>,
}

/// Argument-completion advertisement. Currently no sub-flags;
/// presence = supported.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompletionCapability {}

/// SEP-2549 cache advertisement marker.
///
/// Today this is intentionally empty — *presence* of the field on a
/// capability declares the server emits `ttlMs` / `cacheScope` on
/// the corresponding list result. Future SEPs may add sub-fields
/// (max ttl, scope policy) without breaking the wire.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CacheCapability {}

// ---------------------------------------------------------------------------
// Client capability advertisement.
// ---------------------------------------------------------------------------

/// Modern client-side capabilities.
///
/// The `inputs` (MRTR — SEP-2322) and `subscriptions`
/// (`subscriptions/listen` — SEP-2575) flags are advertised
/// alongside the corresponding server-side dispatch. The
/// `roots`, `sampling`, and `logging` sub-flags on the legacy
/// `ClientCapabilities` are intentionally absent here per
/// SEP-2577.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// SEP-2133 extensions — reverse-DNS-keyed advertisement
    /// map (e.g., `io.modelcontextprotocol/tasks` if the client
    /// implements the tasks extension; `vendor.example/feature`
    /// for vendor-specific opt-ins).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, Value>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn discover_params_round_trip_minimal() {
        let json = json!({
            "protocolVersion": "2026-07-28",
            "clientInfo": { "name": "test-client", "version": "0.1.0" }
        });
        let params: DiscoverParams = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(params.protocol_version.as_deref(), Some("2026-07-28"));
        let ci = params.client_info.as_ref().expect("clientInfo present");
        assert_eq!(ci.name, "test-client");
        assert_eq!(ci.version, "0.1.0");
        // capabilities defaults to an empty ClientCapabilities.
        assert!(params.capabilities.extensions.is_none());
        // Re-serialize and confirm we lose no required fields.
        let re = serde_json::to_value(&params).unwrap();
        assert_eq!(re["protocolVersion"], "2026-07-28");
        assert_eq!(re["clientInfo"]["name"], "test-client");
    }

    #[test]
    fn discover_params_with_full_implementation_info_and_extensions() {
        let v = json!({
            "protocolVersion": "2026-07-28",
            "clientInfo": {
                "name": "io.example.client",
                "title": "Example Client",
                "version": "1.2.3",
                "description": "An MCP client for examples.",
                "icons": [
                    {
                        "src": "https://example.com/icon.png",
                        "mimeType": "image/png",
                        "sizes": ["48x48"]
                    }
                ],
                "websiteUrl": "https://example.com"
            },
            "capabilities": {
                "extensions": {
                    "io.modelcontextprotocol/tasks": {}
                }
            },
            "_meta": { "io.modelcontextprotocol/traceparent": "00-deadbeef-cafef00d-01" }
        });
        let params: DiscoverParams = serde_json::from_value(v).unwrap();
        let ci = params.client_info.as_ref().expect("clientInfo present");
        assert_eq!(ci.title.as_deref(), Some("Example Client"));
        assert_eq!(ci.icons.as_ref().unwrap().len(), 1);
        assert_eq!(ci.website_url.as_deref(), Some("https://example.com"));
        let ext = params.capabilities.extensions.as_ref().unwrap();
        assert!(ext.contains_key("io.modelcontextprotocol/tasks"));
        assert!(params.meta.is_some());
    }

    #[test]
    fn discover_result_round_trip() {
        let server_info = ImplementationInfo {
            name: "mcpg".to_owned(),
            title: Some("MCP Gateway".to_owned()),
            version: "1.0.0-rc.15".to_owned(),
            description: Some(
                "Cluster-aware MCP gateway with policy / quota / observability plugins.".to_owned(),
            ),
            icons: None,
            website_url: Some("https://github.com/example/mcpg".to_owned()),
        };
        let caps = ServerCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(true),
                cache: Some(CacheCapability {}),
            }),
            prompts: Some(PromptsCapability {
                list_changed: Some(false),
                cache: None,
            }),
            resources: Some(ResourcesCapability {
                list_changed: Some(true),
                subscribe: Some(true),
                cache: Some(CacheCapability {}),
            }),
            completions: Some(CompletionCapability {}),
            extensions: None,
        };
        let r = DiscoverResult {
            result_type: default_result_type_complete(),
            supported_versions: vec!["2026-07-28".to_owned()],
            server_info,
            capabilities: caps,
            instructions: Some("Welcome to MCPG.".to_owned()),
            ttl_ms: 60_000,
            cache_scope: CacheScope::Public,
            meta: None,
        };
        let v = serde_json::to_value(&r).unwrap();
        // CacheableResult envelope (SEP-2549/2322).
        assert_eq!(v["resultType"], "complete");
        assert_eq!(v["ttlMs"], 60_000);
        assert_eq!(v["cacheScope"], "public");
        // The singular `protocolVersion` field is gone (VN-5);
        // discovery advertises `supportedVersions` instead.
        assert!(v.get("protocolVersion").is_none());
        assert_eq!(v["supportedVersions"][0], "2026-07-28");
        // Top-level camelCase rename.
        assert_eq!(v["serverInfo"]["name"], "mcpg");
        assert_eq!(v["serverInfo"]["title"], "MCP Gateway");
        // Capability sub-flags.
        assert_eq!(v["capabilities"]["tools"]["listChanged"], true);
        assert!(v["capabilities"]["tools"]["cache"].is_object());
        assert_eq!(v["capabilities"]["prompts"]["listChanged"], false);
        assert!(v["capabilities"]["prompts"].get("cache").is_none());
        // Completion advertises presence with an empty object under
        // the plural `completions` key (PR-02).
        assert!(v["capabilities"]["completions"].is_object());
        assert!(v["capabilities"].get("completion").is_none());
        assert!(v["capabilities"].get("extensions").is_none());
        assert_eq!(v["instructions"], "Welcome to MCPG.");
        // Round-trip back.
        let back: DiscoverResult = serde_json::from_value(v).unwrap();
        assert_eq!(back.supported_versions[0], "2026-07-28");
    }

    #[test]
    fn server_capabilities_default_omits_every_field() {
        let caps = ServerCapabilities::default();
        let v = serde_json::to_value(&caps).unwrap();
        assert!(v.as_object().unwrap().is_empty(), "got: {v}");
    }

    #[test]
    fn server_capabilities_extensions_round_trip() {
        let mut ext = serde_json::Map::new();
        ext.insert(
            "io.modelcontextprotocol/tasks".to_owned(),
            json!({ "version": "1" }),
        );
        ext.insert("dev.mcpg/idempotency".to_owned(), json!({}));
        let caps = ServerCapabilities {
            extensions: Some(ext),
            ..Default::default()
        };
        let v = serde_json::to_value(&caps).unwrap();
        assert!(v["extensions"]["io.modelcontextprotocol/tasks"]["version"].is_string());
        assert!(v["extensions"]["dev.mcpg/idempotency"].is_object());
        let back: ServerCapabilities = serde_json::from_value(v).unwrap();
        assert!(
            back.extensions
                .as_ref()
                .unwrap()
                .contains_key("dev.mcpg/idempotency")
        );
    }

    #[test]
    fn implementation_info_minimal_omits_optional_fields() {
        let info = ImplementationInfo {
            name: "x".to_owned(),
            version: "0".to_owned(),
            ..Default::default()
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["name"], "x");
        assert_eq!(v["version"], "0");
        assert!(v.get("title").is_none());
        assert!(v.get("description").is_none());
        assert!(v.get("icons").is_none());
        assert!(v.get("websiteUrl").is_none());
    }

    #[test]
    fn method_constant_matches_spec() {
        assert_eq!(METHOD_SERVER_DISCOVER, "server/discover");
    }
}
