# hongkong-skill-core

Shared runtime for the Hong Kong AI Labs CLI/MCP skills.

Two pieces are identical in every skill:

| Module | What it does |
| --- | --- |
| `mcp` | JSON-RPC 2.0 over stdio: `initialize` with version negotiation, `ping`, `tools/list`, `tools/call`. Failing tools return an `isError: true` result; unknown tools return `-32602`; parse errors return `-32700`; notifications get no reply. |
| `update` | `update --check` / `update` using the installation method the running binary was found in: cargo, release installer, source checkout, Homebrew or container. |

It has no HTTP client, no logging framework and no serialization framework, so
adopting it does not change a skill's data layer.

## Usage

```rust
use hongkong_skill_core::mcp::{self, Tool};
use serde_json::{Value, json};

let tools = vec![Tool::new(
    "hko_warnings",
    "Active Hong Kong weather warnings.",
    json!({ "type": "object", "properties": {} }),
)];

mcp::serve_stdio(
    "hongkong-weather-skill",
    env!("CARGO_PKG_VERSION"),
    tools,
    |name, args| async move {
        match name.as_str() {
            "hko_warnings" => Ok(json!({ "warnings": [] })),
            other => Err(format!("unknown tool: {other}")),
        }
    },
)
.await?;
```

The caller must be async and must await this function. Do **not** build a Tokio
runtime inside `serve_stdio`; that is what made two skills panic with
`Cannot start a runtime from within a runtime`.

## Adoption model

Each skill stays a standalone repository and must remain installable with
`cargo install --path .` and `cargo install --git`. A path dependency on a
sibling checkout would break both, so a consuming repository keeps a vendored
copy of this crate at `core/` and depends on it with:

```toml
[dependencies]
hongkong-skill-core = { path = "core" }

[workspace]
members = ["core"]
```

`hongkong-weather-skill` is the reference port. Its `scripts/sync-core.sh`
refreshes `core/` from this directory, and `scripts/check-core.sh` verifies the
copy is identical. The long-term plan is to publish this crate and replace the
vendored copy with a versioned dependency.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```
