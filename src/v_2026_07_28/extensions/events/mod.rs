//! MCP Events (`io.modelcontextprotocol/events`) — wire types.
//!
//! **Experimental.** The upstream definition is a working-group design
//! sketch (`modelcontextprotocol/experimental-ext-triggers-events`,
//! proposed as SEP-3415), not a ratified specification. Field names have
//! moved between drafts, the capability location and the error-code
//! numbers are still open upstream, and any of it may change. Every wire
//! shape the extension uses lives in this module so a rename touches one
//! place.
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
//! - **push** — `events/stream`, a long-lived request.
//!
//! The server advertises the extension in two places at once, because
//! the drafts disagree: a top-level `capabilities.events` object and an
//! `extensions["io.modelcontextprotocol/events"]` entry.
//!
//! Parsing is lenient so a client written against a neighbouring draft
//! still interoperates:
//!
//! - unknown fields are ignored everywhere;
//! - an absent `cursor` reads as `null`, and absent or `null`
//!   `arguments` read as `{}`;
//! - `ttlMs` is tri-state — absent (server default), `null` (no expiry
//!   requested) or a number;
//! - the older spellings `maxAge` (seconds) and `nextPollSeconds` are
//!   accepted and converted to milliseconds;
//! - an unknown delivery mode or error category parses as `Unknown`
//!   rather than failing the whole message.
//!
//! Emitted shapes follow the latest draft. `refreshBefore` and the
//! response `cursor` are always serialized, `null` included.
//!
//! - [`wire`] — method names, capability, descriptors, params and
//!   results, occurrences, control envelopes, the `whsec_` secret type.
//! - [`errors`] — the error codes in both numberings and their typed
//!   `data`.
//! - [`schema`] — whether a change to a type's schemas keeps its
//!   subscribers whole.

pub mod errors;
pub mod schema;
pub mod wire;
