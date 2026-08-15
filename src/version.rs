//! `ProtocolVersion` — the typed identifier the gateway routes against
//! when selecting a [`ProtocolHandler`].
//!
//! Wire-string constants and per-version capability shape live in the
//! per-version submodules (`protocol/v_<date>/`). This module is the
//! single place where new revisions are enumerated.
//!
//! ### Naming
//!
//! Variants use the spec's revision-date format with underscores
//! (`V_2025_11_25`) so they are readable in code review and match the
//! folder structure under `protocol/v_2025_11_25/`. The
//! `non_camel_case_types` lint is allowed for this enum specifically;
//! every other type in the gateway follows the usual Rust conventions.

use serde::{Deserialize, Serialize};

/// MCP protocol revisions MCPG knows about.
///
/// Variants are ordered chronologically (oldest first). Ordering is
/// informational — version selection at runtime is by set membership in
/// the [`crate::registry::ProtocolRegistry`], not by ordinal.
#[allow(
    non_camel_case_types,
    reason = "date-based variants are deliberately readable in this format"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolVersion {
    /// MCP revision 2025-11-25 — current production-grade revision.
    /// MCPG's default for new deployments until the
    /// [`ProtocolRegistry::COMPILE_TIME_DEFAULT`] constant is flipped.
    ///
    /// [`ProtocolRegistry::COMPILE_TIME_DEFAULT`]: crate::registry::ProtocolRegistry::COMPILE_TIME_DEFAULT
    V_2025_11_25,
    /// MCP revision `2026-07-28` — the stateless, MRTR-based modern
    /// revision (capability discovery, the cached list surface, modern
    /// dispatch, MRTR, and the tasks extension).
    ///
    /// The wire string is the final published value `"2026-07-28"`.
    /// `parse()` additionally accepts the pre-final `"DRAFT-2026-v1"`
    /// label as a transitional inbound alias.
    V_2026_07_28,
}

impl ProtocolVersion {
    /// Wire-string identifier (the spec's revision date).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V_2025_11_25 => "2025-11-25",
            Self::V_2026_07_28 => "2026-07-28",
        }
    }

    /// Parse the spec's wire-string identifier into a [`ProtocolVersion`].
    /// Returns `None` for unknown values; the caller is responsible for
    /// minting an `UnsupportedProtocolVersionError` response.
    ///
    /// Accepts both `"DRAFT-2026-v1"` and `"2026-07-28"` for the
    /// modern revision so clients pinning either string get the
    /// same handler.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "2025-11-25" => Some(Self::V_2025_11_25),
            "DRAFT-2026-v1" | "2026-07-28" => Some(Self::V_2026_07_28),
            _ => None,
        }
    }

    /// True iff this version requires the legacy `initialize` /
    /// session lifecycle. The modern revision is stateless
    /// (SEP-2575 + SEP-2567) and skips the handshake.
    pub fn requires_session(&self) -> bool {
        match self {
            Self::V_2025_11_25 => true,
            Self::V_2026_07_28 => false,
        }
    }

    /// True iff this version's HTTP transport must carry the
    /// `Mcp-Method` / `Mcp-Name` / `Mcp-Param-{Name}` routing headers
    /// (SEP-2243).
    pub fn header_routing_required(&self) -> bool {
        match self {
            Self::V_2025_11_25 => false,
            Self::V_2026_07_28 => true,
        }
    }

    /// True iff this version supports the SEP-2133 extensions
    /// framework (per-request capabilities map with reverse-DNS keys).
    pub fn supports_extensions(&self) -> bool {
        match self {
            // The `dev.mcpg/idempotency` advertisement is plumbed
            // today; SEP-2133 itself is a 2026-07-28 addition.
            Self::V_2025_11_25 => false,
            Self::V_2026_07_28 => true,
        }
    }
}

impl std::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_unknown_returns_none() {
        assert!(ProtocolVersion::parse("1900-01-01").is_none());
        assert!(ProtocolVersion::parse("").is_none());
        assert!(ProtocolVersion::parse("not-a-version").is_none());
    }

    #[test]
    fn parse_rejects_retired_legacy_strings() {
        // These revision strings no longer resolve to a routable
        // handler and are rejected at parse time.
        assert!(ProtocolVersion::parse("2025-03-26").is_none());
        assert!(ProtocolVersion::parse("2025-06-18").is_none());
    }

    #[test]
    fn parse_accepts_both_modern_strings() {
        // Operators on the RC pin "DRAFT-2026-v1"; clients on the
        // final spec pin "2026-07-28". Both must resolve.
        assert_eq!(
            ProtocolVersion::parse("DRAFT-2026-v1"),
            Some(ProtocolVersion::V_2026_07_28)
        );
        assert_eq!(
            ProtocolVersion::parse("2026-07-28"),
            Some(ProtocolVersion::V_2026_07_28)
        );
    }

    #[test]
    fn display_matches_wire_string() {
        assert_eq!(ProtocolVersion::V_2025_11_25.to_string(), "2025-11-25");
        assert_eq!(ProtocolVersion::V_2026_07_28.to_string(), "2026-07-28");
    }

    #[test]
    fn session_version_has_legacy_capability_shape() {
        let v = ProtocolVersion::V_2025_11_25;
        assert!(v.requires_session(), "{v} should require session");
        assert!(
            !v.header_routing_required(),
            "{v} should not require SEP-2243 header routing"
        );
        assert!(!v.supports_extensions(), "{v} should not support SEP-2133");
    }

    #[test]
    fn modern_version_is_stateless_and_uses_sep_2243() {
        let v = ProtocolVersion::V_2026_07_28;
        assert!(!v.requires_session(), "modern revision is stateless");
        assert!(
            v.header_routing_required(),
            "modern revision uses SEP-2243 routing headers"
        );
        assert!(v.supports_extensions(), "modern revision supports SEP-2133");
    }

    #[test]
    fn serde_round_trip_uses_kebab_case() {
        // Wire format is the same dash-separated date string used as
        // a JSON enum tag — operator configs reference it directly.
        let json = serde_json::to_string(&ProtocolVersion::V_2025_11_25).unwrap();
        assert_eq!(json, "\"v-2025-11-25\"");
        let back: ProtocolVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ProtocolVersion::V_2025_11_25);
    }
}
