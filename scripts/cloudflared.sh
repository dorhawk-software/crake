#!/bin/sh
# cloudflared for CI's real-gateway jobs (CLAUDE.md, 7j), pinned by
# version and checksum (the release's own list), put at DEST.
#   scripts/cloudflared.sh linux|macos|windows DEST
set -eu
VERSION=2026.9.3
case "$1" in
  linux) FILE=cloudflared-linux-amd64 SHA256=77e26d8d900e0b8469f416239d14b5f296525fdf79fee6f511ef55609e3fbac2 ;;
  macos) FILE=cloudflared-darwin-arm64.tgz SHA256=587c2cfb1c230fe36c7fa7727da78be459dae028cabe8c001291999350f07095 ;;
  windows) FILE=cloudflared-windows-amd64.exe SHA256=f096265ec2fcbe9bb6e2d64268db167ced3fcbb83d894bdb9e2fcdb26f2ea7e2 ;;
  *) echo "usage: $0 linux|macos|windows DEST" >&2; exit 2 ;;
esac
DEST=$2
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
curl -fsSL -o "$TMP/$FILE" "https://github.com/cloudflare/cloudflared/releases/download/$VERSION/$FILE"
if command -v sha256sum >/dev/null; then
  echo "$SHA256  $TMP/$FILE" | sha256sum -c - >/dev/null
else
  echo "$SHA256  $TMP/$FILE" | shasum -a 256 -c - >/dev/null
fi
case "$FILE" in
  *.tgz) tar -xzf "$TMP/$FILE" -C "$TMP" && mv "$TMP/cloudflared" "$DEST" ;;
  *) mv "$TMP/$FILE" "$DEST" ;;
esac
chmod +x "$DEST"
