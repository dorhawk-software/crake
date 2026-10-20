#!/bin/sh
# CI's Mac or Windows reaches ACME's gateways in the run's `gateways` job
# through Cloudflare Tunnel (CLAUDE.md, 7j): cloudflared carries each to a
# port on 127.0.0.1 (with the repository's service token), and the tests in
# crates/crake/src/scenarios/ and gateway_tests/ connect to them. macOS: craked in the
# test, as root, with Homebrew's openconnect and openvpn. Windows: the
# installer (SETUP) puts Crake's service, openconnect and openvpn there,
# and the tests (TESTS, the app's test binary) drive it.
#   scripts/ci-through-cloudflare.sh macos
#   scripts/ci-through-cloudflare.sh windows SETUP TESTS
# Needs CF_ACCESS_CLIENT_ID, CF_ACCESS_CLIENT_SECRET, GH_TOKEN.
# CRAKE_GATEWAY_FILTER picks the tests (default the gateway tests and that
# system's journey, the regression suite; the Doctor workflow:
# gateway_tests::doctor, with its CRAKE_DOCTOR_* settings).
set -eu
cd "$(dirname "$0")/.."
# The gateways' names in Cloudflare (each a TCP route of the tunnel).
ANYCONNECT_HOST=crake-ci-anyconnect.dorhawk.software
OPENVPN_HOST=crake-ci-openvpn.dorhawk.software
# OpenVPN's browser sign-in page, and openconnect's fake servers for the
# protocols without an open-source gateway (e2e_protocols.rs), each on the
# port the gateways job publishes it on.
SSO_HOST=crake-ci-sso.dorhawk.software
FAKE_HOSTS="crake-ci-fortinet:4431 crake-ci-gp:4432 crake-ci-f5:4433 crake-ci-pulse:4434"
# How long the tunnel gets to carry the gateways once they're up (5 s a
# try), and the gateways job, queued behind other runs', to be up at all
# (10 s a try: an hour).
READY_TRIES=60
WAIT_TRIES=360
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
access "$SSO_HOST" 8080
for fake in $FAKE_HOSTS; do
  access "${fake%%:*}.dorhawk.software" "${fake##*:}"
done
# The gateways job hands its OpenVPN CA over as an artifact once they're up.
# It waits its turn for the one tunnel behind other runs' (2026-10-19: 20
# minutes behind a run started by hand, while this waited 10 and failed),
# so this follows the job: waiting while it's queued or running, giving up
# once it ended without the artifact. GitHub keeps one queued job per
# group and cancels the one before when another queues: a newer run took
# this one's turn and tests the gateways itself, so a branch skips them
# (said as a warning); main, whose run a release waits on, fails.
gateways_job() {
  gh run view "$GITHUB_RUN_ID" -R "$GITHUB_REPOSITORY" --json jobs \
    --jq '.jobs[] | select(.name == "gateways") | .status + " " + .conclusion' 2>/dev/null || true
}
i=0
until gh run download "$GITHUB_RUN_ID" -R "$GITHUB_REPOSITORY" -n gateways-ready -D "$WORK/ready" >/dev/null 2>&1; do
  i=$((i + 1))
  job=$(gateways_job)
  case "$job" in
    "completed cancelled")
      if [ "${GITHUB_REF:-}" = refs/heads/main ]; then
        echo "the gateways job was cancelled: another run took its turn" >&2
        exit 1
      fi
      echo "::warning::The gateways job was cancelled (a newer run took its turn with the tunnel); the real-gateway tests are skipped in this run."
      exit 0
      ;;
    completed*)
      # Its artifact may land a moment after it ends: one more look.
      gh run download "$GITHUB_RUN_ID" -R "$GITHUB_REPOSITORY" -n gateways-ready -D "$WORK/ready" >/dev/null 2>&1 && break
      echo "the gateways job ended ($job) without saying they're up" >&2
      exit 1
      ;;
  esac
  [ "$i" -lt "$WAIT_TRIES" ] || { echo "the gateways never said they're up (their job: ${job:-not found})" >&2; exit 1; }
  sleep 10
done
i=0
until curl -sk -o /dev/null https://127.0.0.1:8443; do
  i=$((i + 1))
  [ "$i" -lt "$READY_TRIES" ] || { cat "$WORK"/access-*.log; echo "no gateway through Cloudflare" >&2; exit 1; }
  sleep 5
done
# Words, each a filter: a test that matches any runs.
# The newer gateways, where Cloudflare lets CI reach them (their hostnames
# in the Access application): the tests that use them skip otherwise.
SSO= FAKES=
curl -s -o /dev/null --max-time 20 http://127.0.0.1:8080/ && SSO=127.0.0.1:8080
curl -sk -o /dev/null --max-time 20 https://127.0.0.1:4431/ && FAKES=127.0.0.1
echo "browser sign-in page: ${SSO:-not reached}; fake servers: ${FAKES:-not reached}"
FILTER=${CRAKE_GATEWAY_FILTER:-scenarios gateway_tests ${SYSTEM}_journey e2e_protocols::each_protocols_sign_in}
DOCTOR="CRAKE_DOCTOR_TUNNEL=${CRAKE_DOCTOR_TUNNEL:-} CRAKE_DOCTOR_MINUTES=${CRAKE_DOCTOR_MINUTES:-} CRAKE_DOCTOR_PHASE=${CRAKE_DOCTOR_PHASE:-}"
[ -n "${CRAKE_DOCTOR_PHASE:-}" ] || DOCTOR=
status=0
if [ "$SYSTEM" = macos ]; then
  cargo build -q --workspace --locked --bin crake-hook --bin craked
  HOOK=$PWD/target/debug/crake-hook
  BIN=$(cargo test --workspace --locked --no-run --message-format=json 2>/dev/null |
    jq -r 'select(.executable != null and .target.name == "crake" and .profile.test) | .executable' | head -n1)
  [ -n "$BIN" ] || { echo "could not build the tests" >&2; exit 1; }
  sudo env CRAKE_E2E=1 CRAKE_E2E_GATEWAY=https://127.0.0.1:8443 \
    CRAKE_GATEWAY_SSO="$SSO" CRAKE_FAKE_GATEWAY="$FAKES" \
    CRAKE_GATEWAY_OPENVPN="127.0.0.1 1194" CRAKE_GATEWAY_CA="$WORK/ready/openvpn-ca.crt" \
    CRAKE_GATEWAY_USER_CERT="$WORK/ready/acme-user.p12" \
    CRAKE_E2E_HOOK="$HOOK" CRAKE_E2E_CRAKED="$PWD/target/debug/craked" $DOCTOR \
    "$BIN" $FILTER --test-threads=1 --nocapture || status=$?
else
  CRAKE_E2E=1 CRAKE_E2E_GATEWAY=https://127.0.0.1:8443 \
    CRAKE_GATEWAY_SSO="$SSO" CRAKE_FAKE_GATEWAY="$FAKES" \
    CRAKE_GATEWAY_OPENVPN="127.0.0.1 1194" CRAKE_GATEWAY_CA="$WORK/ready/openvpn-ca.crt" \
    CRAKE_GATEWAY_USER_CERT="$WORK/ready/acme-user.p12" \
    CRAKE_OPENCONNECT="C:/Program Files/Crake/openconnect/openconnect.exe" \
    "$3" $FILTER --test-threads=1 --nocapture || status=$?
  [ "$status" = 0 ] || tail -n 60 "C:/ProgramData/Crake/craked.log"
fi
[ "$status" = 0 ] || tail -n 30 "$WORK"/access-*.log
exit "$status"
