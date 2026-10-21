#!/bin/sh
# Crake VERSION into the AUR (ssh://aur@aur.archlinux.org/crake.git): the
# PKGBUILD (packaging/arch) with that version and the release's source
# tarball's real checksum, its install script, and .SRCINFO, made in a
# throwaway Arch container, where makepkg also checks the checksum
# against the tarball before anything is pushed. Needs AUR_SSH_KEY (the
# key on the AUR account; docs/engineering/releases.md). The release workflow runs
# it; by hand for a release that's out already:
#   AUR_SSH_KEY="$(cat key)" scripts/aur-publish.sh VERSION [--dry-run]
set -eu
cd "$(dirname "$0")/.."
V=${1:?usage: scripts/aur-publish.sh VERSION [--dry-run]}
DRY=${2:-}
[ -n "${AUR_SSH_KEY:-}" ] || [ "$DRY" = --dry-run ] || { echo "AUR_SSH_KEY is empty" >&2; exit 1; }
WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-aur.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
mkdir "$WORK/pkg"
cp packaging/arch/PKGBUILD packaging/arch/crake.install "$WORK/pkg/"
# The version, and the tarball's checksum (GitHub's archive of the tag).
sed -i -e "s/^pkgver=.*/pkgver=$V/" -e 's/^pkgrel=.*/pkgrel=1/' "$WORK/pkg/PKGBUILD"
sum=$(curl -fsSL "https://github.com/dorhawk-software/crake/archive/refs/tags/v$V.tar.gz" | sha256sum | cut -d' ' -f1)
sed -i "s/^sha256sums=.*/sha256sums=('$sum')/" "$WORK/pkg/PKGBUILD"
# .SRCINFO, and the checksum checked, as an Arch user's makepkg will.
docker run --rm -v "$WORK/pkg:/pkg" -e HOST_UID="$(id -u)" archlinux:latest sh -euc '
  pacman -Sy --noconfirm --needed base-devel >/dev/null 2>&1
  useradd -m build && chown -R build /pkg
  su build -c "cd /pkg && makepkg --verifysource >/dev/null && makepkg --printsrcinfo > .SRCINFO"
  rm -f /pkg/*.tar.gz
  chown -R "$HOST_UID" /pkg'
grep -q "pkgver = $V" "$WORK/pkg/.SRCINFO" || { echo ".SRCINFO doesn't say $V" >&2; exit 1; }
if [ "$DRY" = --dry-run ]; then
    cat "$WORK/pkg/.SRCINFO"
    echo "Dry run: nothing pushed."
    exit 0
fi
# The AUR's repository: cloned (empty the very first time), updated, pushed.
printf '%s\n' "$AUR_SSH_KEY" > "$WORK/key"
chmod 600 "$WORK/key"
export GIT_SSH_COMMAND="ssh -i $WORK/key -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"
git clone -q ssh://aur@aur.archlinux.org/crake.git "$WORK/aur"
cp "$WORK/pkg/PKGBUILD" "$WORK/pkg/crake.install" "$WORK/pkg/.SRCINFO" "$WORK/aur/"
cd "$WORK/aur"
git config user.name "Crake release"
git config user.email ondrej@dorhawk.software
git add PKGBUILD crake.install .SRCINFO
if git diff --cached --quiet; then
    echo "The AUR has crake $V already."
    exit 0
fi
git commit -q -m "crake $V"
git push -q origin HEAD:master
echo "crake $V is in the AUR: https://aur.archlinux.org/packages/crake"
