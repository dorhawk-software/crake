#!/bin/sh
# OpenVPN for Windows (CLAUDE.md, 7i, slice 4): what Crake ships of it, in
# target/windows/openvpn/: openvpn.exe and the DLLs it needs, tapctl.exe
# (Crake makes each tunnel's adapter with it), the ovpn-dco driver (data
# channel offload, OpenVPN 2.7's default on Windows; a build for Windows 10
# and one for 11) and tap-windows6 (for gateways that can't take offload),
# both signed by Microsoft for OpenVPN Inc. and shipped unmodified. Taken
# from OpenVPN's own MSI and tap-windows6's release, pinned by checksum.
# OpenVPN is GPL-2.0 (its source: github.com/OpenVPN/openvpn at the tag).
# The script inside the container is single-quoted: no apostrophes in it.
#   scripts/fetch-openvpn-windows.sh [amd64|arm64]   (default amd64)
set -eu
cd "$(dirname "$0")/.."
ARCH=${1:-amd64}
case "$ARCH" in
  amd64)
    MSI=OpenVPN-2.7.8-I001-amd64.msi
    MSI_SHA256=1ea61a3056905b90607d4b0f084c04e5cf3043e07cf192d9cd44b0474f29cc49
    LIB=x64
    OUT=$PWD/target/windows/openvpn ;;
  arm64)
    # Windows on ARM64 (CLAUDE.md, Not now): the same release, its ARM64 MSI.
    MSI=OpenVPN-2.7.8-I001-arm64.msi
    MSI_SHA256=6fc0e8899cc5275ee297fb837bc84aa2bc49f8ef96d7ad93ee3fee1e7163a25c
    LIB=arm64
    OUT=$PWD/target/windows-arm64/openvpn ;;
  *) echo "usage: $0 [amd64|arm64]" >&2; exit 2 ;;
esac
TAP=9.27.0
TAP_SHA256=36e2609b7ceefedcb978ce5c48caf9e0e5af83423717c4e2e3c1d7ebca8f62a5
rm -rf "$OUT"
mkdir -p "$OUT"
docker run --rm -v "$OUT:/out" -e MSI="$MSI" -e MSI_SHA256="$MSI_SHA256" -e TAP="$TAP" -e ARCH="$ARCH" -e LIB="$LIB" \
  -e TAP_SHA256="$TAP_SHA256" -e HOST_UID="$(id -u)" debian:bookworm sh -euc '
  apt-get update -qq >/dev/null
  apt-get install -y -qq --no-install-recommends msitools curl ca-certificates unzip >/dev/null 2>&1
  cd /tmp
  curl -fsSL -O "https://build.openvpn.net/downloads/releases/$MSI"
  echo "$MSI_SHA256  $MSI" | sha256sum -c -
  curl -fsSL -o tap.zip "https://github.com/OpenVPN/tap-windows6/releases/download/$TAP/dist.win10.zip"
  echo "$TAP_SHA256  tap.zip" | sha256sum -c -
  mkdir msi && cd msi && msiextract "../$MSI" >/dev/null && cd ..
  mkdir -p /out/bin /out/ovpn-dco /out/tap-windows6
  for f in openvpn.exe tapctl.exe "libcrypto-3-$LIB.dll" "libssl-3-$LIB.dll" libpkcs11-helper-1.dll vcruntime140.dll; do
    cp "msi/OpenVPN/bin/$f" /out/bin/
  done
  cp -r "msi/OpenVPN/Common Files/ovpn-dco/Win10" "msi/OpenVPN/Common Files/ovpn-dco/Win11" /out/ovpn-dco/
  unzip -q tap.zip
  cp "dist.win10/$ARCH/OemVista.inf" "dist.win10/$ARCH/tap0901.cat" "dist.win10/$ARCH/tap0901.sys" /out/tap-windows6/
  echo "OpenVPN $MSI, tap-windows6 $TAP" > /out/VERSION
  printf "%s\r\n" \
    "OpenVPN for Crake: what Crake runs of OpenVPN, unmodified." \
    "" \
    "bin/: openvpn.exe and tapctl.exe from $MSI, OpenVPN Inc., GPL-2.0;" \
    "  the source: https://github.com/OpenVPN/openvpn (the release tag)." \
    "  With OpenSSL (Apache-2.0) and pkcs11-helper (BSD) as built there." \
    "ovpn-dco/: the ovpn-dco-win driver, OpenVPN Inc., MIT:" \
    "  https://github.com/OpenVPN/ovpn-dco-win" \
    "tap-windows6/: the TAP driver $TAP, OpenVPN Inc., GPL-2.0:" \
    "  https://github.com/OpenVPN/tap-windows6" > /out/README.txt
  chown -R "$HOST_UID" /out'
find "$OUT" -type f | sort
