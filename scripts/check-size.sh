#!/usr/bin/env bash
# No Rust file grows past what a person can hold in their head: at most 700
# lines of code in a source file (its #[cfg(test)] module not counted), at most
# 1000 in an integration test file. Past that, split it by topic.
# (Functions: clippy::too_many_lines, limit in clippy.toml.)
#
#   scripts/check-size.sh
#
# Run by scripts/lint.sh.
set -uo pipefail
cd "$(dirname "$0")/.."
CODE=700
TESTS=1000
fail=0
while IFS= read -r f; do
    total=$(wc -l <"$f")
    tests_at=$(grep -n '^#\[cfg(test)\]' "$f" | head -1 | cut -d: -f1)
    code=$total
    [ -n "$tests_at" ] && code=$((tests_at - 1))
    case "$f" in
        */tests/*) limit=$TESTS ;;
        *) limit=$CODE ;;
    esac
    if [ "$code" -gt "$limit" ]; then
        echo "$f: $code lines (at most $limit): split it by topic."
        fail=1
    fi
done < <(git ls-files --cached --others --exclude-standard 'crates/*.rs')
exit $fail
