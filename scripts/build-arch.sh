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
cp scripts/sweep-target.sh "$B/"
# The PKGBUILD is the AUR's, as is: inside the container its cargo build
# is asked for JSON (what it used, for sweep-target.sh), in makepkg's own
# environment, by this cargo first on PATH.
cat > "$B/cargo-json" <<'EOF'
#!/bin/sh
[ "$1" = build ] || exec /usr/bin/cargo "$@"
exec /usr/bin/cargo "$@" --message-format=json-render-diagnostics >> /arch/cargo.json
EOF
# Cargo's downloads and build: Docker volumes here, a folder when CRAKE_CACHE
# names one (CI keeps it between runs).
if [ -n "${CRAKE_CACHE:-}" ]; then
  mkdir -p "$CRAKE_CACHE/arch-cargo" "$CRAKE_CACHE/arch-target"
  CACHES="-v $CRAKE_CACHE/arch-cargo:/cargo -v $CRAKE_CACHE/arch-target:/cargo-target"
else
  CACHES="-v crake-arch-cargo:/cargo -v crake-arch-target:/cargo-target"
fi
# shellcheck disable=SC2086
docker run --rm -v "$B:/arch" $CACHES -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  archlinux:latest sh -euc '
    pacman -Syu --noconfirm --needed base-devel rust cmake pkgconf fontconfig freetype2 \
      libxcb libxkbcommon libxkbcommon-x11 nftables iproute2 polkit >/dev/null 2>&1
    useradd -m builder
    chown -R builder /arch /cargo /cargo-target
    install -m755 /arch/cargo-json /usr/local/bin/cargo
    su builder -c "cd /arch/build && CARGO_HOME=/cargo CARGO_TARGET_DIR=/cargo-target CARGO_INCREMENTAL=0 makepkg -f --noconfirm --skipchecksums" >/arch/makepkg.log 2>&1 \
      || { tail -n 40 /arch/makepkg.log; exit 1; }
    sh /arch/sweep-target.sh /cargo-target /cargo /arch/cargo.json
    mv /arch/build/*.pkg.tar.zst /arch/
    chown -R "$HOST_UID:$HOST_GID" /arch'
ls "$B"/*.pkg.tar.zst
