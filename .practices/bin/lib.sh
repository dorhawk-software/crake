# shellcheck shell=bash disable=SC2034 # (settings read by the scripts that source this)
# Sourced by every script in .practices/bin: finds the project's root,
# loads its settings (.practices/config) over the defaults below, and
# gives the helpers the checks share.
#
# Written for bash 3.2 (macOS' /bin/bash) and BSD tools as much as GNU
# ones: no mapfile, no associative arrays, no GNU-only flags. A check that
# can't do its work fails; it never reports "clean" (in Crake until
# 2026-10-26, a privacy check let every commit through on macOS that way).

set -uo pipefail

PRACTICES_BIN=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# The project is the repository this kit sits in (.practices/bin), not
# the one the caller happens to be in.
if ! ROOT=$(git -C "$PRACTICES_BIN" rev-parse --show-toplevel 2>/dev/null); then
    echo "practices: run this inside the project's git repository" >&2
    exit 2
fi
cd "$ROOT" || exit 2

# Defaults. A project overrides any of them in .practices/config (shell
# KEY=value lines, sourced here).

# Git pathspecs (space separated) left out of every check: vendored code
# isn't the project's to fix.
EXCLUDE=':!vendor'
# Lines of code per Rust source file, and per integration test file
# (under a tests/ folder). An inline #[cfg(test)] module isn't counted.
SIZE_CODE=700
SIZE_TESTS=1000
# Folders where `unsafe` may appear (space separated); empty: nowhere.
UNSAFE_ALLOWED=''
# The git-ignored list of names that must never be committed (clients,
# their hosts and addresses), one PCRE pattern per line. "none" says the
# project has no such names: the check then passes on purpose.
PRIVATE_TERMS=dev/private-terms
# The disk the build folder is on keeps at least this many GB free.
DISK_FLOOR_GB=300
# Files (a regex on paths relative to the root) left out of the coverage
# share: test harness code that only runs outside `cargo test`.
COVERAGE_EXCLUDE=''
# Files (a regex on paths from the root) the size limit skips: generated
# code nobody edits by hand.
SIZE_EXCLUDE=''
# A JavaScript/TypeScript project (a package.json at the root): the npm
# scripts the lint gate runs after `tsc --noEmit`, and the one the
# pre-commit hook runs when JS/TS files are committed (empty: none).
NPM_CHECKS='lint'
NPM_TEST='test'

if [ -f .practices/config ]; then
    # shellcheck source=/dev/null
    . .practices/config
fi

has_rust() { [ -f "$ROOT/Cargo.toml" ]; }
has_node() { [ -f "$ROOT/package.json" ]; }

# The project's Rust files, tracked or new, without the excluded ones.
rust_files() {
    local spec
    read -r -a spec <<<"$EXCLUDE"
    git ls-files --cached --others --exclude-standard -- '*.rs' ${spec[@]+"${spec[@]}"}
}

# The project's JavaScript and TypeScript files, tracked or new, without
# the excluded ones (none in a project without a package.json).
js_files() {
    local spec
    has_node || return 0
    read -r -a spec <<<"$EXCLUDE"
    git ls-files --cached --others --exclude-standard -- \
        '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.cjs' ${spec[@]+"${spec[@]}"}
}
