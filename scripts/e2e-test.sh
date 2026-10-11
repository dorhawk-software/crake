#!/bin/sh
# End to end against a real AnyConnect gateway: ocserv in one container
# (scripts/e2e/), the app's sign-in, craked, openconnect, nftables and policy
# routing in another (crates/crake/src/e2e_tests.rs). Neither container has a
# network: their only link is a socket pair in a temporary folder
# (scripts/e2e/relay.py). The client runs the host's own openconnect, nft and
# ip from the host's filesystem, read-only, as scripts/netns-test.sh does.
# Nothing on the host's network is touched; nothing is installed.
set -eu
cd "$(dirname "$0")/.."
cargo build -q -p craked --bin crake-hook
HOOK=$PWD/target/debug/crake-hook
BIN=$(cargo test -p crake --no-run --message-format=json 2>/dev/null \
  | sed -n 's/.*"executable":"\([^"]*\/deps\/crake-[^"]*\)".*/\1/p' | head -n1)
[ -n "$BIN" ] || { echo "could not build the tests" >&2; exit 1; }
docker build -q -t crake-e2e-gateway scripts/e2e >/dev/null
WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-e2e.XXXXXX")
GW=crake-e2e-gateway-$$
trap 'docker rm -f "$GW" >/dev/null 2>&1 || true; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/link" "$WORK/tmp"
docker run -d --rm --name "$GW" --network none --cap-add NET_ADMIN --device /dev/net/tun \
  -v "$WORK/link:/link" crake-e2e-gateway >/dev/null
docker run --rm --network none --cap-add NET_ADMIN --device /dev/net/tun \
  -v /:/host:ro -v "$WORK/tmp:/host/mnt" -v "$WORK/link:/link" \
  crake-e2e-gateway sh /client.sh env CRAKE_E2E_HOOK="$HOOK" \
  env ${CRAKE_JOURNEY_DEBUG:+CRAKE_JOURNEY_DEBUG=$CRAKE_JOURNEY_DEBUG} ${CRAKE_DEBUG:+CRAKE_DEBUG=$CRAKE_DEBUG} "$BIN" e2e_tests journey_tests --test-threads=1 --nocapture "$@"
