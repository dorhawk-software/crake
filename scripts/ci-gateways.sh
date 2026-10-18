#!/bin/sh
# ACME's gateways for CI's Mac (and, next, Windows) job (CLAUDE.md, 7j): the
# e2e gateway's image (scripts/e2e, its ci-start.sh) on the Linux runner,
# ocserv on 127.0.0.1:8443 and openvpn over TCP on 127.0.0.1:1194, where
# Cloudflare Tunnel (cloudflared, started by the workflow) picks them up.
#   scripts/ci-gateways.sh up OUT   the gateways; their CA and logs in OUT
#   scripts/ci-gateways.sh wait JOB… until this run's jobs JOB… have finished
#   scripts/ci-gateways.sh down OUT what they said, and they go
set -eu
cd "$(dirname "$0")/.."
NAME=crake-ci-gateways
# How long the gateways wait for the job that uses them: the Mac's job
# takes ~10 minutes; past this something hangs.
WAIT_MINUTES=50
case "${1:-}" in
  up)
    OUT=$2
    mkdir -p "$OUT"
    docker build -q -t crake-e2e-gateway scripts/e2e >/dev/null
    docker run -d --rm --name "$NAME" --cap-add NET_ADMIN --device /dev/net/tun \
      -p 127.0.0.1:8443:443/tcp -p 127.0.0.1:1194:1194/tcp \
      -v "$OUT:/link" crake-e2e-gateway sh /ci-start.sh >/dev/null
    i=0
    until [ -s "$OUT/openvpn-ca.crt" ] && curl -sk -o /dev/null https://127.0.0.1:8443; do
      i=$((i + 1))
      [ "$i" -lt 60 ] || { docker logs "$NAME"; exit 1; }
      sleep 1
    done
    echo "ACME's gateways are up"
    ;;
  wait)
    shift
    end=$(( $(date +%s) + WAIT_MINUTES * 60 ))
    while [ "$(date +%s)" -lt "$end" ]; do
      jobs=$(gh api "repos/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID/jobs" --paginate \
        --jq '.jobs[] | "\(.name) \(.status)"')
      left=
      for job in "$@"; do
        echo "$jobs" | grep -qx "$job completed" || left="$left $job"
      done
      [ -z "$left" ] && { echo "$* done"; exit 0; }
      sleep 20
    done
    echo "still going after $WAIT_MINUTES minutes:$left" >&2
    exit 1
    ;;
  down)
    OUT=$2
    docker logs "$NAME" 2>&1 | tail -n 40 || true
    tail -n 40 "$OUT/openvpn-server.log" 2>/dev/null || true
    docker rm -f "$NAME" >/dev/null 2>&1 || true
    ;;
  *)
    echo "usage: $0 up OUT | wait JOB | down OUT" >&2
    exit 2
    ;;
esac
