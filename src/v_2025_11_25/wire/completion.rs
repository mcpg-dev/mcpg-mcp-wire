//! Completion wire types for MCP revision `2025-11-25`.
//!
//! `completion/complete` is the autocomplete surface clients call
//! when a user is editing a prompt argument or resource-template
//! variable. Per spec the method is singular (`completion/complete`)
//! while the server capability is plural (`completions`).
//!
//! All operation routing for `completion/complete` lives in
//! `routing.rs`. The wire types are shared with the `2026-07-28`
//! revision and re-exported here from
//! [`crate::shared::completion`].

pub use crate::shared::completion::{
    CompletionArgument, CompletionCompleteParams, CompletionContext, CompletionReference,
    CompletionResult, CompletionValues,
};
