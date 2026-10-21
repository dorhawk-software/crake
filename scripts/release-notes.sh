#!/bin/sh
# A release's text: CHANGELOG.md's section for VERSION ("## 0.1.0
# (2026-10-14)"), without its heading. Fails when there's none (an
# undescribed release), or when the version in Cargo.toml, the Nix package
# or the PKGBUILD disagrees with it. Used by .github/workflows/release.yml.
#   scripts/release-notes.sh VERSION
# With --which, only the table that ends it: which file is for which
# computer (for the release's page, and the release run's summary).
#   scripts/release-notes.sh --which VERSION
set -eu
cd "$(dirname "$0")/.."
which_file() {
    d="https://github.com/${GITHUB_REPOSITORY:-dorhawk-software/crake}/releases/download/v$1"
    cat <<EOF
### Which file?

| Your computer | Download | Or, updated with the rest of the system |
| --- | --- | --- |
| Windows 11, Intel or AMD | [crake-setup-$1.exe]($d/crake-setup-$1.exe) | |
| Windows 11 on ARM | [crake-setup-$1-arm64.exe]($d/crake-setup-$1-arm64.exe) | |
| Mac | [Crake-$1.pkg]($d/Crake-$1.pkg) | \`brew install --cask dorhawk-software/tap/crake\` |
| Ubuntu 22.04 and newer, Debian 12 and 13 | [crake_$1_amd64.deb]($d/crake_$1_amd64.deb) | [the apt repository](https://packages.dorhawk.software) |
| Fedora 43 and newer | [crake-$1-1.x86_64.rpm]($d/crake-$1-1.x86_64.rpm) | [the dnf repository](https://packages.dorhawk.software) |
EOF
}
if [ "$1" = --which ]; then
    which_file "$2"
    exit 0
fi
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
# Under a rule: the kinds (### New, …) are the release's own headings.
printf '\n---\n\n'
which_file "$V"
