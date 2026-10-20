#!/bin/sh
# End to end against real gateways: ocserv (AnyConnect) and openvpn, with a
# browser sign-in, in one container (scripts/e2e/), the app's sign-in,
# craked, openconnect or openvpn, nftables and policy routing in another
# (crates/crake/src/e2e_tests.rs, e2e_openvpn.rs). Neither container has a
# network: their only link is a socket pair in a temporary folder
# (scripts/e2e/relay.py). The client runs the host's own openconnect, nft and
# ip from the host's filesystem, read-only, as scripts/netns-test.sh does.
# Nothing on the host's network is touched; nothing is installed.
# CRAKE_E2E_FILTER picks the tests (e.g. "scenarios::anyconnect").
# CRAKE_E2E_KEEP=<folder> keeps the gateway's logs there.
set -eu
cd "$(dirname "$0")/.."
# These tests run the app headless (frames drawn by hand), so they don't need
# the dev profile's opt-level 1, which is there for interactive GPUI stutter.
# Building the workspace crates unoptimised cuts the crake crate's compile from
# minutes to seconds; dependencies keep their own opt-level and stay cached.
# (CI's check job hands its own settings here, as to scripts/netns-test.sh.)
: "${CARGO_PROFILE_DEV_OPT_LEVEL:=0}"
export CARGO_PROFILE_DEV_OPT_LEVEL
# shellcheck disable=SC2086
cargo build -q ${CRAKE_CARGO_SCOPE:--p craked} --bin crake-hook
HOOK=${CARGO_TARGET_DIR:-$PWD/target}/debug/crake-hook
# shellcheck disable=SC2086
BIN=$(cargo test ${CRAKE_CARGO_SCOPE:--p crake} --no-run --message-format=json 2>/dev/null \
  | sed -n 's/.*"executable":"\([^"]*\/deps\/crake-[^"]*\)".*/\1/p' | head -n1)
[ -n "$BIN" ] || { echo "could not build the tests" >&2; exit 1; }
docker build -q -t crake-e2e-gateway scripts/e2e >/dev/null
WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-e2e.XXXXXX")
GW=crake-e2e-gateway-$$
trap 'docker rm -f "$GW" >/dev/null 2>&1 || true; [ -z "${CRAKE_E2E_KEEP:-}" ] || cp -r "$WORK/link/." "$CRAKE_E2E_KEEP/"; rm -rf "$WORK"' EXIT
mkdir -p "$WORK/link" "$WORK/tmp"
docker run -d --rm --name "$GW" --network none --cap-add NET_ADMIN --device /dev/net/tun \
  -v "$WORK/link:/link" crake-e2e-gateway >/dev/null
# /host/run is the container's own: this computer's /run holds its system
# bus (openvpn3, resolved), which a test running as root must never reach.
docker run --rm --network none --cap-add NET_ADMIN --device /dev/net/tun \
  -v /:/host:ro -v "$WORK/tmp:/host/mnt" -v "$WORK/link:/link" -v "$WORK/link:/host/mnt/link:ro" --tmpfs /host/run \
  crake-e2e-gateway sh /client.sh env CRAKE_E2E_HOOK="$HOOK" CRAKE_E2E_IPSEC=1 LLVM_PROFILE_FILE=/dev/null \
  env ${CRAKE_JOURNEY_DEBUG:+CRAKE_JOURNEY_DEBUG=$CRAKE_JOURNEY_DEBUG} ${CRAKE_DEBUG:+CRAKE_DEBUG=$CRAKE_DEBUG} "$BIN" ${CRAKE_E2E_FILTER:-e2e_tests e2e_openvpn e2e_ipsec e2e_protocols scenarios journey_tests journey_ipsec} --test-threads=1 --nocapture "$@"
