#!/bin/sh
# A release's text: CHANGELOG.md's section for VERSION ("## 0.1.0
# (2026-10-14)"), without its heading. Fails when there's none (an
# undescribed release), or when the version in Cargo.toml, the Nix package
# or the PKGBUILD disagrees with it. Used by .github/workflows/release.yml.
#   scripts/release-notes.sh VERSION
set -eu
cd "$(dirname "$0")/.."
V=$1
fail() {
    echo "$*" >&2
    exit 1
}
cargo=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
nix=$(sed -n 's/^ *version = "\(.*\)";/\1/p' packaging/nix/package.nix | head -n1)
arch=$(sed -n 's/^pkgver=//p' packaging/arch/PKGBUILD)
for found in "Cargo.toml:$cargo" "packaging/nix/package.nix:$nix" "packaging/arch/PKGBUILD:$arch"; do
    [ "${found#*:}" = "$V" ] || fail "${found%%:*} says ${found#*:}, the tag $V"
done
notes=$(awk -v v="$V" '
    index($0, "## " v " (") == 1 { on = 1; next }
    on && /^## / { exit }
    on { print }
' CHANGELOG.md)
[ -n "$(printf '%s' "$notes" | tr -d '[:space:]')" ] ||
    fail "CHANGELOG.md has no section \"## $V (<date>)\""
printf '%s\n' "$notes" | sed -e '/./,$!d'
