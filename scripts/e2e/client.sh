#!/bin/sh
# The client's container (no network): the wire to the gateway as its only
# network, then the test, run from the host's read-only filesystem with the
# host's own openconnect, nft and ip (as scripts/netns-test.sh does). A test
# blocks UDP on the wire by creating block-udp in its TMPDIR (/mnt).
set -e
ip link set lo up
python3 /relay.py lan 198.51.100.20/24 /link/client.sock /link/gateway.sock /host/mnt/block-udp &
sleep 1
# The local network's router: everything not on the LAN goes to it.
ip route add default via 198.51.100.10
exec chroot /host env CRAKE_E2E=1 TMPDIR=/mnt "$@"
