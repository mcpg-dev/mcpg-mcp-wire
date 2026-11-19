//! MCP wire types for protocol revision `2026-07-28`.
//!
//! The gateway serves with these types and the inspector dials with
//! them, so the two share one set of frame definitions:
//!
//! - [`shared`] — protocol primitives (JSON-RPC envelope, content
//!   blocks, error codes, `_meta` rules, apps / caching / deprecation
//!   surfaces, and the request / result shapes the gateway runtime
//!   builds).
//! - [`descriptors`] — the tool / prompt / resource descriptor shapes
//!   of a backend catalog.
//! - [`v_2026_07_28`] — the revision's wire module, including the
//!   extensions' wire shapes. Its `SUPPORTED_PROTOCOL_VERSION` is the
//!   one version this crate speaks.
//!
//! Server-side dispatch stays in the gateway: this crate is frames
//! only. The flat re-exports below give the commonly used types a
//! short `mcpg_mcp_wire::Type` path.

// Mirrors `[lints.clippy]` in Cargo.toml. Cargo reads the manifest table and
// other build systems do not, and a command-line `-D warnings` outranks a
// build file's flags but not a source attribute — so the crate states its own
// lint decisions here, where every toolchain honours them. The decisions are
// the gateway's: wire unions are legitimately large enums and `ProtocolError`
// is a legitimately large `Err` payload.
#![allow(clippy::large_enum_variant)]
#![allow(clippy::result_large_err)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]

pub mod descriptors;
pub mod shared;
pub mod v_2026_07_28;

pub use shared::PROTOCOL_VERSION_HEADER;
pub use shared::client_capabilities::{
    CapabilityFlag, ClientCapabilities, ClientElicitationCapability, ClientRootsCapability,
    ClientSamplingCapability,
};
pub use shared::completion::{
    CompletionArgument, CompletionCompleteParams, CompletionContext, CompletionReference,
    CompletionResult, CompletionValues,
};
pub use shared::content::{ContentAnnotations, EmbeddedResource, Icon, ToolContent};
pub use shared::error::{
    HEADER_MISMATCH_CODE, INTERNAL_ERROR_CODE, INVALID_PARAMS_CODE, INVALID_REQUEST_CODE,
    METHOD_NOT_FOUND_CODE, MISSING_REQUIRED_CLIENT_CAPABILITY_CODE, PARSE_ERROR_CODE,
    PAYMENT_REQUIRED_CODE, PAYMENT_VERIFICATION_FAILED_CODE, ProtocolError,
    UNSUPPORTED_PROTOCOL_VERSION_CODE,
};
pub use shared::jsonrpc::{
    ClientMessage, JSONRPC_VERSION, JsonRpcError, JsonRpcErrorBody, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, JsonRpcSuccess, ProtocolHttpResponse, ProtocolResponse,
    parse_client_message,
};
pub use shared::notifications::{
    CancelledNotificationParams, ListChangedNotification, ProgressNotification, ProgressParams,
    ResourceUpdatedNotification, ResourceUpdatedParams,
};
pub use shared::prompts::{PromptGetParams, PromptGetResult, PromptMessage, PromptMessageContent};
pub use shared::resources::{
    BlobResourceContents, ResourceContents, ResourceReadParams, ResourceReadResult,
    ResourceTemplate, ResourceTextContents,
};
pub use shared::server_requests::{
    DEFAULT_SAMPLING_MAX_TOKENS, ElicitationCreateParams, SamplingCreateMessageParams,
    SamplingIncludeContext, SamplingMessage, SamplingMessageContent, ServerJsonRpcRequest,
};
pub use shared::tasks::{Task, TaskStatus};
pub use shared::tools::{ToolCallParams, ToolCallResult};
pub use v_2026_07_28::wire::SUPPORTED_PROTOCOL_VERSION;
