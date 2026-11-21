#!/bin/sh
# Install the hkweather binary from a GitHub release, or build it with cargo.
set -eu

REPO="${HKWEATHER_REPO:-hongkong-ai-labs/hongkong-weather-skill}"
VERSION="${HKWEATHER_VERSION:-}"
BIN_DIR="${HKWEATHER_BIN_DIR:-$HOME/.local/bin}"
FORCE="${HKWEATHER_FORCE_INSTALL:-0}"
BINARY=hkweather
WORK_DIR="${TMPDIR:-/tmp}/hkweather-install.$$"
SCRIPT_DIR=""
TARGET=""
TAG=""
SOURCE_DIR=""

log() { printf 'install: %s\n' "$*" >&2; }
die() { log "error: $*"; exit 1; }

cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT INT TERM

# When the script is a real file (not piped through sh), remember scripts/.
if [ -n "${0:-}" ] && [ -f "${0}" ]; then
  SCRIPT_DIR=$(cd "$(dirname "${0}")" && pwd)
fi

download() {
  # download URL OUTPUT_FILE
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$2" "$1"
  else
    log "need curl or wget to download $1"
    return 1
  fi
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    log "need shasum or sha256sum to verify the download"
    return 1
  fi
}

detect_target() {
  OS=$(uname -s)
  ARCH=$(uname -m)
  case "$OS:$ARCH" in
    Darwin:x86_64) printf '%s\n' x86_64-apple-darwin ;;
    Darwin:arm64) printf '%s\n' aarch64-apple-darwin ;;
    Linux:x86_64 | Linux:amd64) printf '%s\n' x86_64-unknown-linux-musl ;;
    Linux:aarch64 | Linux:arm64) printf '%s\n' aarch64-unknown-linux-musl ;;
    *)
      log "unsupported platform: $OS $ARCH (expected macOS x86_64/arm64 or Linux x86_64/aarch64)"
      return 1
      ;;
  esac
}

latest_tag() {
  JSON="$WORK_DIR/latest.json"
  download "https://api.github.com/repos/$REPO/releases/latest" "$JSON" || return 1
  FOUND=$(sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$JSON" | head -n 1)
  [ -n "$FOUND" ] || return 1
  printf '%s\n' "$FOUND"
}

install_binary() {
  # install_binary SOURCE_FILE; prints the installed absolute path.
  chmod +x "$1" || return 1
  mkdir -p "$BIN_DIR" || return 1
  TMP_BIN="$BIN_DIR/.$BINARY.tmp.$$"
  cp "$1" "$TMP_BIN" || return 1
  chmod +x "$TMP_BIN" || return 1
  mv -f "$TMP_BIN" "$BIN_DIR/$BINARY" || return 1
  printf '%s\n' "$BIN_DIR/$BINARY"
}

install_from_release() {
  ASSET="$BINARY-$TAG-$TARGET.tar.gz"
  BASE="https://github.com/$REPO/releases/download/$TAG"
  log "downloading $BASE/$ASSET"
  download "$BASE/$ASSET" "$WORK_DIR/$ASSET" || return 1
  download "$BASE/SHA256SUMS" "$WORK_DIR/SHA256SUMS" || return 1
  EXPECTED=$(awk -v name="$ASSET" '{ file = $2; sub(/^\*/, "", file); if (file == name) { print $1; exit } }' "$WORK_DIR/SHA256SUMS")
  [ -n "$EXPECTED" ] || {
    log "SHA256SUMS has no entry for $ASSET"
    return 1
  }
  ACTUAL=$(sha256 "$WORK_DIR/$ASSET") || return 1
  [ "$ACTUAL" = "$EXPECTED" ] || {
    log "checksum mismatch for $ASSET"
    return 1
  }
  mkdir -p "$WORK_DIR/extract" || return 1
  tar -xzf "$WORK_DIR/$ASSET" -C "$WORK_DIR/extract" || return 1
  [ -f "$WORK_DIR/extract/$BINARY" ] || {
    log "archive did not contain $BINARY"
    return 1
  }
  install_binary "$WORK_DIR/extract/$BINARY"
}

install_from_source() {
  if [ -n "$SCRIPT_DIR" ] && [ -f "$SCRIPT_DIR/../Cargo.toml" ]; then
    SOURCE_DIR=$(cd "$SCRIPT_DIR/.." && pwd)
  elif [ -f "./Cargo.toml" ]; then
    SOURCE_DIR=$(pwd)
  fi
  if [ -z "$SOURCE_DIR" ] || ! command -v cargo >/dev/null 2>&1; then
    die "no release at https://github.com/$REPO/releases/download/$TAG/ and no cargo build possible (looked for Cargo.toml in ${SCRIPT_DIR:-the script directory} and the current directory)"
  fi
  log "building from source in $SOURCE_DIR"
  (cd "$SOURCE_DIR" && cargo build --release --locked >&2) ||
    die "cargo build --release --locked failed in $SOURCE_DIR"
  [ -x "$SOURCE_DIR/target/release/$BINARY" ] ||
    die "cargo build produced no $SOURCE_DIR/target/release/$BINARY"
  install_binary "$SOURCE_DIR/target/release/$BINARY"
}

command -v tar >/dev/null 2>&1 || die "tar is required"
mkdir -p "$BIN_DIR"
BIN_DIR=$(cd "$BIN_DIR" && pwd)
mkdir -p "$WORK_DIR"

if [ -n "$VERSION" ]; then
  case "$VERSION" in
    v*) TAG="$VERSION" ;;
    *) TAG="v$VERSION" ;;
  esac
else
  TAG=$(latest_tag) || die "could not resolve the latest release tag for $REPO; set HKWEATHER_VERSION"
fi

# Keep an installed binary only when it is already the requested version, so a
# re-run of this script upgrades instead of silently keeping a stale build.
if [ -x "$BIN_DIR/$BINARY" ] && [ "$FORCE" != "1" ]; then
  INSTALLED=$("$BIN_DIR/$BINARY" --version 2>/dev/null | awk '{print $NF}') || INSTALLED=""
  if [ "v$INSTALLED" = "$TAG" ]; then
    printf '%s\n' "$BIN_DIR/$BINARY"
    exit 0
  fi
  log "replacing $BIN_DIR/$BINARY (${INSTALLED:-unknown version}) with $TAG"
fi

TARGET=$(detect_target) || exit 1

install_from_release || install_from_source
