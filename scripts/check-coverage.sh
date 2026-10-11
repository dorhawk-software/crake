#!/usr/bin/env bash
# Coverage may not fall: each crate's share of lines run by `cargo test`
# (cargo-llvm-cov) must stay at or above its floor. Raise a floor when the
# tests have grown past it; never lower one to let a change through, add the
# tests instead.
#
#   scripts/check-coverage.sh
#
# About a minute, so it isn't part of scripts/lint.sh (run after every build);
# scripts/pre-commit.sh runs it when Rust files are committed.
set -uo pipefail
cd "$(dirname "$0")/.."

# Lines covered, in percent.
declare -A floor=(
    [crake]=66
    [craked]=63
    [crake-proto]=95
)

if ! summary=$(cargo llvm-cov --workspace --summary-only 2>&1); then
    printf 'cargo llvm-cov failed:\n%s\n' "$summary"
    exit 1
fi

fail=0
for crate in "${!floor[@]}"; do
    # Columns: file, regions (3), functions (3), lines (total, missed, cover), …
    read -r total missed < <(printf '%s\n' "$summary" | awk -v c="$crate/" '
        index($1, c) == 1 && $1 ~ /\.rs$/ { t += $8; m += $9 }
        END { print t + 0, m + 0 }')
    if [ "$total" -eq 0 ]; then
        echo "coverage: no lines found for $crate"
        fail=1
        continue
    fi
    pct=$(( (total - missed) * 100 / total ))
    if [ "$pct" -lt "${floor[$crate]}" ]; then
        echo "coverage: $crate is at $pct% of lines, its floor is ${floor[$crate]}%."
        # Where: the lines no test ran, per file.
        cargo llvm-cov report --show-missing-lines 2>/dev/null | grep "/$crate/src/" | sed 's/^/  /'
        fail=1
    fi
done
exit $fail
