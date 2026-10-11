#!/bin/sh
# craked compiles for Windows, lint-clean (the Windows plan in CLAUDE.md:
# its OS parts live in crates/craked/src/sys/, Windows ones are stubs until
# phase 3). Uses the installed x86_64-pc-windows-gnu target when there is
# one (CI); otherwise a throwaway container, so nothing is installed here.
set -eu
cd "$(dirname "$0")/.."
TARGET=x86_64-pc-windows-gnu
CHECK="cargo clippy --target $TARGET -p craked -p crake-proto --all-targets"
if rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  exec $CHECK
fi
exec docker run --rm -v "$PWD:/src:ro" -v crake-win-cargo:/usr/local/cargo/registry \
  -v crake-win-target:/target -e CARGO_TARGET_DIR=/target rust:1-bookworm sh -c "
    rustup target add $TARGET >/dev/null 2>&1 && rustup component add clippy >/dev/null 2>&1 &&
    cd /src && $CHECK"
