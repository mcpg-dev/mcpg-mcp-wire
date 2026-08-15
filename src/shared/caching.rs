//! SEP-2549 caching hints — version-agnostic.
//!
//! `ttlMs` + `cacheScope` are response-envelope hints that tell a
//! client how long it may cache a cacheable result (`tools/list`,
//! `prompts/list`, `resources/list`, `resources/templates/list`,
//! `resources/read`, `server/discover`) and whether the entry is
//! shareable across identities. On the modern (`DRAFT-2026-v1`) wire
//! they are **required** fields of `CacheableResult`; on the legacy
//! (`2025-11-25`) wire they are emitted as optional, ignorable fields
//! for forward-compatible clients.
//!
//! The hints live here, not under a single protocol revision, because
//! they're a cross-cutting caching concern rather than a protocol
//! shape. Both `ProtocolHandler` impls draw `CacheScope` from this
//! module so the vocabulary can't drift between versions.

use serde::{Deserialize, Serialize};

/// SEP-2549 cache scope. The wire vocabulary is the two-value
/// `public` / `private` enum the spec ratified. `Public` shares one
/// cache entry across the deployment; `Private` is bucketed per
/// caller identity (the gateway treats per-client and per-tenant as
/// equivalent for spec purposes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheScope {
    /// Identical for every client of this server — clients SHOULD
    /// share the cache entry.
    Public,
    /// Bucketed by the caller's identity (per-client or per-tenant).
    /// Clients MUST NOT share cache entries across identities.
    Private,
}

/// Default TTL (ms) advertised on catalog list results when an
/// operator hasn't configured a per-surface override. One minute is
/// conservative: long enough to spare a chatty client repeated
/// re-list round-trips, short enough that a hot catalog mutation
/// (tool/prompt/resource added or removed) is picked up promptly.
pub const DEFAULT_LIST_TTL_MS: u64 = 60_000;

/// Default TTL (ms) advertised on `resources/read` results. Resource
/// bodies churn more than the catalog shape, so the read TTL is
/// shorter than the list TTL.
pub const DEFAULT_READ_TTL_MS: u64 = 30_000;

/// SEP-2322 `resultType` discriminator for a result that ran to
/// completion. Every modern complete-path result carries this value;
/// the suspended (`"input_required"`) and task (`"task"`) shapes use
/// their own discriminators.
pub const RESULT_TYPE_COMPLETE: &str = "complete";

/// `#[serde(default = ...)]` helper so a `CacheableResult`'s
/// `resultType` field is never accidentally omitted on the wire.
pub fn default_result_type_complete() -> String {
    RESULT_TYPE_COMPLETE.to_owned()
}
