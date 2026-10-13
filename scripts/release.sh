#!/bin/sh
# Release Crake VERSION (CLAUDE.md, Releases): CHANGELOG.md's "Unreleased"
# section becomes "## VERSION (today)", the version goes into Cargo.toml
# (and Cargo.lock), the Nix package and the PKGBUILD, that is committed
# and pushed to main, and once CI has passed on the commit the tag
# vVERSION is pushed: .github/workflows/release.yml takes it from there
# (builds, signs, publishes, the cask, packages.dorhawk.software).
# Interrupted while waiting for CI, or after pushing a fix CI asked for,
# run it again with the same VERSION: it waits for CI on main's latest
# commit and tags that one.
#   scripts/release.sh VERSION            e.g. scripts/release.sh 0.3.0
#   scripts/release.sh --dry-run VERSION  the changes made here and shown;
#                                         nothing committed or pushed
set -eu
cd "$(dirname "$0")/.."
DRY=
if [ "${1:-}" = --dry-run ]; then
    DRY=1
    shift
fi
V=${1:?usage: scripts/release.sh [--dry-run] VERSION}
fail() {
    echo "release: $*" >&2
    exit 1
}
echo "$V" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || fail "$V isn't a version like 0.3.0"
git rev-parse -q --verify "refs/tags/v$V" >/dev/null && fail "v$V is tagged already"
[ "$(git branch --show-current)" = main ] || fail "not on main"
git fetch -q origin main
current=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)

if [ "$current" = "$V" ]; then
    [ -z "$DRY" ] || fail "the version is $V already: nothing to try"
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || fail "push main first"
    echo "Crake $V is committed already: waiting for CI on main's latest commit."
else
    [ -z "$(git status --porcelain)" ] || fail "uncommitted changes"
    [ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || fail "main isn't the same as origin/main"
    # What "Unreleased" says: the release's text. Refused when empty.
    notes=$(awk '/^## Unreleased/ { on = 1; next } on && /^## / { exit } on { print }' CHANGELOG.md)
    [ -n "$(printf '%s' "$notes" | tr -d '[:space:]')" ] ||
        fail "CHANGELOG.md has nothing under \"## Unreleased\": write what changed for users first"
    today=$(date +%F)
    sed -i "s/^## Unreleased\$/## Unreleased\n\n## $V ($today)/" CHANGELOG.md
    sed -i "0,/^version = \"$current\"/s//version = \"$V\"/" Cargo.toml
    sed -i "s/^\( *version = \)\"$current\";/\1\"$V\";/" packaging/nix/package.nix
    sed -i "s/^pkgver=$current\$/pkgver=$V/" packaging/arch/PKGBUILD
    cargo metadata --format-version 1 >/dev/null
    # The release workflow's own check, before anything is pushed.
    scripts/release-notes.sh "$V" >/dev/null
    if [ -n "$DRY" ]; then
        git --no-pager diff --stat
        git --no-pager diff CHANGELOG.md
        echo "Dry run: the changes above are in the working tree; \"git checkout -- .\" takes them back."
        exit 0
    fi
    git add CHANGELOG.md Cargo.toml Cargo.lock packaging/nix/package.nix packaging/arch/PKGBUILD
    printf 'Crake %s\n\n%s\n' "$V" "$(printf '%s' "$notes" | sed -e '/./,$!d')" | git commit -q -F -
    git push -q origin main
    echo "Crake $V committed and pushed: waiting for CI."
fi

sha=$(git rev-parse HEAD)
# CI's run for the commit (it may take a moment to appear), then its end.
run=""
while [ -z "$run" ]; do
    run=$(gh run list --workflow ci.yml --commit "$sha" --json databaseId --jq '.[0].databaseId // empty')
    [ -n "$run" ] || sleep 15
done
echo "CI: https://github.com/dorhawk-software/crake/actions/runs/$run"
while [ "$(gh run view "$run" --json status --jq .status)" != completed ]; do
    sleep 60
done
if [ "$(gh run view "$run" --json conclusion --jq .conclusion)" != success ]; then
    gh run view "$run" --json jobs --jq '.jobs[] | select(.conclusion != "success") | "  \(.name): \(.conclusion)"' >&2
    fail "CI didn't pass on $sha (no tag was pushed): push a fix to main, then run this again"
fi
git tag -a "v$V" -m "Crake $V"
git push -q origin "v$V"
echo "v$V pushed: the release workflow publishes it."
echo "  https://github.com/dorhawk-software/crake/actions/workflows/release.yml"
