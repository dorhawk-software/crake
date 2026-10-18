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
# The script inside the container is single-quoted: no apostrophes in it.
set -eu
cd "$(dirname "$0")/.."
DIR=${1:-target/windows/app}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
for f in crake.exe craked.exe crake-hook.exe; do
  [ -f "$DIR/$f" ] || { echo "no $DIR/$f" >&2; exit 1; }
done
[ -x target/windows/openconnect/openconnect.exe ] || scripts/build-openconnect-windows.sh
[ -f target/windows/openvpn/bin/openvpn.exe ] || scripts/fetch-openvpn-windows.sh
STAGE=$(mktemp -d "${TMPDIR:-/tmp}/crake-setup.XXXXXX")
trap 'rm -rf "$STAGE"' EXIT
cp "$DIR/crake.exe" "$DIR/craked.exe" "$DIR/crake-hook.exe" packaging/windows/crake-hook.js \
  assets/icons/app/crake.ico "$STAGE/"
cp LICENSE "$STAGE/LICENSE.txt"
cp -r target/windows/openconnect "$STAGE/openconnect"
cp -r target/windows/openvpn "$STAGE/openvpn"
cp packaging/windows/crake.nsi "$STAGE/"
docker run --rm -v "$STAGE:/stage" -e VERSION="$VERSION" -e HOST_UID="$(id -u)" debian:bookworm sh -euc '
  apt-get update -qq >/dev/null && apt-get install -y -qq --no-install-recommends nsis >/dev/null
  cd /stage
  makensis -V2 -DVERSION="$VERSION" -DSRC=/stage -DOUT=/stage/crake-setup.exe crake.nsi
  chown "$HOST_UID" crake-setup.exe'
mkdir -p target/windows
cp "$STAGE/crake-setup.exe" "target/windows/crake-setup-$VERSION.exe"
ls -la "target/windows/crake-setup-$VERSION.exe"
