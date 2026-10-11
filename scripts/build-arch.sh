#!/bin/sh
# Build crake's Arch package (packaging/arch/PKGBUILD) in an Arch container,
# from the current commit instead of a release tarball (git archive HEAD,
# named as the PKGBUILD's source so makepkg takes it as downloaded). The
# result is target/arch/crake-<version>-1-x86_64.pkg.tar.zst. Test it with
# scripts/deb-test.sh archlinux:latest. Commit first: uncommitted changes
# aren't in the build.
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
grep -q "^pkgver=$VERSION$" packaging/arch/PKGBUILD || { echo "PKGBUILD's pkgver isn't $VERSION" >&2; exit 1; }
B=$PWD/target/arch
rm -rf "$B" && mkdir -p "$B/build"
git archive --format=tar.gz --prefix="crake-$VERSION/" HEAD > "$B/build/crake-$VERSION.tar.gz"
cp packaging/arch/PKGBUILD packaging/arch/crake.install "$B/build/"
docker run --rm -v "$B:/arch" -v crake-arch-cargo:/cargo -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  archlinux:latest sh -euc '
    pacman -Syu --noconfirm --needed base-devel rust cmake pkgconf fontconfig freetype2 \
      libxcb libxkbcommon libxkbcommon-x11 nftables iproute2 polkit >/dev/null 2>&1
    useradd -m builder
    chown -R builder /arch /cargo
    su builder -c "cd /arch/build && CARGO_HOME=/cargo makepkg -f --noconfirm --skipchecksums" >/arch/makepkg.log 2>&1 \
      || { tail -n 40 /arch/makepkg.log; exit 1; }
    mv /arch/build/*.pkg.tar.zst /arch/
    chown -R "$HOST_UID:$HOST_GID" /arch'
ls "$B"/*.pkg.tar.zst
