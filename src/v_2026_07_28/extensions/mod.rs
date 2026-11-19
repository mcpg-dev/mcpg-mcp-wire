//! SEP-2133 extension surfaces published by `mcpg`:
//!
//! - [`tasks`](crate::v_2026_07_28::extensions::tasks)
//!   (SEP-2663) — long-running async work: `tasks/{get,update,cancel}`
//!   and the `notifications/tasks` status notification.
//! - [`apps`](crate::v_2026_07_28::extensions::apps)
//!   (SEP-1865) — MCP Apps. No dispatch arm (the `ui/*` protocol is
//!   host↔iframe over postMessage); only the typed `_meta.ui`
//!   projection lives here.
//! - [`events`](crate::v_2026_07_28::extensions::events)
//!   (`io.modelcontextprotocol/events`, experimental) — event types,
//!   webhook subscriptions, poll, and their error codes.
//!
//! Future extensions register their wire types (and, if they add
//! MCP-wire methods, their dispatch arms) here as their own modules.

pub mod apps;
pub mod events;
pub mod tasks;
