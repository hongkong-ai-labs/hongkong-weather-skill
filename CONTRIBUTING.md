# Contributing

## Requirements

- Rust 1.89 or newer (the MSRV is declared as `rust-version` in
  `Cargo.toml`). `rustup` reads `rust-toolchain.toml` and installs the stable
  toolchain with `rustfmt` and `clippy`.
- `just` is optional but runs the same commands as CI. Install it with
  `cargo install just` or use the raw `cargo` commands below.
- Git commits must be signed; the repository enforces this in CI.

## Setup

```bash
git clone https://github.com/hongkong-ai-labs/hongkong-weather-skill
cd hongkong-weather-skill
cargo build --locked
```

## Checks

Run everything CI runs:

```bash
just check        # fmt, clippy and tests
just doc          # cargo doc --no-deps
```

Or run the individual steps:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all --locked
cargo doc --no-deps --locked
```

Build and smoke-test the binary:

```bash
cargo run -- --help
cargo run -- --version
```

## Pull requests

- Keep changes focused and add tests for new parsing or data paths.
- Update `README.md` and `SKILL.md` when commands, tools or flags change.
- Sign your commits (`git commit -S`) and keep the working tree free of build
  artefacts.

## Releases

Version bumps are tagged with `just tag <version>`; pushing the tag runs the
release workflow, which builds the binaries for macOS and Linux.
