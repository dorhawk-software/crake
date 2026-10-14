#!/bin/sh
# A real GPUI window on a real X server (crates/crake/tests/x11_scale.rs):
# Xvfb with a software GPU, in a throwaway container with no network. Run by
# CI's check job, in its measured build, like the netns and e2e scripts.
set -eu
cd "$(dirname "$0")/.."
: "${CARGO_PROFILE_DEV_OPT_LEVEL:=0}"
export CARGO_PROFILE_DEV_OPT_LEVEL
# shellcheck disable=SC2086
BIN=$(cargo test ${CRAKE_CARGO_SCOPE:--p crake} --test x11_scale --no-run --message-format=json 2>/dev/null \
  | sed -n 's/.*"executable":"\([^"]*x11_scale-[^"]*\)".*/\1/p')
[ -n "$BIN" ] || { echo "could not build the test" >&2; exit 1; }
. /etc/os-release
IMAGE="crake-x11-test:$VERSION_ID"
docker build -q -t "$IMAGE" --build-arg "UBUNTU=$VERSION_ID" scripts/x11 >/dev/null
docker run --rm --network none -v "$BIN:/x11_scale:ro" \
  -e CRAKE_X11_TEST=1 -e LLVM_PROFILE_FILE=/dev/null "$IMAGE" sh -c '
    Xvfb :7 -screen 0 1920x1080x24 -nolisten tcp 2>/dev/null &
    for _ in $(seq 100); do [ -e /tmp/.X11-unix/X7 ] && break; sleep 0.1; done
    DISPLAY=:7 /x11_scale'
