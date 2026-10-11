#!/bin/sh
# craked compiles for macOS, lint-clean, tests included (the macOS plan in
# CLAUDE.md: the OS parts live in crates/craked/src/sys/macos/). In a
# throwaway container with Rust's aarch64-apple-darwin target added, so
# nothing is installed here. Type-checking needs no Mac; the app does (C
# parts need Apple's toolchain): CI job macos checks it on GitHub's
# runner, and runs the tests.
set -eu
cd "$(dirname "$0")/.."
exec docker run --rm -v "$PWD:/src:ro" -v crake-mac-cargo:/usr/local/cargo/registry \
  -v crake-mac-target:/target -e CARGO_TARGET_DIR=/target rust:1-bookworm sh -c \
  "rustup target add aarch64-apple-darwin >/dev/null 2>&1 && rustup component add clippy >/dev/null 2>&1 &&
   cd /src && cargo clippy --target aarch64-apple-darwin -p craked -p crake-proto --all-targets"
