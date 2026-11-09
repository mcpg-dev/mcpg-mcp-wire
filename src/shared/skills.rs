//! Version-agnostic wire of the **Skills extension** (SEP-2640,
//! `io.modelcontextprotocol/skills`).
//!
//! A skill is a directory of files with a `SKILL.md` at its root, served
//! as MCP resources under `skill://<skill-path>/<file-path>`. The
//! extension adds three methods on top of `resources/read`:
//!
//! - `skills/list` — the skills a server serves, each entry a complete
//!   manifest (verbatim frontmatter plus every file's digest and size);
//! - `skills/get` — one entry by its `SKILL.md` URI;
//! - `resources/directory/read` — the direct children of a directory
//!   resource, offered when the server declares `directoryRead: true`.
//!
//! Both wire revisions route the same three methods to
//! [`SkillsOperation`]; whether a gateway serves them is the dispatcher's
//! decision, so params that do not parse become
//! [`SkillsOperation::InvalidParams`] rather than a routing error.

use serde::{Deserialize, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::descriptors::ResourceDescriptor;
use crate::shared::error::ProtocolError;
use crate::shared::jsonrpc::JsonRpcRequest;
use crate::shared::routing as shared_routing;

/// The extension's identifier in `capabilities.extensions`.
pub const EXTENSION_ID: &str = "io.modelcontextprotocol/skills";
/// Enumerates the skills a server serves.
pub const METHOD_LIST_SKILLS: &str = "skills/list";
/// Returns one skill's entry by the URI of its `SKILL.md`.
pub const METHOD_GET_SKILL: &str = "skills/get";
/// Lists the direct children of a directory resource.
pub const METHOD_READ_DIRECTORY: &str = "resources/directory/read";

/// The URI scheme skills are served under.
pub const SKILL_SCHEME: &str = "skill";
/// The file every skill holds at its root.
pub const SKILL_FILE: &str = "SKILL.md";
/// `mimeType` of a `SKILL.md`.
pub const SKILL_MIME_TYPE: &str = "text/markdown";
/// `mimeType` of a directory resource.
pub const DIRECTORY_MIME_TYPE: &str = "inode/directory";
/// The `resources` marker of a skill whose content is generated.
pub const RESOURCES_DYNAMIC: &str = "dynamic";
/// The `_meta` key prefix the extension reserves on skill resources.
pub const META_PREFIX: &str = "io.modelcontextprotocol.skills/";

/// The most files one skill may list and still load on every host.
pub const MAX_RESOURCES_PER_SKILL: usize = 512;
/// The most bytes one skill's files may total and still load on every host.
pub const MAX_BYTES_PER_SKILL: u64 = 16 * 1024 * 1024;

/// What a server declares under [`EXTENSION_ID`].
pub fn capability_value(directory_read: bool) -> Value {
    if directory_read {
        serde_json::json!({ "directoryRead": true })
    } else {
        serde_json::json!({})
    }
}

/// `skills/list` params.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ListSkillsParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// `skills/get` params.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GetSkillParams {
    /// The URI of the skill's `SKILL.md`.
    pub uri: String,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// `resources/directory/read` params.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReadDirectoryParams {
    /// The directory's URI, without a trailing slash.
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// One file of a skill, as its entry lists it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillFile {
    pub uri: String,
    /// `sha256:` and 64 lowercase hex digits of the file's raw bytes.
    pub digest: String,
    /// The length in bytes of the file's raw content.
    pub size: u64,
}

/// A skill's files: every one listed with its digest, or `"dynamic"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillResources {
    Listed(Vec<SkillFile>),
    Dynamic,
}

impl Serialize for SkillResources {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Listed(files) => files.serialize(serializer),
            Self::Dynamic => serializer.serialize_str(RESOURCES_DYNAMIC),
        }
    }
}

/// One skill entry of `skills/list` and `skills/get`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SkillEntry {
    /// The URI of the skill's `SKILL.md`.
    pub uri: String,
    /// The `SKILL.md` frontmatter, verbatim, as JSON.
    pub frontmatter: Map<String, Value>,
    pub resources: SkillResources,
}

