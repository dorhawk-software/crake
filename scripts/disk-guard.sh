#!/usr/bin/env bash
# Keeps at least 300 GB free on the disk Crake builds on (asked by the user,
# 2026-10-19, after the build folder filled it a second time). Under the
# floor it clears the compiler's incremental caches (cheap to lose); if
# that isn't enough it fails, saying what takes the room, so whoever is
# building stops and asks before going on. Claude Code runs it after every
# shell command (scripts/claude-lint-hook.sh).
#
#   CRAKE_DISK_FLOOR_GB=300   the floor, in GB
set -uo pipefail
cd "$(dirname "$0")/.."
FLOOR_GB=${CRAKE_DISK_FLOOR_GB:-300}
# POSIX df (-Pk: 1024-byte blocks, "Available" in column 4), as GNU's
# -BG/--output aren't in macOS' df.
free_gb() { df -Pk . | awk 'NR == 2 { print int($4 / 1024 / 1024) }'; }
[ "$(free_gb)" -ge "$FLOOR_GB" ] && exit 0
rm -rf target/debug/incremental target/llvm-cov-target/debug/incremental \
    target/release/incremental
free=$(free_gb)
if [ "$free" -ge "$FLOOR_GB" ]; then
    echo "disk: cleared the incremental caches; ${free} GB free again."
    exit 0
fi
printf 'disk: %s GB free, under the %s GB floor. Stop and ask the user before building more.\nWhat Crake'"'"'s builds take:\n%s\n' \
    "$free" "$FLOOR_GB" "$(du -sh target/* 2>/dev/null | sort -rh | head -n 6)"
exit 1
