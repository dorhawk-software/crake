#!/usr/bin/env bash
# Unsafe code lives in crates/crake-os alone (CLAUDE.md, the unsafe layer):
# fails if `unsafe` appears in code (not a comment) anywhere else. Each crate
# also forbids it (#![forbid(unsafe_code)]); this catches what a cfg leaves
# out, tests and build scripts included.
set -euo pipefail
cd "$(dirname "$0")/.."
found=$(grep -rnE '\bunsafe\b' crates --include='*.rs' |
    grep -v '^crates/crake-os/' |
    # The code before a line comment.
    awk -F: '{ line = $0; sub(/^[^:]*:[^:]*:/, "", line); sub(/\/\/.*/, "", line);
               if (line ~ /(^|[^a-z_])unsafe([^a-z_]|$)/) print }' || true)
if [ -n "$found" ]; then
    printf 'unsafe outside crates/crake-os (put it behind a safe function there):\n%s\n' "$found"
    exit 1
fi