/// `skills/list` result. The modern wire fills `result_type` and the
/// SEP-2549 caching fields; the legacy wire leaves them out.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ListSkillsResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_type: Option<String>,
    pub skills: Vec<SkillEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_scope: Option<String>,
}

/// `skills/get` result.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GetSkillResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_type: Option<String>,
    pub skill: SkillEntry,
}

/// `resources/directory/read` result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadDirectoryResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_type: Option<String>,
    pub resources: Vec<ResourceDescriptor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// A routed Skills-extension request.
#[derive(Debug, Clone, Serialize)]
pub enum SkillsOperation {
    List {
        request_id: Value,
        params: ListSkillsParams,
    },
    Get {
        request_id: Value,
        params: GetSkillParams,
    },
    ReadDirectory {
        request_id: Value,
        params: ReadDirectoryParams,
    },
    /// A routed request whose params do not parse. While the gateway
    /// serves no skills the answer is the `-32601` of any unknown method,
    /// not a `-32602` that reveals the method exists.
    InvalidParams {
        request_id: Value,
        method: &'static str,
        message: String,
    },
}

impl SkillsOperation {
    /// The JSON-RPC `id` of the request.
    pub fn request_id(&self) -> &Value {
        match self {
            Self::List { request_id, .. }
            | Self::Get { request_id, .. }
            | Self::ReadDirectory { request_id, .. }
            | Self::InvalidParams { request_id, .. } => request_id,
        }
    }

    /// The JSON-RPC method this operation was routed from.
    pub fn method(&self) -> &'static str {
        match self {
            Self::List { .. } => METHOD_LIST_SKILLS,
            Self::Get { .. } => METHOD_GET_SKILL,
            Self::ReadDirectory { .. } => METHOD_READ_DIRECTORY,
            Self::InvalidParams { method, .. } => method,
        }
    }

    /// The metrics label, one per method.
    pub fn label(&self) -> &'static str {
        match self.method() {
            METHOD_LIST_SKILLS => "skills_ext.list",
            METHOD_GET_SKILL => "skills_ext.get",
            METHOD_READ_DIRECTORY => "skills_ext.read_directory",
            _ => "skills_ext.unknown",
        }
    }
}

/// Whether `method` is one of the extension's three methods.
pub fn is_skills_method(method: &str) -> bool {
    matches!(
        method,
        METHOD_LIST_SKILLS | METHOD_GET_SKILL | METHOD_READ_DIRECTORY
    )
}

/// Route a request whose method [`is_skills_method`].
pub fn route_request(request: &JsonRpcRequest) -> SkillsOperation {
    let request_id = request.id.clone();
    let invalid = |method: &'static str, error: ProtocolError| SkillsOperation::InvalidParams {
        request_id: request.id.clone(),
        method,
        message: error.message().to_owned(),
    };
    match request.method.as_str() {
        METHOD_LIST_SKILLS => {
            match shared_routing::optional_params::<ListSkillsParams>(request, METHOD_LIST_SKILLS) {
                Ok(params) => SkillsOperation::List { request_id, params },
                Err(error) => invalid(METHOD_LIST_SKILLS, error),
            }
        }
        METHOD_GET_SKILL => {
            match shared_routing::required_params::<GetSkillParams>(request, METHOD_GET_SKILL) {
                Ok(params) => SkillsOperation::Get { request_id, params },
                Err(error) => invalid(METHOD_GET_SKILL, error),
            }
        }
        _ => match shared_routing::required_params::<ReadDirectoryParams>(
            request,
            METHOD_READ_DIRECTORY,
        ) {
            Ok(params) => SkillsOperation::ReadDirectory { request_id, params },
            Err(error) => invalid(METHOD_READ_DIRECTORY, error),
        },
    }
}

