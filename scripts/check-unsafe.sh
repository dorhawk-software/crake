#!/usr/bin/env bash
# Unsafe code lives in crates/crake-os alone (CLAUDE.md, the unsafe layer):
# fails if `unsafe` appears in code (not a comment) anywhere else. Each crate
# also forbids it (#![forbid(unsafe_code)]); this catches what a cfg leaves
# out, tests and build scripts included.
#
# STILL_TO_MOVE: what hasn't moved to crake-os yet, system by system. It
# shrinks to nothing as each one moves; don't add to it.
set -euo pipefail
cd "$(dirname "$0")/.."
STILL_TO_MOVE='/sys/windows/|/sys/macos/|crates/craked/tests/macos(_openvpn)?\.rs'
found=$(grep -rnE '\bunsafe\b' crates --include='*.rs' |
    grep -v '^crates/crake-os/' |
    grep -vE "$STILL_TO_MOVE" |
    # The code before a line comment.
    awk -F: '{ line = $0; sub(/^[^:]*:[^:]*:/, "", line); sub(/\/\/.*/, "", line);
               if (line ~ /(^|[^a-z_])unsafe([^a-z_]|$)/) print }' || true)
if [ -n "$found" ]; then
    printf 'unsafe outside crates/crake-os (put it behind a safe function there):\n%s\n' "$found"
    exit 1
fi
