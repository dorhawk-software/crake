#!/bin/sh
# Inside the FortiGate's client VM (scripts/fortigate-vm.sh client): plain
# strongSwan, by hand, against the FortiGate's IKEv2 dial-up gateway, before
# Crake drives it (CLAUDE.md, 7f). Reports to the second serial port.
set -u
say() { echo "== $*"; }
status=1
finish() { echo "CLIENT-RESULT: $status"; }
trap finish EXIT
# The cable to the FortiGate's port2 (its MAC set by the script).
LAN=$(ip -o link | awk -F': ' '/52:54:00:c0:02:0a/ {print $2}')
ip link set "$LAN" up
ip addr add 192.0.2.10/24 dev "$LAN"
say "the gateway on the cable"
ping -c 2 -W 2 192.0.2.1 || { say "no FortiGate on port2"; exit 1; }
say "ACME's address before the tunnel (should not answer)"
ping -c 1 -W 2 10.40.0.1 && { say "10.40.0.1 answers without a tunnel: proves nothing"; exit 1; }
say "strongSwan from Ubuntu's archive"
# EAP-MSCHAPv2 (the password) is in extauth, EAP-Identity (the FortiGate
# asks for the user's name first) in extra, which is universe's.
export DEBIAN_FRONTEND=noninteractive NEEDRESTART_SUSPEND=1
{ apt-get update -q && apt-get install -y -q strongswan-swanctl charon-systemd \
    libcharon-extauth-plugins libcharon-extra-plugins bind9-dnsutils; } > /tmp/apt.log 2>&1 ||
  { tail -n 20 /tmp/apt.log; exit 1; }
swanctl --version
cp /test/acme.conf /etc/swanctl/conf.d/acme.conf
systemctl restart strongswan
sleep 2
swanctl --load-all
say "IKEv2 to the FortiGate"
swanctl --initiate --child acme --timeout 30 || { journalctl -u strongswan --no-pager | tail -n 40; exit 1; }
swanctl --list-sas
say "what the gateway handed over"
ip -o addr show
ip xfrm policy | head -n 20
say "ACME's address through the tunnel"
ping -c 3 -W 2 10.40.0.1 || exit 1
say "the DNS the gateway handed over, and a name only it knows"
journalctl -u strongswan --no-pager | grep -i -e dns -e "virtual IP" | tail -n 5
dig +short +time=2 @10.40.0.1 wiki.acme.internal | grep -qx 10.40.0.1 || { dig @10.40.0.1 wiki.acme.internal; exit 1; }
echo "wiki.acme.internal is 10.40.0.1"
status=0