/// `sha256:{hex}` of `bytes`' digest, given as 32 raw bytes.
pub fn format_digest(sha256: &[u8]) -> String {
    let mut out = String::with_capacity(7 + sha256.len() * 2);
    out.push_str("sha256:");
    for byte in sha256 {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Whether `name` meets the Agent Skills naming rules: 1 to 64 characters
/// of lowercase ASCII letters, digits and single hyphens, with no hyphen
/// at either end.
pub fn is_valid_skill_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(method: &str, params: Option<Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_owned(),
            id: json!(4),
            method: method.to_owned(),
            params,
        }
    }

    #[test]
    fn routes_each_method_with_its_params() {
        assert!(matches!(
            route_request(&request(METHOD_LIST_SKILLS, None)),
            SkillsOperation::List { params, .. } if params.cursor.is_none()
        ));
        assert!(matches!(
            route_request(&request(METHOD_LIST_SKILLS, Some(json!({ "cursor": "c" })))),
            SkillsOperation::List { params, .. } if params.cursor.as_deref() == Some("c")
        ));
        assert!(matches!(
            route_request(&request(
                METHOD_GET_SKILL,
                Some(json!({ "uri": "skill://a/SKILL.md" }))
            )),
            SkillsOperation::Get { params, .. } if params.uri == "skill://a/SKILL.md"
        ));
        assert!(matches!(
            route_request(&request(
                METHOD_READ_DIRECTORY,
                Some(json!({ "uri": "skill://a", "cursor": "x" }))
            )),
            SkillsOperation::ReadDirectory { params, .. }
                if params.uri == "skill://a" && params.cursor.as_deref() == Some("x")
        ));
    }

    #[test]
    fn params_that_do_not_parse_become_an_operation() {
        let op = route_request(&request(METHOD_GET_SKILL, None));
        assert!(
            matches!(op, SkillsOperation::InvalidParams { method, .. } if method == METHOD_GET_SKILL)
        );
        assert_eq!(op.label(), "skills_ext.get");
        assert_eq!(op.request_id(), &json!(4));
        let op = route_request(&request(METHOD_READ_DIRECTORY, Some(json!({ "uri": 3 }))));
        assert!(matches!(op, SkillsOperation::InvalidParams { .. }));
    }

    #[test]
    fn an_entry_serializes_as_the_extension_defines_it() {
        let entry = SkillEntry {
            uri: "skill://a/SKILL.md".to_owned(),
            frontmatter: json!({ "name": "a", "description": "d" })
                .as_object()
                .cloned()
                .unwrap(),
            resources: SkillResources::Listed(vec![SkillFile {
                uri: "skill://a/SKILL.md".to_owned(),
                digest: format_digest(&[0xab; 32]),
                size: 12,
            }]),
        };
        let listed = ListSkillsResult {
            result_type: Some("complete".to_owned()),
            skills: vec![entry.clone()],
            next_cursor: None,
            ttl_ms: Some(30_000),
            cache_scope: Some("private".to_owned()),
        };
        assert_eq!(
            serde_json::to_value(&listed).unwrap(),
            json!({
                "resultType": "complete",
                "skills": [{
                    "uri": "skill://a/SKILL.md",
                    "frontmatter": { "name": "a", "description": "d" },
                    "resources": [{
                        "uri": "skill://a/SKILL.md",
                        "digest": format!("sha256:{}", "ab".repeat(32)),
                        "size": 12
                    }]
                }],
                "ttlMs": 30000,
                "cacheScope": "private"
            })
        );
        let dynamic = SkillEntry {
            resources: SkillResources::Dynamic,
            ..entry
        };
        assert_eq!(
            serde_json::to_value(&dynamic).unwrap()["resources"],
            "dynamic"
        );
    }

    #[test]
    fn the_capability_names_directory_read_only_when_served() {
        assert_eq!(capability_value(true), json!({ "directoryRead": true }));
        assert_eq!(capability_value(false), json!({}));
    }

    #[test]
    fn skill_names_follow_the_agent_skills_rules() {
        for good in ["a", "git-workflow", "pdf2-tools", &"x".repeat(64)] {
            assert!(is_valid_skill_name(good), "{good}");
        }
        for bad in ["", "-a", "a-", "a--b", "Git", "a_b", "a.b", &"x".repeat(65)] {
            assert!(!is_valid_skill_name(bad), "{bad}");
        }
    }
}
