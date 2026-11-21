#!/usr/bin/env bash
# Refresh the vendored hongkong-skill-core copy from the canonical checkout.
#
# The crate is vendored so this repository stays installable with
# `cargo install --path .` and `cargo install --git` from a standalone clone.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(dirname "$script_dir")"
source_dir="${HK_SKILL_CORE_DIR:-$repo_root/../hongkong-skill-core}"
target_dir="$repo_root/core"

if [[ ! -f "$source_dir/Cargo.toml" ]]; then
  printf 'sync-core: canonical crate not found at %s\n' "$source_dir" >&2
  printf 'set HK_SKILL_CORE_DIR to the hongkong-skill-core checkout\n' >&2
  exit 1
fi

rm -rf "$target_dir"
mkdir -p "$target_dir"
cp "$source_dir/Cargo.toml" "$source_dir/README.md" "$source_dir/LICENSE" "$target_dir/"
cp -R "$source_dir/src" "$target_dir/src"

printf 'synced %s from %s\n' "$target_dir" "$source_dir"
