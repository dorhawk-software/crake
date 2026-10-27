#!/bin/sh
# cloudflared for CI's real-gateway jobs (CLAUDE.md, 7j), pinned by
# version and checksum (the release's own list), put at DEST.
#   scripts/cloudflared.sh linux|macos|windows DEST
set -eu
# Older than 2026.10.2 trusts eMudhra intermediates with weak keys
# (Cloudflare's notice, 2026-10-20); 2026.10.3 also fixes the macOS build.
# Taken at once despite the two-weeks-old habit: a security fix.
# The checksums are GitHub's own per asset: the release notes' macOS one
# is the tarball's before Cloudflare signed it.
VERSION=2026.10.3
case "$1" in
  linux) FILE=cloudflared-linux-amd64 SHA256=053b80b0a23031a038fdbf3c073f59f82b8f1f152aa67971d7383ffafc0dbab0 ;;
  macos) FILE=cloudflared-darwin-arm64.tgz SHA256=1165d14b6f6360a3b8c283d7d1100024d6ec7cd9bc9e32519e96150645f3ec55 ;;
  windows) FILE=cloudflared-windows-amd64.exe SHA256=76b2805253d0d6ffacb092a7acfbee99e7af6917835c0c5410e23f3ef5052da2 ;;
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
