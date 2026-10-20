#!/bin/sh
# The client's container (no network): the wire to the gateway as its only
# network, then the test, run from the host's read-only filesystem with the
# host's own openconnect, nft and ip (as scripts/netns-test.sh does). A test
# blocks UDP on the wire by creating block-udp in its TMPDIR (/mnt).
# The system's own systemd-resolved runs here too, on a D-Bus of the
# container's own (in its /run), with the gateway's public DNS stand-in
# (203.0.113.53) as the network's DNS, as DHCP would set it: names are
# asked as an app asks them (crates/crake/src/scenarios/).
set -e
ip link set lo up
python3 /relay.py lan 198.51.100.20/24 /link/client.sock /link/gateway.sock /host/mnt/block-udp &
sleep 1
# The local network's router: everything not on the LAN goes to it.
ip route add default via 198.51.100.10
ip -6 addr add 2001:db8:1::20/64 dev lan nodad
ip -6 route add default via 2001:db8:1::10
mkdir -p /host/run/dbus /host/run/systemd/resolve
cat > /host/run/dbus/e2e.conf <<'CONF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>system</type>
  <listen>unix:path=/run/dbus/system_bus_socket</listen>
  <auth>EXTERNAL</auth>
  <apparmor mode="disabled"/>
  <policy context="default">
    <allow user="*"/><allow own="*"/><allow send_destination="*"/><allow receive_sender="*"/>
  </policy>
</busconfig>
CONF
chroot /host chown systemd-resolve:systemd-resolve /run/systemd/resolve
chroot /host dbus-daemon --config-file=/run/dbus/e2e.conf --nofork --nopidfile > /host/run/dbus.log 2>&1 &
i=0
until [ -S /host/run/dbus/system_bus_socket ]; do
  i=$((i + 1))
  [ "$i" -lt 50 ] || { cat /host/run/dbus.log; echo "the D-Bus didn't start" >&2; exit 1; }
  sleep 0.1
done
chroot /host /usr/lib/systemd/systemd-resolved > /host/run/resolved.log 2>&1 &
i=0
until chroot /host resolvectl dns lan 203.0.113.53 2>/dev/null; do
  i=$((i + 1))
  [ "$i" -lt 50 ] || { cat /host/run/resolved.log; echo "systemd-resolved didn't start" >&2; exit 1; }
  sleep 0.1
done
exec chroot /host env CRAKE_E2E=1 TMPDIR=/mnt "$@"
