#!/usr/bin/env bash
# Fail when the vendored core crate differs from the canonical checkout.
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(dirname "$script_dir")"
source_dir="${HK_SKILL_CORE_DIR:-$repo_root/../hongkong-skill-core}"
target_dir="$repo_root/core"

if [[ ! -f "$source_dir/Cargo.toml" ]]; then
  printf 'check-core: canonical crate not found at %s (skipped)\n' "$source_dir" >&2
  exit 0
fi

for file in Cargo.toml README.md LICENSE; do
  diff -u "$source_dir/$file" "$target_dir/$file"
done
diff -ru "$source_dir/src" "$target_dir/src"

printf 'core copy is in sync with %s\n' "$source_dir"
