//! SEP-2663 `io.modelcontextprotocol/tasks` extension (final
//! 2026-07-28 shape) — long-running async work.
//!
//! **Materialization is server-directed.** There is no client
//! `createTask`. A task is materialized during `tools/call`: a
//! long-running tool elects to go async and the server returns a
//! `CreateTaskResult` (`resultType: "task"`) in lieu of the standard
//! result. The client opts in once, per request, by declaring the
//! `io.modelcontextprotocol/tasks` extension in
//! `_meta.io.modelcontextprotocol/clientCapabilities.extensions`. A
//! server MUST NOT return a task to a client that did not declare it.
//!
//! **Methods (bare, under the extension capability):**
//!
//! - `tasks/get` — poll a task's state; terminal states carry
//!   `result` (`completed`) or `error` (`failed`), and an
//!   `input_required` task carries `inputRequests`.
//! - `tasks/update` — the **client→server** channel that answers a
//!   task's outstanding `inputRequests` with `inputResponses`. The
//!   server acks empty. A task awaiting input is a suspended MRTR
//!   pipeline, so this routes through the MRTR resume codec rather
//!   than duplicating the resume machinery.
//! - `tasks/cancel` — cooperative cancellation; empty ack.
//!
//! **Shape:**
//!
//! - There is no `tasks/result` (poll `tasks/get`; the terminal
//!   `result` rides inline) and no `tasks/list` (clients track their
//!   own ids).
//! - The status notification is the bare `notifications/tasks`,
//!   carried over `subscriptions/listen`.
//! - Durations are `ttlMs` and `pollIntervalMs`. Result envelopes are
//!   flat (`Result & Task`), not `{ task: {…} }`.
//!
//! This module owns the wire types; the dispatch arms attach to the
//! gateway's protocol handler.

pub mod wire;
