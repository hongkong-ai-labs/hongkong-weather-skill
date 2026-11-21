set shell := ["bash", "-uc"]
set dotenv-load := false

# Run the full check suite.
default: check

# Format check, clippy and tests.
check: fmt-check clippy test

# Format the source.
fmt:
    cargo fmt --all

# Check formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Lint every target, warnings are errors.
clippy:
    cargo clippy --all-targets --locked -- -D warnings

# Run the test suite.
test:
    cargo test --all --locked

# Audit dependencies for known vulnerabilities.
audit:
    @if command -v cargo-audit >/dev/null 2>&1; then cargo audit; else echo "cargo-audit is not installed; skipping (cargo install cargo-audit)"; fi

# Build the API documentation.
doc:
    cargo doc --no-deps --locked

# Build the debug binary.
build:
    cargo build --locked

# Build the optimised release binary.
build-release:
    cargo build --release --locked

# Run the CLI, passing arguments through.
run *ARGS:
    cargo run -- {{ ARGS }}

# Install the binary into the cargo bin directory.
install:
    cargo install --path . --locked --force

# Build the container and print CLI help.
container:
    docker build -t hkweather:local .
    docker run --rm hkweather:local --help

# Validate SKILL.md and the shell scripts the way CI does.
skill-check:
    #!/usr/bin/env bash
    python3 - <<'PY'
    import pathlib
    import sys
    path = pathlib.Path("SKILL.md")
    text = path.read_text(encoding="utf-8") if path.is_file() else sys.exit("SKILL.md is missing at the repo root")
    sys.exit("SKILL.md does not start with YAML frontmatter") if not text.startswith("---\n") else None
    parts = text.split("---\n", 2)
    sys.exit("SKILL.md frontmatter is not closed") if len(parts) != 3 else None
    rows = [line.split(":", 1) for line in parts[1].splitlines() if line.strip() and not line.lstrip().startswith("#") and ":" in line]
    fields = dict((key.strip(), value.strip().strip('"').strip("'")) for key, value in rows)
    name = fields.get("name")
    description = fields.get("description")
    body = parts[2].lstrip("\n").splitlines()
    sys.exit("frontmatter name must be hongkong-weather-skill, got %r" % name) if name != "hongkong-weather-skill" else None
    sys.exit("frontmatter description is empty") if not description else None
    sys.exit("SKILL.md body is %d lines, must stay under 500" % len(body)) if len(body) >= 500 else None
    print("SKILL.md ok: %d body lines" % len(body))
    PY
    for script in scripts/*.sh; do bash -n "$script"; done
    sh -n scripts/install.sh
    echo "scripts ok"

# Refresh the vendored hongkong-skill-core copy from ../hongkong-skill-core.
core-sync:
    bash scripts/sync-core.sh

# Verify the vendored core copy matches ../hongkong-skill-core (skips if absent).
core-check:
    bash scripts/check-core.sh

# Print the release tag CI would use for the current version.
version:
    @python3 -c 'import json, subprocess; out = subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"]); print("v" + json.loads(out)["packages"][0]["version"])'

# Create the annotated release tag without pushing it.
tag VERSION:
    tag="v{{ VERSION }}"; tag="${tag#vv}"; git tag -a "$tag" -m "Release $tag"; echo "created $tag"

# Tag and push, which triggers the release workflow.
release VERSION:
    tag="v{{ VERSION }}"; tag="${tag#vv}"; just tag "$tag"; git push origin "$tag"

# Remove build artefacts.
clean:
    cargo clean

# Run the checks and a release build.
ci: check doc build-release
