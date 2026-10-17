#!/bin/sh
# Leaves in a Cargo target folder only what the build just done used, so
# the package cache CI keeps doesn't grow without end (scripts/build-deb.sh
# and scripts/build-arch.sh, inside their containers). Cargo lists every
# unit it built or found fresh in its JSON messages; anything in deps/,
# build/ and .fingerprint/ whose unit hash isn't among them goes: an older
# compiler's whole build, earlier versions of Crake's own crates, crates
# since updated. On 2026-10-17 the package cache had reached 6 GB that way
# (from 2.6), and the macOS cache was evicted for it. Cargo's unpacked
# registry sources go too (rust-cache does the same): they unpack again
# from the downloaded .crate files in seconds.
#   sweep-target.sh TARGET_DIR CARGO_HOME JSON...
set -eu
target=$1
home=$2
shift 2
keep=$(mktemp)
cat "$@" | grep -oE -- '-[0-9a-f]{16}' | sort -u > "$keep" || true
if [ ! -s "$keep" ]; then
  echo "sweep: Cargo listed nothing, nothing removed"
  exit 0
fi
before=$(du -sm "$target" "$home" | awk '{s += $1} END {print s}')
for dir in "$target"/*/deps "$target"/*/build "$target"/*/.fingerprint; do
  [ -d "$dir" ] || continue
  for entry in "$dir"/*; do
    hash=$(basename "$entry" | grep -oE -- '-[0-9a-f]{16}' | tail -n1) || continue
    grep -qxF -- "$hash" "$keep" || rm -rf "$entry"
  done
done
rm -rf "$home/registry/src"
after=$(du -sm "$target" "$home" | awk '{s += $1} END {print s}')
echo "sweep: $before MB before, $after MB kept"
