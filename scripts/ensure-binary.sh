#!/usr/bin/env bash
# Print the absolute path of a working hkweather binary.
# Order: $HKWEATHER_BIN, hkweather on PATH, cargo build --release in this checkout.
set -euo pipefail

if [ -n "${HKWEATHER_BIN:-}" ] && [ -x "$HKWEATHER_BIN" ]; then
  printf '%s\n' "$HKWEATHER_BIN"
  exit 0
fi

if command -v hkweather >/dev/null 2>&1; then
  command -v hkweather
  exit 0
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -f "$ROOT/Cargo.toml" ] && command -v cargo >/dev/null 2>&1; then
  (
    cd "$ROOT"
    cargo build --release --locked >/dev/null
  )
  BIN="$ROOT/target/release/hkweather"
  if [ -x "$BIN" ]; then
    printf '%s\n' "$BIN"
    exit 0
  fi
fi

echo "hkweather: no binary found and no Cargo build available" >&2
echo "Set HKWEATHER_BIN, put hkweather on PATH, or build with: cargo build --release --locked" >&2
exit 1
