#!/bin/sh
# The Doctor workflow waits for its tunnel's phase (gateway_tests::doctor
# writes it to CRAKE_DOCTOR_PHASE: connected, off, done), at most SECONDS;
# it gives up at once if the run ended without reaching it (its log,
# OUT/run.log, says why).
#   scripts/doctor-wait.sh PHASE SECONDS
set -u
WANT=$1 LEFT=$2
order() { case "$1" in connected) echo 1 ;; off) echo 2 ;; done) echo 3 ;; *) echo 0 ;; esac; }
while [ "$LEFT" -gt 0 ]; do
  NOW=$(cat "$CRAKE_DOCTOR_PHASE" 2>/dev/null || true)
  [ "$(order "$NOW")" -ge "$(order "$WANT")" ] && { echo "$WANT"; exit 0; }
  if grep -q "^test result:" "$OUT/run.log" 2>/dev/null; then
    tail -n 40 "$OUT/run.log"
    echo "the run ended before \"$WANT\"" >&2
    exit 1
  fi
  sleep 5
  LEFT=$((LEFT - 5))
done
tail -n 40 "$OUT/run.log" 2>/dev/null
echo "never \"$WANT\"" >&2
exit 1
