//! SEP-2133 extension surfaces published by `mcpg` on the modern
//! wire:
//!
//! - [`tasks`](crate::v_2026_07_28::extensions::tasks)
//!   (SEP-2663) — the post-2025-11-25 home of the legacy core
//!   `tasks/*` methods.
//! - [`apps`](crate::v_2026_07_28::extensions::apps)
//!   (SEP-1865) — MCP Apps. No dispatch arm (the `ui/*` protocol is
//!   host↔iframe over postMessage); only the typed `_meta.ui`
//!   projection lives here.
//!
//! Future extensions register their wire types (and, if they add
//! MCP-wire methods, their dispatch arms) here as their own modules.

pub mod apps;
pub mod tasks;
