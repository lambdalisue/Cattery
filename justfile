# Run `nix develop` (or use direnv) first so every tool is on PATH.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes.
default:
    @just --list

# Format every source.
fmt:
    cargo fmt --all
    nix fmt flake.nix

# Check formatting without changing files.
fmt-check:
    cargo fmt --all --check

# Lint with every warning as an error.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Run the tests.
test:
    cargo test --workspace

# Stamp a copy of the manifests the way a release does, so a placeholder the
# release workflow would no longer find fails here, on every OS.
stamp-check:
    tmp=$(mktemp -d) && cp Cargo.toml Cargo.lock "$tmp" && perl scripts/stamp-version.pl 0.0.0-ci "$tmp"

# Everything CI runs.
ci: fmt-check lint test stamp-check

# Build Cattery for release and package it as a GitHub Release carries it,
# into dist/. Run outside the Nix shell, as the release does: a build made
# inside it refers into /nix/store, which the bundle refuses.
bundle version="0.0.0-ci":
    cargo build --release --locked -p cattery
    scripts/bundle.sh {{version}} target/release/cattery{{ if os_family() == "windows" { ".exe" } else { "" } }} dist

# Make the identity releases sign Cattery.app with, into a new folder, for
# the secrets of the GitHub environment "release".
[macos]
macos-release-identity folder:
    scripts/macos/release-identity.sh {{folder}}

# Import the identity releases sign with, given as MACOS_SIGNING_P12 and
# MACOS_SIGNING_PASSWORD, into the login keychain, so `bundle` signs with it
# too.
[macos]
macos-import-identity:
    scripts/macos/import-identity.sh
