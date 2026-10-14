#!/bin/sh
# Deletes main's caches that a newer one of the same kind replaced (CI job
# `caches`, after main's runs save theirs). A cache's kind is its key
# without the trailing hashes and dates: rust-cache's
# "v0-rust-linux-Linux-x64-<env>-<Cargo.lock>" is "v0-rust-linux-Linux-x64",
# the packages' "packages-<week>-<Cargo.lock>" is "packages". After a
# Cargo.lock change the old ones are never restored again, yet they count
# toward GitHub's 10 GB per repository; on 2026-10-14 they pushed out the
# packages cache and 0.7.0's .deb was built from scratch (17 min, not 7).
# The newest of each kind stays.
#   scripts/prune-caches.sh [--dry-run]    (GH_TOKEN with actions: write)
set -eu
DRY=
[ "${1:-}" = --dry-run ] && DRY=1
REPO=${GITHUB_REPOSITORY:-dorhawk-software/crake}
gh cache list -R "$REPO" --ref refs/heads/main --limit 100 \
    --json id,key,createdAt,sizeInBytes \
    --jq 'map(. + {kind: (.key | sub("(-[0-9a-f]+)+$"; ""))})
          | group_by(.kind)[]
          | sort_by(.createdAt) | reverse | .[1:][]
          | "\(.id) \(.sizeInBytes) \(.key)"' |
    while read -r id size key; do
        echo "replaced: $key ($((size / 1000000)) MB)"
        [ -n "$DRY" ] || gh cache delete -R "$REPO" "$id"
    done
