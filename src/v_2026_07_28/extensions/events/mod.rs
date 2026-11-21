//! MCP Events (`io.modelcontextprotocol/events`) — wire types.
//!
//! The extension is at the Beta stage of its specification
//! (`modelcontextprotocol/ext-events`, `specification/beta/events`), in
//! its initial revision. Every wire shape the extension uses lives in this
//! module so a change touches one place.
//!
//! A server declares event types with `events/list`. A client subscribes
//! with `(name, arguments)` and receives occurrences
//! (`{eventId, name, timestamp, data, cursor}`) through one of three
//! delivery modes:
//!
//! - **webhook** — `events/subscribe` registers an `https` callback and a
//!   client-supplied `whsec_` secret; the server POSTs each occurrence,
//!   signed per Standard Webhooks, and the client refreshes the
//!   subscription before `refreshBefore`. `events/unsubscribe` is eager
//!   cleanup.
//! - **poll** — `events/poll`, one request per subscription.
//! - **push** — `events/stream`, a long-lived request whose heartbeat
//!   interval the server grants on `notifications/events/active`.
//!
//! The server advertises the extension as the
//! `capabilities.extensions["io.modelcontextprotocol/events"]` entry of
//! `server/discover`. Over Streamable HTTP, `events/poll`,
//! `events/stream`, `events/subscribe` and `events/unsubscribe` carry
//! `Mcp-Name` (the event name) and an `Mcp-Param-{Name}` header for each
//! argument the type's `inputSchema` annotates with `x-mcp-header`.
//!
//! Parsing reads the specification's field names and codes only, and is
//! lenient within them:
//!
//! - unknown fields are ignored everywhere;
//! - an absent `cursor` reads as `null`, and absent or `null`
//!   `arguments` read as `{}`;
//! - `ttlMs` is tri-state — absent (server default), `null` (no expiry
//!   requested) or a number;
//! - an unknown delivery mode or error category parses as `Unknown`
//!   rather than failing the whole message.
//!
//! `refreshBefore` and the response `cursor` are always serialized, `null`
//! included.
//!
//! - [`wire`] — method names, capability, descriptors, params and
//!   results, occurrences, control envelopes, the `whsec_` secret type.
//! - [`errors`] — the error codes and their typed `data`.
//! - [`schema`] — whether a change to a type's schemas keeps its
//!   subscribers whole.

pub mod errors;
pub mod schema;
pub mod wire;
