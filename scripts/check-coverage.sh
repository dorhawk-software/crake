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

# Lines covered, in percent: crate=floor (a list, not bash 4's
# associative array: macOS has bash 3.2). macOS has floors of its own,
# first measured there 2026-10-26: its system code runs in no test
# without root (tests/macos.rs and the journey run as root in CI), and
# Linux's own code isn't built, so the shares are lower. CI holds
# Linux's on every push.
case $(uname -s) in
    Darwin) floors="crake=67 craked=44 crake-os=34 crake-proto=98 lint-literals=82" ;;
    *) floors="crake=73 craked=63 crake-os=70 crake-proto=98 lint-literals=82" ;;
esac

# The tests that need a real gateway (CRAKE_E2E: the scenario suite, the
# e2e tests, the journeys, the gateway tests and their helpers) are test
# code `cargo test` can't run, measured by running them: left out of the
# share (2026-10-20, when the scenario suite's growth alone took crake
# under its floor; the floor then rose from 66 to 73, what crake's own code
# had without them).
harness='^crake/src/(scenarios/|e2e_|journey_|gateway_tests/|windows_tests|windows_journey|macos_journey|test_app|journey_kit)'

# Every test runs here, measured: cargo-llvm-cov's "show-env" way (cargo
# test itself, instrumented), in a folder of its own, so the build stays
# there for any tests given as arguments (CI: the namespace and e2e tests,
# which then build nothing). A failing test fails this.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$PWD/target/llvm-cov-target}"
: "${CARGO_PROFILE_DEV_OPT_LEVEL:=0}"
export CARGO_PROFILE_DEV_OPT_LEVEL
eval "$(cargo llvm-cov show-env --export-prefix 2>/dev/null)"
cargo llvm-cov clean --workspace 2>/dev/null
if ! tests=$(cargo test --workspace 2>&1); then
    printf 'cargo test failed:\n%s\n' "$tests"
    exit 1
fi
printf '%s\n' "$tests" | grep -E '^(     Running|test result:)'
summary=$(cargo llvm-cov report --summary-only 2>&1)

fail=0
for entry in $floors; do
    crate=${entry%=*}
    floor=${entry#*=}
    # Columns: file, regions (3), functions (3), lines (total, missed, cover), …
    read -r total missed < <(printf '%s\n' "$summary" | awk -v c="$crate/" -v h="$harness" '
        index($1, c) == 1 && $1 ~ /\.rs$/ && $1 !~ h { t += $8; m += $9 }
        END { print t + 0, m + 0 }')
    if [ "$total" -eq 0 ]; then
        echo "coverage: no lines found for $crate"
        fail=1
        continue
    fi
    pct=$(( (total - missed) * 100 / total ))
    if [ "$pct" -lt "$floor" ]; then
        echo "coverage: $crate is at $pct% of lines, its floor is $floor%."
        # Where: the lines no test ran, per file.
        cargo llvm-cov report --show-missing-lines 2>/dev/null | grep "/$crate/src/" |
            grep -Ev "/${harness#^}" | sed 's/^/  /'
        fail=1
    fi
done
# Then, in the same build, the tests the caller gave (their binaries run in
# containers that can't write coverage data: measured above, not here).
for extra in "$@"; do
    echo "== $extra"
    CRAKE_CARGO_SCOPE=--workspace "$extra" || fail=1
done
exit $fail
