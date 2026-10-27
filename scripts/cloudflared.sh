#!/bin/sh
# cloudflared for CI's real-gateway jobs (CLAUDE.md, 7j), pinned by
# version and checksum (the release's own list), put at DEST.
#   scripts/cloudflared.sh linux|macos|windows DEST
set -eu
# Older than 2026.10.2 trusts eMudhra intermediates with weak keys
# (Cloudflare's notice, 2026-10-20); 2026.10.3 also fixes the macOS build,
# 2026.10.4 the metrics server's debug endpoints (VULN-176088).
# Taken at once despite the two-weeks-old habit: security fixes.
# The checksums are GitHub's own per asset: the release notes' macOS one
# is the tarball's before Cloudflare signed it.
VERSION=2026.10.4
case "$1" in
  linux) FILE=cloudflared-linux-amd64 SHA256=ae5e8add7884b0531d66bbca4570d1de491ff6da51e4ebe7eb160cdf241d9e42 ;;
  macos) FILE=cloudflared-darwin-arm64.tgz SHA256=68b6bc4080ac1330234db2a9002d07f98b000537a1bd531a687b660d4160fa7f ;;
  windows) FILE=cloudflared-windows-amd64.exe SHA256=411b16d40c8d872243efeb52c550053d00af0a8a97fbb32e730fb8aba429eaa8 ;;
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
