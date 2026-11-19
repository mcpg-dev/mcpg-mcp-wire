//! Completion wire types for MCP revision `2026-07-28`.
//!
//! `completion/complete` is the autocomplete surface clients call
//! when a user is editing a prompt argument or resource-template
//! variable. The types live in [`crate::shared::completion`] and are
//! re-exported here beside the method-name constant.

/// JSON-RPC method name.
pub const METHOD_COMPLETION_COMPLETE: &str = "completion/complete";

pub use crate::shared::completion::{
    CompletionArgument, CompletionCompleteParams, CompletionContext, CompletionReference,
    CompletionResult, CompletionValues,
};
