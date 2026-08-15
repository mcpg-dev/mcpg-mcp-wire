//! MCP wire types for every protocol revision mcpg speaks.
//!
//! Extracted from the gateway's `protocol` module so the gateway and
//! the inspector share one set of frame definitions:
//!
//! - [`version`] — `ProtocolVersion` enum + parsing.
//! - [`shared`] — version-agnostic primitives (JSON-RPC envelope,
//!   content blocks, error codes, `_meta` rules, apps / caching /
//!   deprecation surfaces).
//! - [`descriptors`] — the tool / prompt / resource descriptor shapes
//!   the sessionful wire serializes directly.
//! - `v_<date>` — per-revision wire modules ([`v_2025_11_25`],
//!   [`v_2026_07_28`], including the modern extensions' wire shapes).
//!
//! Server-side dispatch (the `ProtocolHandler` trait, the registry and
//! the per-revision handlers) stays in the gateway: this crate is
//! frames only. The flat re-exports below preserve the historical
//! `protocol::{Type, ...}` surface that gateway code imports through
//! its own re-export shim.

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
pub mod v_2025_11_25;
pub mod v_2026_07_28;
pub mod version;

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
pub use v_2025_11_25::wire::common::{
    CONTENT_TOO_LARGE_CODE, CancelledNotificationParams, EmptyResult, GUARDRAIL_DENIED_CODE,
    GUARDRAIL_SERVICE_ERROR_CODE, ListChangedNotification, ListParams, ProgressNotification,
    ProgressParams, ServerJsonRpcRequest,
};
pub use v_2025_11_25::wire::completion::{
    CompletionArgument, CompletionCompleteParams, CompletionContext, CompletionReference,
    CompletionResult, CompletionValues,
};
pub use v_2025_11_25::wire::elicitation::{
    ELICITATION_NOT_SUPPORTED_CODE, ElicitationAction, ElicitationCompleteNotification,
    ElicitationCompleteParams, ElicitationCreateParams, URL_ELICITATION_REQUIRED_CODE,
};
pub use v_2025_11_25::wire::lifecycle::{
    CapabilityFlag, ClientCapabilities, ClientElicitationCapability, ClientRootsCapability,
    ClientSamplingCapability, ClientTaskElicitationCapability, ClientTaskRequestsCapability,
    ClientTaskRootsCapability, ClientTaskSamplingCapability, ClientTasksCapability,
    ImplementationInfo, InitializeParams, InitializeResult, ListCapability, ResourceCapability,
    ServerCapabilities, ServerTaskRequestsCapability, ServerTaskToolsCapability, TasksCapability,
};
pub use v_2025_11_25::wire::logging::{
    LoggingLevel, LoggingMessageNotification, LoggingMessageParams, LoggingSetLevelParams,
};
pub use v_2025_11_25::wire::operations::{
    CapabilityOperation, LifecycleOperation, LoggingOperation, ProtocolOperation, TaskOperation,
};
pub use v_2025_11_25::wire::prompts::{
    PromptGetParams, PromptGetResult, PromptMessage, PromptMessageContent, PromptsListResult,
};
pub use v_2025_11_25::wire::resources::{
    BlobResourceContents, ResourceContents, ResourceReadParams, ResourceReadResult,
    ResourceSubscribeParams, ResourceTemplate, ResourceTemplatesListResult, ResourceTextContents,
    ResourceUpdatedNotification, ResourceUpdatedParams, ResourcesListResult,
};
pub use v_2025_11_25::wire::routing::map_client_message_to_operation;
pub use v_2025_11_25::wire::sampling::{
    SamplingCreateMessageParams, SamplingIncludeContext, SamplingMessage, SamplingMessageContent,
};
pub use v_2025_11_25::wire::tasks::{
    CreateTaskResult, MODEL_IMMEDIATE_RESPONSE_META_KEY, RELATED_TASK_META_KEY, Task,
    TaskAugmentParams, TaskCancelParams, TaskGetParams, TaskResultParams, TaskStatus,
    TaskStatusNotification, TaskStatusNotificationParams, TasksListParams, TasksListResult,
    related_task_meta,
};
pub use v_2025_11_25::wire::tools::{ToolCallParams, ToolCallResult, ToolsListResult};
pub use v_2025_11_25::wire::{
    DEFAULT_SAMPLING_MAX_TOKENS, LEGACY_DEFAULT_PROTOCOL_VERSION, LEGACY_PROTOCOL_VERSIONS,
    PROTOCOL_VERSION_HEADER, SESSION_ID_HEADER, SUPPORTED_PROTOCOL_VERSION,
};
