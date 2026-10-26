#!/bin/sh
# ACME's gateways for CI's Mac (and, next, Windows) job (CLAUDE.md, 7j): the
# e2e gateway's image (scripts/e2e, its ci-start.sh) on the Linux runner,
# ocserv on 127.0.0.1:8443, openvpn over TCP on 127.0.0.1:1194, its browser
# sign-in page on 8080 and openconnect's fake servers on 4431-4434, where
# Cloudflare Tunnel (cloudflared, started by the workflow) picks them up.
#   scripts/ci-gateways.sh up OUT   the gateways; their CA and logs in OUT
#   scripts/ci-gateways.sh wait JOB… until this run's jobs JOB… have finished
#   scripts/ci-gateways.sh down OUT what they said, and they go
set -eu
cd "$(dirname "$0")/.."
NAME=crake-ci-gateways
# How long a job that uses the gateways may run, from its own start: the
# Mac's and Windows' take ~10 minutes; past this something hangs. Until
# 2026-10-25 it counted from the gateways' start, so a Windows build that
# missed its cache (40 minutes, after the 0.15.0 version bump) used it up
# and the gateways went in the middle of Windows' tests.
WAIT_MINUTES=50
# And how long the gateways wait in all, for jobs that haven't started yet
# (their builds before them): GitHub's own limit for a job is six hours.
ALL_MINUTES=180
case "${1:-}" in
  up)
    OUT=$2
    mkdir -p "$OUT"
    docker build -q -t crake-e2e-gateway scripts/e2e >/dev/null
    docker run -d --rm --name "$NAME" --cap-add NET_ADMIN --device /dev/net/tun \
      --sysctl net.ipv6.conf.all.disable_ipv6=0 --sysctl net.ipv6.conf.default.disable_ipv6=0 \
      -p 127.0.0.1:8443:443/tcp -p 127.0.0.1:1194:1194/tcp -p 127.0.0.1:8080:8080/tcp \
      -p 127.0.0.1:4431-4434:4431-4434/tcp \
      -p 127.0.0.1:8444-8445:8444-8445/tcp \
      -v "$OUT:/link" crake-e2e-gateway sh /ci-start.sh >/dev/null
    i=0
    until [ -s "$OUT/openvpn-ca.crt" ] && [ -s "$OUT/acme-user.p12" ] && curl -sk -o /dev/null https://127.0.0.1:8443; do
      i=$((i + 1))
      [ "$i" -lt 60 ] || { docker logs "$NAME"; exit 1; }
      sleep 1
    done
    echo "ACME's gateways are up"
    ;;
  wait)
    shift
    end=$(( $(date +%s) + ALL_MINUTES * 60 ))
    while [ "$(date +%s)" -lt "$end" ]; do
      # GitHub's API fails now and then (a 502, 2026-10-20): asked again
      # at the next look, not taken for the jobs' end.
      if ! jobs=$(gh api "repos/$GITHUB_REPOSITORY/actions/runs/$GITHUB_RUN_ID/jobs" --paginate \
        --jq '.jobs[] | "\(.name)\t\(.status)\t\(.started_at)"'); then
        sleep 20
        continue
      fi
      left=
      hung=
      now=$(date +%s)
      for job in "$@"; do
        # A matrix job skipped before it was expanded (its build failed)
        # keeps its unexpanded name, "windows-gateways (${{ matrix.half }})":
        # until 2026-10-26 it was never found, and the gateways waited
        # 3 hours holding the shared tunnel's lock (a gpui-update run).
        line=$(printf '%s\n' "$jobs" | awk -F '\t' -v j="$job" '
          $1 == j { print; found = 1; exit }
          index(j, " (") && index($1, "${{") && substr($1, 1, index($1, " (")) == substr(j, 1, index(j, " (")) { m = $0 }
          END { if (!found && m != "") print m }' | head -n 1)
        status=$(printf '%s' "$line" | cut -f 2)
        [ "$status" = completed ] && continue
        left="$left $job"
        # Running: its own clock; queued behind its build: none yet.
        if [ "$status" = in_progress ]; then
          started=$(date -d "$(printf '%s' "$line" | cut -f 3)" +%s 2>/dev/null || echo "$now")
          [ $((now - started)) -lt $((WAIT_MINUTES * 60)) ] || hung="$hung $job"
        fi
      done
      [ -z "$left" ] && { echo "$* done"; exit 0; }
      [ -z "$hung" ] || { echo "running for over $WAIT_MINUTES minutes:$hung" >&2; exit 1; }
      sleep 20
    done
    echo "still not done after $ALL_MINUTES minutes:$left" >&2
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
