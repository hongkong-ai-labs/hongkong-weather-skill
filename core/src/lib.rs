//! Shared runtime for the Hong Kong AI Labs CLI/MCP skills.
//!
//! Two pieces are generic enough to be identical in every skill:
//!
//! - [`mcp`] is a small JSON-RPC 2.0 server over stdio. It owns protocol
//!   framing, version negotiation and the rule that a failing tool returns an
//!   `isError: true` result instead of a protocol error.
//! - [`update`] implements `update --check` / `update` using the method the
//!   binary was installed with (cargo, release installer, source build,
//!   Homebrew or container).
//!
//! The crate deliberately has no HTTP client, no serialization framework and
//! no logging dependency, so a skill can adopt it without changing its data
//! layer.

pub mod mcp;
pub mod update;
