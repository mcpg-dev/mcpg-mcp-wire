# mcpg-mcp-wire

MCP wire types for every protocol revision mcpg speaks: the
`2025-11-25` sessionful wire and the `2026-07-28` stateless wire
(including the tasks/apps extension shapes), plus the version-agnostic
primitives both share — the JSON-RPC envelope, content blocks, error
codes, `_meta` rules, the SEP-2243 header codec and the
`ProtocolVersion` negotiation enum. Frames only: hand-written serde
structs with no transport, no dispatch and no runtime attached. The
mcpg gateway serves with these types and the mcpg inspector dials
with them, so the two can never disagree about a byte on the wire.

This repository is read-only: development happens upstream, and each release
is published here as a tagged snapshot. Issues are welcome. Consume the crate
by git reference:

```toml
[dependencies]
mcpg-mcp-wire = { git = "https://github.com/mcpg-dev/mcpg-mcp-wire", tag = "<release-tag>" }
```

## Building and testing

```sh
cargo build
cargo test
```
