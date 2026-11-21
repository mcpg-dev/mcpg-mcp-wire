//! Client capability declarations, as a request carries them in
//! `_meta.io.modelcontextprotocol/clientCapabilities` (SEP-2575).
//!
//! The gateway reads them to decide which input requests a suspended
//! pipeline may put to the client (`sampling`, `elicitation`,
//! `roots`) and which extensions the client opted into (`extensions`,
//! for example the tasks extension).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Client-declared capability flags. A pipeline step that requires
/// sampling fails when the client did not declare `sampling: {}`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub roots: Option<ClientRootsCapability>,
    #[serde(default)]
    pub sampling: Option<ClientSamplingCapability>,
    #[serde(default)]
    pub elicitation: Option<ClientElicitationCapability>,
    #[serde(default)]
    pub experimental: Option<Value>,
    /// SEP-2133 reverse-DNS-keyed extension declarations. The gateway
    /// reads it to gate the tasks extension (SEP-2663).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<serde_json::Map<String, Value>>,
}

/// Presence marker of a capability sub-flag: `{}`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityFlag {}

/// `capabilities.roots` declaration from the client.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientRootsCapability {
    /// The `listChanged` flag as the client declares it.
    #[serde(
        default,
        rename = "listChanged",
        skip_serializing_if = "Option::is_none"
    )]
    pub list_changed: Option<bool>,
}

/// `capabilities.sampling` declaration from the client.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientSamplingCapability {
    /// Client accepts tool-enabled `sampling/createMessage` requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<CapabilityFlag>,
    /// Client accepts `includeContext` on sampling requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<CapabilityFlag>,
}

/// `capabilities.elicitation` declaration from the client.
///
/// A bare `"elicitation": {}` means the client supports form-mode
/// elicitation; explicit sub-keys opt into specific modes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClientElicitationCapability {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form: Option<CapabilityFlag>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<CapabilityFlag>,
}

impl ClientCapabilities {
    /// The settings the client declares for extension `extension_id`, when
    /// it declares the extension at a revision this build implements
    /// ([`crate::shared::extension_revision`]).
    pub fn supported_extension(&self, extension_id: &str) -> Option<&Value> {
        crate::shared::extension_revision::supported_extension(
            self.extensions.as_ref(),
            extension_id,
        )
    }

    /// Check if the client supports sampling with tools.
    /// Per spec: client must declare `{ sampling: { tools: {} } }`.
    pub fn supports_sampling_tools(&self) -> bool {
        self.sampling
            .as_ref()
            .and_then(|s| s.tools.as_ref())
            .is_some()
    }

    /// Check if the client supports sampling context inclusion.
    pub fn supports_sampling_context(&self) -> bool {
        self.sampling
            .as_ref()
            .and_then(|s| s.context.as_ref())
            .is_some()
    }

    /// Client supports sampling at all (advertised `sampling: {...}`).
    pub fn supports_sampling(&self) -> bool {
        self.sampling.is_some()
    }

    /// Client supports roots (advertised `roots: {...}`).
    pub fn supports_roots(&self) -> bool {
        self.roots.is_some()
    }

    /// Client supports elicitation at all (advertised
    /// `elicitation: {...}`). A bare `{}` implies form mode.
    pub fn supports_elicitation(&self) -> bool {
        self.elicitation.is_some()
    }

    /// Check if the client supports form-based elicitation.
    ///
    /// An elicitation capability with no sub-keys (`"elicitation": {}`)
    /// implies form-mode support, and an explicit `form: {}` sub-key
    /// keeps that signal. Only a client that declares other modes
    /// (e.g. `url: {}`) without `form` opts out of form mode.
    pub fn supports_elicitation_form(&self) -> bool {
        let Some(elicit) = self.elicitation.as_ref() else {
            return false;
        };
        if elicit.form.is_some() {
            return true;
        }
        elicit.url.is_none()
    }

    /// Check if the client supports URL-based elicitation.
    pub fn supports_elicitation_url(&self) -> bool {
        self.elicitation
            .as_ref()
            .and_then(|s| s.url.as_ref())
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn caps(v: Value) -> ClientCapabilities {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn empty_elicitation_capability_implies_form_support() {
        let caps = caps(json!({ "elicitation": {} }));
        assert!(caps.supports_elicitation());
        assert!(caps.supports_elicitation_form());
        assert!(!caps.supports_elicitation_url());
    }

    #[test]
    fn url_only_elicitation_capability_does_not_imply_form() {
        let caps = caps(json!({ "elicitation": { "url": {} } }));
        assert!(caps.supports_elicitation_url());
        assert!(!caps.supports_elicitation_form());
    }

    #[test]
    fn explicit_form_and_url_declare_both_modes() {
        let caps = caps(json!({ "elicitation": { "form": {}, "url": {} } }));
        assert!(caps.supports_elicitation_form());
        assert!(caps.supports_elicitation_url());
    }

    #[test]
    fn no_elicitation_capability_supports_no_mode() {
        let caps = caps(json!({}));
        assert!(!caps.supports_elicitation());
        assert!(!caps.supports_elicitation_form());
        assert!(!caps.supports_elicitation_url());
    }

    #[test]
    fn sampling_sub_flags_are_read() {
        let caps = caps(json!({ "sampling": { "tools": {} } }));
        assert!(caps.supports_sampling());
        assert!(caps.supports_sampling_tools());
        assert!(!caps.supports_sampling_context());

        let bare = super::ClientCapabilities::default();
        assert!(!bare.supports_sampling());
        assert!(!bare.supports_sampling_tools());
    }

    #[test]
    fn roots_is_read() {
        let caps = caps(json!({ "roots": { "listChanged": true } }));
        assert!(caps.supports_roots());

        let bare_roots = super::ClientCapabilities {
            roots: Some(ClientRootsCapability::default()),
            ..Default::default()
        };
        assert!(bare_roots.supports_roots());
    }

    #[test]
    fn extensions_are_kept_and_skipped_when_absent() {
        let caps = caps(json!({
            "extensions": { "io.modelcontextprotocol/tasks": {} }
        }));
        assert!(
            caps.extensions
                .as_ref()
                .unwrap()
                .contains_key("io.modelcontextprotocol/tasks")
        );

        let v = serde_json::to_value(super::ClientCapabilities::default()).unwrap();
        assert!(v.get("extensions").is_none());
    }

    #[test]
    fn an_extension_at_an_unknown_revision_is_not_supported() {
        let caps = caps(json!({
            "extensions": {
                "io.modelcontextprotocol/tasks": {},
                "io.modelcontextprotocol/events": { "revision": "2" },
            }
        }));
        assert!(
            caps.supported_extension("io.modelcontextprotocol/tasks")
                .is_some()
        );
        assert!(
            caps.supported_extension("io.modelcontextprotocol/events")
                .is_none()
        );
        assert!(
            super::ClientCapabilities::default()
                .supported_extension("io.modelcontextprotocol/tasks")
                .is_none()
        );
    }

    #[test]
    fn serialized_shape_is_stable() {
        let caps = super::ClientCapabilities {
            sampling: Some(ClientSamplingCapability {
                tools: Some(CapabilityFlag {}),
                context: None,
            }),
            elicitation: Some(ClientElicitationCapability::default()),
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&caps).unwrap(),
            json!({
                "roots": null,
                "sampling": { "tools": {} },
                "elicitation": {},
                "experimental": null
            })
        );
    }
}
