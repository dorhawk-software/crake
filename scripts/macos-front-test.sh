#!/bin/sh
# Crake's windows open in front, on a real Mac (CI job macos). Crake is a
# menu bar app whose panel doesn't make it the active app, so its windows
# opened behind the app in front. This starts the installed Crake.app in
# demo mode in the background (open -g: the app in front stays in front),
# opening Settings at start; Crake must then be the app in front.
# Usage: macos-front-test.sh [--expect-behind]
#   --expect-behind: a Crake from before the fix (0.3.0), where Settings
#   must stay behind: the check itself tells the two apart.
set -eu
APP=/Applications/Crake.app
WAIT=15

front() {
    lsappinfo info -only name "$(lsappinfo front)" 2>/dev/null || true
}

pkill -x crake 2>/dev/null || true
before=$(front)
open -g --env CRAKE_DEMO=1 --env CRAKE_SHOW=settings "$APP"
infront=no
for _ in $(seq "$WAIT"); do
    sleep 1
    case "$(front)" in
        *'"Crake"'*)
            infront=yes
            break
            ;;
    esac
done
pkill -x crake 2>/dev/null || true
echo "in front before: ${before:-nothing}; Crake in front after opening Settings: $infront"
if [ "${1:-}" = --expect-behind ]; then
    [ "$infront" = no ] || {
        echo "FAIL: this Crake should have opened Settings behind; the check can't tell" >&2
        exit 1
    }
    echo "macOS front (before the fix): Settings opened behind, as expected"
else
    [ "$infront" = yes ] || {
        echo "FAIL: Crake's Settings opened behind ${before:-the app in front}" >&2
        exit 1
    }
    echo "macOS front: OK"
fi
