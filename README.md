# hkweather

Hong Kong Observatory weather data as a Rust CLI and an MCP server.

`hkweather` reads HKO's public, keyless open data and exposes current
readings, district rainfall, UV, active warnings, the local forecast and the
nine-day forecast as tables, JSON, or MCP tools.

It is an independent, unofficial project, not affiliated with the Hong Kong
Observatory.

## Install

**Agent skill** — installs `SKILL.md` and the binary bootstrap script:

```bash
npx skills add hongkong-ai-labs/hongkong-weather-skill
npx skills update        # refresh the installed skill later
```

**Prebuilt binary** — macOS (Intel or Apple silicon) and Linux (x86_64 or
aarch64, static musl), checksum-verified, into `~/.local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/hongkong-ai-labs/hongkong-weather-skill/main/scripts/install.sh | sh
```

**From source** — Rust 1.89 or newer:

```bash
git clone https://github.com/hongkong-ai-labs/hongkong-weather-skill
cd hongkong-weather-skill
cargo install --path . --locked
# or directly from GitHub:
cargo install --git https://github.com/hongkong-ai-labs/hongkong-weather-skill --locked
# or: cargo build --release --locked   # target/release/hkweather
```

Verify:

```console
$ hkweather --version
```

`hkweather update` detects whether the binary came from cargo, the release
installer, a source checkout or Homebrew, and updates with that same
method. `hkweather update --check` only reports.

## CLI

```bash
hkweather current                       # all current readings
hkweather current --place "Sha Tin"     # one place
hkweather forecast --days 3             # first three forecast days
hkweather warnings                      # active warnings
hkweather warnings --all                # include cancelled
hkweather local                         # local forecast + outlook
hkweather rain                          # district rainfall
hkweather uv                            # UV index
hkweather all                           # everything
hkweather --json current                # machine-readable
hkweather --lang tc forecast            # Traditional Chinese HKO text
hkweather mcp                           # MCP over stdio
hkweather update --check
```

## MCP

```json
{
  "mcpServers": {
    "hongkong-weather": {
      "command": "hkweather",
      "args": ["mcp"]
    }
  }
}
```

Register it with any MCP client:

```bash
npx add-mcp "$(command -v hkweather) mcp" --name hongkong-weather -a claude-code -a codex -a cursor -y
claude mcp add hongkong-weather -- hkweather mcp
codex mcp add hongkong-weather -- hkweather mcp
```

`add-mcp` detects the client configuration files in the current project; add
`-g` for a global install. `$(command -v hkweather)` keeps the entry correct
when the binary is on `PATH`; substitute an absolute path if your client does
not inherit the shell `PATH`.


Tools: `hko_current_weather`, `hko_forecast`, `hko_warnings`,
`hko_local_forecast`, `hko_rainfall`.

## Data

| Dataset | HKO `dataType` |
| --- | --- |
| Current readings | `rhrread` |
| Nine-day forecast | `fnd` |
| Local forecast | `flw` |
| Warnings | `warninginfo` |

Source: `https://data.weather.gov.hk/weatherAPI/opendata/weather.php`.
No API key. Responses are cached for 60 seconds (`--cache-ttl 0` disables).

## Limits

HKO data is published for general information. Warnings and forecasts can
change; always check HKO directly for safety-critical decisions. The API does
not provide a per-minute nowcast, lightning location, or official district
rainfall forecast.

## Run modes and deployment

| Mode | Start with | Use for |
| --- | --- | --- |
| CLI | `hkweather <command>` | Interactive use and shell scripts |
| JSON | `<command>` with `--json` | `jq`, automation, other programs |
| MCP stdio | `hkweather mcp` | Desktop and CLI MCP clients |
| Container | `docker run --rm -i hkweather mcp` | No local Rust toolchain |

### MCP and authentication

This server is **stdio-only**: the client launches the process and speaks MCP
over stdin/stdout. There is no listening port, so there is no network
authentication layer and no API-key setting. Optional API keys in this project
family apply only to servers that expose an HTTP transport. Keep the process
local and let the MCP client manage it.

Typical client entry:

```json
{
  "mcpServers": {
    "hongkong-weather-skill": {
      "command": "hkweather",
      "args": ["mcp"]
    }
  }
}
```

If the client does not inherit `PATH`, use the absolute installed path.

### Docker and Apple container

```bash
docker build -t hkweather .
docker run --rm hkweather --help
docker run --rm -i hkweather mcp          # stdio MCP
docker compose run --rm stdio             # same, through Compose
```

Apple's `container` runtime uses the same OCI image:

```bash
container build -t hkweather .
container run --rm hkweather --help
container run --rm -i hkweather mcp
```

The runtime image contains the release binary and CA certificates, runs as a
non-root user, and defaults to `--help` so a bare `docker run` is harmless.

### Cloudflare and WASM

There is no Worker or WASM deployment. The binary talks directly to upstream
HTTP APIs over TLS and uses a Tokio runtime; packaging it as a stdio server is
the supported integration. A Worker would need a separate HTTP/JavaScript
adapter and is out of scope for this repo.

### Releases

Pushing a `v*` tag runs the release workflow, which checks the tag against
`Cargo.toml` and `CHANGELOG.md`, builds four targets, smoke-tests native
binaries and publishes:

- `hkweather-<tag>-<target>.tar.gz` for macOS and static Linux
- `SHA256SUMS` covering every tarball

`scripts/install.sh` downloads the latest release, verifies its checksum and
falls back to `cargo build --release --locked` when no release is available.
`just tag X.Y.Z` creates the annotated tag locally; `just release X.Y.Z` tags
and pushes to trigger the workflow.

## Development

```bash
just                 # fmt check, clippy with -D warnings, tests
just skill-check     # validate SKILL.md and shell syntax
just container       # build the image and print CLI help
cargo test --all --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --all -- --check
```

`rust-toolchain.toml` pins the stable toolchain with `rustfmt` and
`clippy`, so `rustup` sets contributors up automatically. `just doc`
builds the API documentation. See `CONTRIBUTING.md` for the full check
list and the signed-commit requirement.


CI runs the same checks. Dependabot proposes weekly Cargo and GitHub Actions
updates, and the audit workflow checks the lockfile against the RustSec
advisory database.

## Licence

MIT. See [LICENSE](LICENSE). HKO data remains subject to the Observatory's
terms of use.
