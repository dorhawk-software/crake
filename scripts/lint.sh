#!/usr/bin/env bash
# Every check that must pass before a build counts: rustfmt's layout, clippy
# with all lints as errors (set in Cargo.toml's [workspace.lints]),
# cargo-deny (known vulnerabilities, licences, banned and unknown sources) and
# cargo-machete (unused dependencies), no client names in what would be committed
# (scripts/check-private.sh), and no file or function past its size limit
# (scripts/check-size.sh, clippy::too_many_lines), and Crake's patched GPUI
# the one built (scripts/check-gpui-patch.sh). Exits non-zero if any fails.
#
#   scripts/lint.sh            # run it by hand
#
# Claude Code runs it after every `cargo build` / `cargo test` through the
# project hook in .claude/settings.json (scripts/claude-lint-hook.sh).
set -uo pipefail
cd "$(dirname "$0")/.."

fail=0
step() {
    local name=$1
    shift
    if ! out=$("$@" 2>&1); then
        printf '%s failed:\n%s\n\n' "$name" "$out"
        fail=1
    fi
}

step "rustfmt (run cargo fmt)" cargo fmt --all --check
step "clippy" cargo clippy --workspace --all-targets --quiet
step "cargo deny" cargo deny --log-level error check
step "cargo machete" cargo machete
step "private names" scripts/check-private.sh
step "file sizes" scripts/check-size.sh
step "patched GPUI in use" scripts/check-gpui-patch.sh

exit $fail
