//! SEP-1865 **MCP Apps** extension — modern-wire typed surface.
//!
//! MCP Apps (`io.modelcontextprotocol/ui`) lets a server attach an
//! interactive HTML UI to a tool. Unlike the [`tasks`] extension, Apps
//! adds **no new MCP-wire methods**: the `ui/*` protocol runs
//! host↔iframe over `postMessage` and never reaches MCPG, and the
//! action proxy (iframe → host → `tools/call`) re-enters through the
//! ordinary `tools/call` arm. So there is no dispatch arm here — only
//! wire types.
//!
//! The version-agnostic machinery — constants, the `ui://`
//! resourceUri rewrite used by federation, and the CSP/permission
//! policy engine — lives in [`crate::shared::apps`] and is
//! re-exported here for convenience.
//!
//! [`tasks`]: crate::v_2026_07_28::extensions::tasks

pub use crate::shared::apps::{
    AppsPolicy, EXTENSION_ID, PolicyReport, UI_MIME_TYPE, UI_URI_SCHEME,
};
