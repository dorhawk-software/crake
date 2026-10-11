#!/bin/sh
# Install crake's .deb (scripts/build-deb.sh) in throwaway containers that
# boot systemd, one per Ubuntu release, and check it (scripts/deb-test/
# check.sh): apt resolves its Depends, craked runs as a service on the
# system bus, polkit guards it, a real tunnel works through the installed
# service (ocserv in a namespace of the container's own, after the container
# is cut off from every network), and removing it leaves nothing. Never on
# this machine. The containers reach Ubuntu's archive for the Depends.
#
#   scripts/deb-test.sh [release...]     # default: 22.04 24.04 26.04
#                                        # (Ubuntu), or e.g. debian:12,
#                                        # fedora:44 (the RPM),
#                                        # archlinux:latest (scripts/build-arch.sh)
#
# Arch has no ocserv outside the AUR: there the gateway runs in a second
# container, e2e-test.sh's (no network either), wired to this one by
# scripts/e2e/relay.py through a socket in a temporary folder.
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
DEB=$PWD/target/deb/crake_${VERSION}_amd64.deb
# The journey's test binary, from scripts/build-deb.sh (run after the tunnel).
JOURNEY=$PWD/target/deb/journey
RPMF=$PWD/target/rpm/crake-${VERSION}-1.x86_64.rpm
[ $# -gt 0 ] || set -- 22.04 24.04 26.04
ARCHP=$(ls "$PWD"/target/arch/crake-"$VERSION"-*-x86_64.pkg.tar.zst 2>/dev/null | head -n1)
fail=0
NAME=
GW=
LINK=$(mktemp -d "${TMPDIR:-/tmp}/crake-link.XXXXXX")
trap 'for n in $NAME $GW; do docker rm -f "$n" >/dev/null 2>&1; done; rm -rf "$LINK"' EXIT
for REL in "$@"; do
  case "$REL" in *:*) IMAGE=$REL ;; *) IMAGE=ubuntu:$REL ;; esac
  case "$IMAGE" in
    fedora:*) PKG=$RPMF; AS=/pkg/crake.rpm; HINT=scripts/build-rpm.sh ;;
    archlinux:*) PKG=${ARCHP:-$PWD/target/arch/crake.pkg.tar.zst}; AS=/pkg/crake.pkg.tar.zst; HINT=scripts/build-arch.sh ;;
    *) PKG=$DEB; AS=/pkg/crake.deb; HINT=scripts/build-deb.sh ;;
  esac
  [ -f "$PKG" ] || { echo "no $PKG: run $HINT" >&2; exit 1; }
  TAG=$(echo "$IMAGE" | tr ':' '-')
  echo "== $IMAGE"
  docker build -q --build-arg IMAGE="$IMAGE" -t "crake-deb-test:$TAG" scripts/deb-test >/dev/null
  NAME=crake-deb-test-$$-$TAG
  # Its own cgroup namespace, made writable for systemd inside it only.
  docker run -d --name "$NAME" --cgroupns=private --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cap-add SYS_ADMIN --cap-add NET_ADMIN --device /dev/net/tun --security-opt apparmor=unconfined \
    --sysctl net.ipv4.ip_unprivileged_port_start=1024 \
    -v "$PKG:$AS:ro" -v "$PWD/scripts/deb-test:/deb-test:ro" -v "$PWD/scripts/e2e:/e2e:ro" \
    $([ -f "$JOURNEY" ] && echo "-v $JOURNEY:/pkg/journey:ro") \
    $(case "$IMAGE" in archlinux:*) echo "-v $LINK:/link" ;; esac) \
    "crake-deb-test:$TAG" sh -c 'mount -o remount,rw /sys/fs/cgroup && exec /sbin/init' >/dev/null
  until docker exec "$NAME" systemctl is-system-running --wait >/dev/null 2>&1 \
     || [ "$(docker exec "$NAME" systemctl is-system-running 2>/dev/null)" = degraded ]; do sleep 1; done
  docker exec "$NAME" sh /deb-test/check.sh install || fail=1
  docker network disconnect bridge "$NAME"
  case "$IMAGE" in archlinux:*)
    docker build -q -t crake-e2e-gateway scripts/e2e >/dev/null
    GW=crake-deb-test-gw-$$
    docker run -d --rm --name "$GW" --network none --cap-add NET_ADMIN --device /dev/net/tun \
      -v "$LINK:/link" crake-e2e-gateway >/dev/null ;;
  esac
  docker exec "$NAME" sh /deb-test/check.sh tunnel || fail=1
  docker exec "$NAME" sh /deb-test/check.sh journey || fail=1
  docker exec "$NAME" sh /deb-test/check.sh remove || fail=1
  docker rm -f "$NAME" >/dev/null
  [ -z "$GW" ] || { docker rm -f "$GW" >/dev/null 2>&1; GW=; }
done
exit $fail
