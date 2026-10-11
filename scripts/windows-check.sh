#!/bin/sh
# Crake compiles for Windows, lint-clean, tests included (the Windows plan
# in CLAUDE.md: the OS parts live in crates/*/src/sys/). Uses the installed
# x86_64-pc-windows-gnu target when there is one (CI, with MinGW's windres
# for GPUI's manifest); otherwise a throwaway container
# (scripts/windows/Dockerfile), so nothing is installed here. The .exe
# files are built on a Windows runner (CI job windows-build): GPUI's
# release shaders need the Windows SDK's fxc.
set -eu
cd "$(dirname "$0")/.."
TARGET=x86_64-pc-windows-gnu
CHECK="cargo clippy --target $TARGET --workspace --all-targets"
if rustup target list --installed 2>/dev/null | grep -qx "$TARGET"; then
  exec $CHECK
fi
docker build -q -t crake-windows-build scripts/windows >/dev/null
exec docker run --rm -v "$PWD:/src:ro" -v crake-win-cargo:/usr/local/cargo/registry \
  -v crake-win-target:/target -e CARGO_TARGET_DIR=/target crake-windows-build \
  sh -c "cd /src && $CHECK"
