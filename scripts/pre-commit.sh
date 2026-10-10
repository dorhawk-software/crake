#!/usr/bin/env bash
# git's pre-commit hook: nothing customer-specific (scripts/check-private.sh),
# and, when Rust files are committed, coverage at or above its floors
# (scripts/check-coverage.sh).
#
#   ln -s ../../scripts/pre-commit.sh .git/hooks/pre-commit
set -uo pipefail
cd "$(dirname "$(readlink -f "$0")")/.."

scripts/check-private.sh || exit 1
if git diff --cached --name-only --diff-filter=ACMR | grep -q '\.rs$'; then
    scripts/check-coverage.sh || exit 1
fi
