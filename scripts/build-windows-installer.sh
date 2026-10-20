#!/bin/sh
# Crake's Windows installer (packaging/windows/crake.nsi), built with NSIS
# in a throwaway container: target/windows/crake-setup-<version>.exe.
# Unsigned (signing is decided before a release, CLAUDE.md).
#
#   scripts/build-windows-installer.sh [DIR]
#
# DIR holds crake.exe, craked.exe and crake-hook.exe (default
# target/windows/app: CI's crake-windows artifact, `gh run download -n
# crake-windows -D target/windows/app`; crake.exe needs a Windows runner,
# GPUI's shaders need the Windows SDK). openconnect comes from
# target/windows/openconnect (scripts/build-openconnect-windows.sh), OpenVPN
# from target/windows/openvpn (scripts/fetch-openvpn-windows.sh).
# CRAKE_WINDOWS_ARCH=arm64 builds Crake for Windows on ARM64 from
# target/windows-arm64/ instead (its openconnect built on an ARM64 runner,
# scripts/build-openconnect-windows-arm64.sh), as crake-setup-<version>-arm64.exe.
# The script inside the container is single-quoted: no apostrophes in it.
set -eu
cd "$(dirname "$0")/.."
ARCH=${CRAKE_WINDOWS_ARCH:-amd64}
case "$ARCH" in
  amd64) BASE=target/windows SUFFIX= ;;
  arm64) BASE=target/windows-arm64 SUFFIX=-arm64 ;;
  *) echo "CRAKE_WINDOWS_ARCH is amd64 or arm64" >&2; exit 2 ;;
esac
DIR=${1:-$BASE/app}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
for f in crake.exe craked.exe crake-hook.exe; do
  [ -f "$DIR/$f" ] || { echo "no $DIR/$f" >&2; exit 1; }
done
if [ ! -f "$BASE/openconnect/openconnect.exe" ]; then
  [ "$ARCH" = amd64 ] || { echo "no $BASE/openconnect: built on an ARM64 runner" >&2; exit 1; }
  scripts/build-openconnect-windows.sh
fi
[ -f "$BASE/openvpn/bin/openvpn.exe" ] || scripts/fetch-openvpn-windows.sh "$ARCH"
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/crake-setup.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT
cp "$DIR/crake.exe" "$DIR/craked.exe" "$DIR/crake-hook.exe" packaging/windows/crake-hook.js \
  assets/icons/app/crake.ico "$STAGE/"
cp LICENSE "$STAGE/LICENSE.txt"
cp -r "$BASE/openconnect" "$STAGE/openconnect"
cp -r "$BASE/openvpn" "$STAGE/openvpn"
cp packaging/windows/crake.nsi "$STAGE/"
docker run --rm -v "$STAGE:/stage" -e VERSION="$VERSION" -e ARCH="$ARCH" -e HOST_UID="$(id -u)" debian:bookworm sh -euc '
  apt-get update -qq >/dev/null && apt-get install -y -qq --no-install-recommends nsis >/dev/null
  cd /stage
  makensis -V2 -DVERSION="$VERSION" -DARCH="$ARCH" -DSRC=/stage -DOUT=/stage/crake-setup.exe crake.nsi
  chown "$HOST_UID" crake-setup.exe'
mkdir -p "$BASE"
cp "$STAGE/crake-setup.exe" "$BASE/crake-setup-$VERSION$SUFFIX.exe"
ls -la "$BASE/crake-setup-$VERSION$SUFFIX.exe"
