#!/bin/sh
# Run the nftables/routing test in a throwaway network namespace: a disposable
# container with no network, the host filesystem read-only, and the test using
# the host's own nft and ip. Nothing on the host's network is touched.
set -eu
cd "$(dirname "$0")/.."
BINS=$(cargo test -p craked --test netns --test dns_netns --test probe_netns --no-run --message-format=json 2>/dev/null \
  | sed -n 's/.*"executable":"\([^"]*netns-[^"]*\)".*/\1/p')
[ -n "$BINS" ] || { echo "could not build the tests" >&2; exit 1; }
WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-netns.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
# One fresh container (fresh namespace) per test binary.
for BIN in $BINS; do
  echo "== $(basename "$BIN")"
  # Strict reverse-path filtering, as on Fedora (the tests check it's on).
  docker run --rm --network none --cap-add NET_ADMIN --device /dev/net/tun \
    --sysctl net.ipv4.conf.all.rp_filter=1 --sysctl net.ipv4.conf.default.rp_filter=1 \
    -v /:/host:ro -v "$WORK:/host/mnt" \
    -e CRAKE_NETNS_TEST=1 -e TMPDIR=/mnt \
    alpine:3 chroot /host "$BIN" --test-threads=1 "$@"
done
