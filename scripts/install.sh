#!/usr/bin/env bash
# Build hkweather from this checkout and install it into ~/.local/bin.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${HKWEATHER_BIN_DIR:-$HOME/.local/bin}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required: https://rustup.rs" >&2
  exit 1
fi

cargo build --release --locked --manifest-path "$ROOT/Cargo.toml"
mkdir -p "$DEST"
install -m 0755 "$ROOT/target/release/hkweather" "$DEST/hkweather"
printf 'installed %s\n' "$DEST/hkweather"
case ":$PATH:" in
  *":$DEST:"*) ;;
  *) echo "note: add $DEST to PATH" >&2 ;;
esac
