//! Extension revisions: which revision of an extension a peer declares,
//! and whether this build may enable the extension with that peer.
//!
//! An extension keeps its identifier through every stage. A breaking
//! change adds or increments a `revision` string in the extension's
//! settings object; an absent `revision` names the initial revision. A
//! peer that declares a `revision` this build does not implement does
//! not get the extension.
//!
//! [`IMPLEMENTED_EXTENSIONS`] is the one list of the revisions this build
//! implements. This build implements the initial revision of each, so it
//! declares no `revision` in its own settings.

use serde_json::{Map, Value};

/// The settings key that names an extension's revision.
pub const REVISION_KEY: &str = "revision";

/// An extension this build implements, and its revisions beyond the
/// initial one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImplementedExtension {
    /// The extension identifier.
    pub id: &'static str,
    /// The `revision` strings this build implements. The initial revision
    /// has no string and is always implemented, so an empty list means
    /// the initial revision only.
    pub revisions: &'static [&'static str],
}

/// The extensions whose revision this build checks, with the revisions it
/// implements.
pub const IMPLEMENTED_EXTENSIONS: &[ImplementedExtension] = &[
    ImplementedExtension {
        id: crate::shared::skills::EXTENSION_ID,
        revisions: &[],
    },
    ImplementedExtension {
        id: crate::v_2026_07_28::extensions::tasks::wire::EXTENSION_NAMESPACE,
        revisions: &[],
    },
    ImplementedExtension {
        id: crate::v_2026_07_28::extensions::events::wire::EXTENSION_ID,
        revisions: &[],
    },
];

/// The revision a peer declares in an extension's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerRevision<'a> {
    /// No `revision`: the initial revision.
    Initial,
    /// A `revision` this build implements.
    Known(&'a str),
    /// A `revision` this build does not implement, or one that is not a
    /// string.
    Unknown,
}

impl PeerRevision<'_> {
    /// Whether this build may enable the extension with the peer.
    pub fn is_supported(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// The revisions this build implements for `extension_id`; empty for the
/// initial revision only, and for an extension not in
/// [`IMPLEMENTED_EXTENSIONS`].
pub fn implemented_revisions(extension_id: &str) -> &'static [&'static str] {
    IMPLEMENTED_EXTENSIONS
        .iter()
        .find(|extension| extension.id == extension_id)
        .map_or(&[], |extension| extension.revisions)
}

/// The revision `settings`, a peer's settings object for `extension_id`,
/// declares. A settings value that is not an object carries no revision.
pub fn peer_revision<'a>(extension_id: &str, settings: &'a Value) -> PeerRevision<'a> {
    match settings.get(REVISION_KEY) {
        None => PeerRevision::Initial,
        Some(Value::String(revision))
            if implemented_revisions(extension_id).contains(&revision.as_str()) =>
        {
            PeerRevision::Known(revision)
        }
        Some(_) => PeerRevision::Unknown,
    }
}

/// The settings a peer's `extensions` map declares for `extension_id`,
/// when it declares the extension at a revision this build implements.
/// `None` when the map is absent, the extension is not declared, or its
/// revision is unknown.
pub fn supported_extension<'a>(
    extensions: Option<&'a Map<String, Value>>,
    extension_id: &str,
) -> Option<&'a Value> {
    extensions?
        .get(extension_id)
        .filter(|settings| peer_revision(extension_id, settings).is_supported())
}

/// [`supported_extension`] for a whole capabilities object, as a server
/// advertises it (`capabilities.extensions[extension_id]`).
pub fn supported_extension_in<'a>(
    capabilities: &'a Value,
    extension_id: &str,
) -> Option<&'a Value> {
    supported_extension(
        capabilities.get("extensions").and_then(Value::as_object),
        extension_id,
    )
}

/// Whether a capabilities object that may declare `extension_id` declares
/// no revision this build does not implement: `true` when the extension is
/// not in its extensions map at all.
pub fn revision_is_supported_in(capabilities: &Value, extension_id: &str) -> bool {
    capabilities
        .get("extensions")
        .and_then(|extensions| extensions.get(extension_id))
        .is_none_or(|settings| peer_revision(extension_id, settings).is_supported())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TASKS: &str = "io.modelcontextprotocol/tasks";

    #[test]
    fn an_absent_revision_is_the_initial_one() {
        assert_eq!(peer_revision(TASKS, &json!({})), PeerRevision::Initial);
        assert_eq!(peer_revision(TASKS, &json!(true)), PeerRevision::Initial);
        assert!(PeerRevision::Initial.is_supported());
    }

    #[test]
    fn an_unknown_or_malformed_revision_is_not_supported() {
        for settings in [
            json!({ "revision": "2" }),
            json!({ "revision": "" }),
            json!({ "revision": 2 }),
            json!({ "revision": null }),
        ] {
            assert_eq!(
                peer_revision(TASKS, &settings),
                PeerRevision::Unknown,
                "{settings}"
            );
        }
    }

    #[test]
    fn this_build_implements_the_initial_revision_of_each_extension() {
        let ids: Vec<&str> = IMPLEMENTED_EXTENSIONS.iter().map(|e| e.id).collect();
        assert_eq!(
            ids,
            [
                "io.modelcontextprotocol/skills",
                "io.modelcontextprotocol/tasks",
                "io.modelcontextprotocol/events",
            ]
        );
        for extension in IMPLEMENTED_EXTENSIONS {
            assert!(extension.revisions.is_empty(), "{}", extension.id);
        }
        assert!(implemented_revisions("example.com/other").is_empty());
    }

    #[test]
    fn a_declaration_is_supported_only_at_a_known_revision() {
        let extensions = json!({
            "io.modelcontextprotocol/tasks": {},
            "io.modelcontextprotocol/events": { "revision": "3" },
        });
        let map = extensions.as_object();
        assert_eq!(supported_extension(map, TASKS), Some(&json!({})));
        assert_eq!(
            supported_extension(map, "io.modelcontextprotocol/events"),
            None
        );
        assert_eq!(
            supported_extension(map, "io.modelcontextprotocol/skills"),
            None
        );
        assert_eq!(supported_extension(None, TASKS), None);
    }

    #[test]
    fn a_server_capabilities_object_is_read_the_same_way() {
        let capabilities = json!({
            "extensions": {
                "io.modelcontextprotocol/skills": { "directoryRead": true },
                "io.modelcontextprotocol/events": { "revision": "next" },
            }
        });
        assert_eq!(
            supported_extension_in(&capabilities, "io.modelcontextprotocol/skills"),
            Some(&json!({ "directoryRead": true }))
        );
        assert!(revision_is_supported_in(
            &capabilities,
            "io.modelcontextprotocol/skills"
        ));
        assert!(!revision_is_supported_in(
            &capabilities,
            "io.modelcontextprotocol/events"
        ));
        assert!(revision_is_supported_in(&capabilities, TASKS));
        assert!(revision_is_supported_in(&json!({}), TASKS));
    }
}
