//! Modern completion wire types for MCP revision `2026-07-28`.
//!
//! `completion/complete` is the autocomplete surface clients call
//! when a user is editing a prompt argument or resource-template
//! variable. The wire shape is **identical** to the legacy
//! 2025-11-25 surface — completion is one of the few methods the
//! modern revision left unchanged. The types are shared with the
//! legacy revision and re-exported here from
//! [`crate::shared::completion`]; only the method-name
//! constant stays version-side so a future revision can diverge
//! without disturbing legacy.

/// JSON-RPC method name.
pub const METHOD_COMPLETION_COMPLETE: &str = "completion/complete";

pub use crate::shared::completion::{
    CompletionArgument, CompletionCompleteParams, CompletionContext, CompletionReference,
    CompletionResult, CompletionValues,
};
