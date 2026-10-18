#!/bin/sh
# CI's Mac or Windows reaches ACME's gateways in the run's `gateways` job
# through Cloudflare Tunnel (CLAUDE.md, 7j): cloudflared carries each to a
# port on 127.0.0.1 (with the repository's service token), and the tests in
# crates/crake/src/gateway_tests.rs connect to them. macOS: craked in the
# test, as root, with Homebrew's openconnect and openvpn. Windows: the
# installer (SETUP) puts Crake's service, openconnect and openvpn there,
# and the tests (TESTS, the app's test binary) drive it.
#   scripts/ci-through-cloudflare.sh macos
#   scripts/ci-through-cloudflare.sh windows SETUP TESTS
# Needs CF_ACCESS_CLIENT_ID, CF_ACCESS_CLIENT_SECRET, GH_TOKEN.
set -eu
cd "$(dirname "$0")/.."
# The gateways' names in Cloudflare (each a TCP route of the tunnel).
ANYCONNECT_HOST=crake-ci-anyconnect.dorhawk.software
OPENVPN_HOST=crake-ci-openvpn.dorhawk.software
# How long the gateways job gets to be up, and the tunnel to carry them.
READY_TRIES=60
SYSTEM=$1
WORK=${RUNNER_TEMP:-/tmp}/through-cloudflare
mkdir -p "$WORK"
if [ "$SYSTEM" = macos ]; then
  brew install openconnect openvpn >/dev/null
else
  # Silent, as a company's software deployment would: the service and both
  # engines' drivers (crake.nsi).
  powershell -NoProfile -Command "\$p = Start-Process '$2' -ArgumentList '/S' -Wait -PassThru; exit \$p.ExitCode"
  # craked's debug lines in its log, for a failure's message (as the VM
  # test sets them).
  powershell -NoProfile -Command "Set-ItemProperty HKLM:\\SYSTEM\\CurrentControlSet\\Services\\craked -Name Environment -Type MultiString -Value 'RUST_LOG=craked=debug,info'; Restart-Service craked"
fi
CLOUDFLARED=$WORK/cloudflared
[ "$SYSTEM" = macos ] || CLOUDFLARED=$CLOUDFLARED.exe
sh scripts/cloudflared.sh "$SYSTEM" "$CLOUDFLARED"
access() {
  "$CLOUDFLARED" access tcp --hostname "$1" --url "127.0.0.1:$2" \
    --service-token-id "$CF_ACCESS_CLIENT_ID" --service-token-secret "$CF_ACCESS_CLIENT_SECRET" \
    > "$WORK/access-$2.log" 2>&1 &
}
access "$ANYCONNECT_HOST" 8443
access "$OPENVPN_HOST" 1194
# The gateways job hands its OpenVPN CA over as an artifact once they're up.
i=0
until gh run download "$GITHUB_RUN_ID" -R "$GITHUB_REPOSITORY" -n gateways-ready -D "$WORK/ready" >/dev/null 2>&1; do
  i=$((i + 1))
  [ "$i" -lt "$READY_TRIES" ] || { echo "the gateways never said they're up" >&2; exit 1; }
  sleep 10
done
i=0
until curl -sk -o /dev/null https://127.0.0.1:8443; do
  i=$((i + 1))
  [ "$i" -lt "$READY_TRIES" ] || { cat "$WORK"/access-*.log; echo "no gateway through Cloudflare" >&2; exit 1; }
  sleep 5
done
status=0
if [ "$SYSTEM" = macos ]; then
  cargo build -q --workspace --locked --bin crake-hook --bin craked
  HOOK=$PWD/target/debug/crake-hook
  BIN=$(cargo test --workspace --locked --no-run --message-format=json 2>/dev/null |
    jq -r 'select(.executable != null and .target.name == "crake" and .profile.test) | .executable' | head -n1)
  [ -n "$BIN" ] || { echo "could not build the tests" >&2; exit 1; }
  sudo env CRAKE_E2E=1 CRAKE_E2E_GATEWAY=https://127.0.0.1:8443 \
    CRAKE_GATEWAY_OPENVPN="127.0.0.1 1194" CRAKE_GATEWAY_CA="$WORK/ready/openvpn-ca.crt" \
    CRAKE_E2E_HOOK="$HOOK" CRAKE_E2E_CRAKED="$PWD/target/debug/craked" \
    "$BIN" gateway_tests --test-threads=1 --nocapture || status=$?
else
  CRAKE_E2E=1 CRAKE_E2E_GATEWAY=https://127.0.0.1:8443 \
    CRAKE_GATEWAY_OPENVPN="127.0.0.1 1194" CRAKE_GATEWAY_CA="$WORK/ready/openvpn-ca.crt" \
    CRAKE_OPENCONNECT="C:/Program Files/Crake/openconnect/openconnect.exe" \
    "$3" gateway_tests --test-threads=1 --nocapture || status=$?
  [ "$status" = 0 ] || tail -n 60 "C:/ProgramData/Crake/craked.log"
fi
[ "$status" = 0 ] || tail -n 30 "$WORK"/access-*.log
exit "$status"
