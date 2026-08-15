//! SEP-2596 feature-lifecycle deprecation advertisements + usage metering.
//!
//! The `2026-07-28` revision marks Roots, Sampling, and Logging
//! Deprecated (SEP-2577) under the SEP-2596 feature-lifecycle policy
//! (Active → Deprecated → Removed, ≥12-month window). The lifecycle
//! policy itself does not define a *wire* advertisement shape — the
//! canonical signal is the `@deprecated` JSDoc tag in `schema.ts` plus
//! the deprecated registry. To give clients a runtime signal without
//! forging the reserved `io.modelcontextprotocol/*` namespace, the
//! gateway surfaces a vendor-namespaced advisory under the
//! `server/discover` result `_meta` (`dev.mcpg/deprecations`). Clients
//! that don't understand the key ignore it; clients that do can warn or
//! steer migration.
//!
//! Usage of each deprecated feature on the modern wire is metered with
//! a counter so an operator can see migration pressure before the
//! removal window closes.

/// `_meta` key under which `server/discover` advertises the deprecated
/// features. Vendor-namespaced (reverse-DNS `dev.mcpg`) so it does not
/// collide with — or forge — the reserved `io.modelcontextprotocol/*`
/// grammar.
pub const DEPRECATIONS_META_KEY: &str = "dev.mcpg/deprecations";

/// Feature names matching the deprecated registry entries.
pub const FEATURE_ROOTS: &str = "roots";
pub const FEATURE_SAMPLING: &str = "sampling";
pub const FEATURE_LOGGING: &str = "logging";

/// Meter one use of a deprecated feature on the modern wire. `feature`
/// is one of the `FEATURE_*` constants. Emits
/// `mcpg_deprecated_feature_used_total{feature=...}` so operators can
/// track migration pressure during the deprecation window.
pub fn meter_deprecated_feature(feature: &'static str) {
    metrics::counter!(
        "mcpg_deprecated_feature_used_total",
        "feature" => feature,
    )
    .increment(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_key_is_vendor_namespaced_not_reserved() {
        // Must NOT forge the reserved `io.modelcontextprotocol/*` grammar.
        assert!(DEPRECATIONS_META_KEY.starts_with("dev.mcpg/"));
        assert!(!DEPRECATIONS_META_KEY.starts_with("io.modelcontextprotocol/"));
    }
}
