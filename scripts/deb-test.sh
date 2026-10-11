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
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
DEB=$PWD/target/deb/crake_${VERSION}_amd64.deb
[ -f "$DEB" ] || { echo "no $DEB: run scripts/build-deb.sh" >&2; exit 1; }
[ $# -gt 0 ] || set -- 22.04 24.04 26.04
fail=0
NAME=
trap '[ -z "$NAME" ] || docker rm -f "$NAME" >/dev/null 2>&1' EXIT
for REL in "$@"; do
  echo "== Ubuntu $REL"
  docker build -q --build-arg RELEASE="$REL" -t "crake-deb-test:$REL" scripts/deb-test >/dev/null
  NAME=crake-deb-test-$$-$REL
  # Its own cgroup namespace, made writable for systemd inside it only.
  docker run -d --name "$NAME" --cgroupns=private --tmpfs /run --tmpfs /run/lock --tmpfs /tmp \
    --cap-add SYS_ADMIN --cap-add NET_ADMIN --device /dev/net/tun --security-opt apparmor=unconfined \
    --sysctl net.ipv4.ip_unprivileged_port_start=1024 \
    -v "$DEB:/pkg/crake.deb:ro" -v "$PWD/scripts/deb-test:/deb-test:ro" -v "$PWD/scripts/e2e:/e2e:ro" \
    "crake-deb-test:$REL" sh -c 'mount -o remount,rw /sys/fs/cgroup && exec /sbin/init' >/dev/null
  until docker exec "$NAME" systemctl is-system-running --wait >/dev/null 2>&1 \
     || [ "$(docker exec "$NAME" systemctl is-system-running 2>/dev/null)" = degraded ]; do sleep 1; done
  docker exec "$NAME" sh /deb-test/check.sh install || fail=1
  docker network disconnect bridge "$NAME"
  docker exec "$NAME" sh /deb-test/check.sh tunnel || fail=1
  docker exec "$NAME" sh /deb-test/check.sh remove || fail=1
  docker rm -f "$NAME" >/dev/null
done
exit $fail
