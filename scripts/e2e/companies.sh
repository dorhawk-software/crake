#!/bin/sh
# The companies behind the test gateways (start.sh here, ci-start.sh on
# CI's Linux runner): ACME's internal network (10.40.0.0/16: its DNS at
# 10.40.0.2, git, wiki and the rest), Globex's (10.41.0.0/16: its DNS at
# 10.41.0.2, git.globex.internal), and one host both reach (10.42.0.10),
# as two companies' networks can both reach a supplier. None answers from
# the wire ($1, the gateway's own side): only through a tunnel, as with a
# real company. Globex's answers only Globex's users (ocserv's bob, whose
# addresses ocserv-users/bob gives), ACME's everyone else.
set -e
lan=$1
dummy() {
  name=$1
  shift
  ip link add "$name" type dummy
  ip link set "$name" up
  for a in "$@"; do ip addr add "$a/32" dev "$name"; done
}
dummy acme 10.40.0.2 10.40.1.10 10.40.1.11 10.40.1.12 10.40.1.13 10.40.1.14 10.40.1.15 10.40.1.16 10.40.1.17 10.40.1.18
# ACME numbers one inside host from its own public range (192.0.2.40), as
# some companies do: still only through a tunnel.
ip addr add 192.0.2.40/32 dev acme
dummy globex 10.41.0.2 10.41.1.10
dummy shared 10.42.0.10
# ACME's IPv6 host (v6.acme.internal), reached only through a tunnel too.
ip -6 addr add fd40:1::10/128 dev acme nodad
nft -f - <<NFT
table ip companies {
  chain input {
    type filter hook input priority 0;
    # 10.40.1.17 from the wire is turned away as a router does, "host
    # unreachable" (the probe starts at once: scenarios' hosts.rs).
    iifname "$lan" ip daddr 10.40.1.17 reject with icmp type host-unreachable
    iifname "$lan" ip daddr { 10.40.0.0/16, 10.41.0.0/16, 10.42.0.0/16, 192.0.2.40 } drop
    iifname "lo" accept
    ip daddr 10.41.0.0/16 ip saddr != 10.41.255.0/24 drop
    ip daddr 10.40.0.0/16 ip saddr 10.41.255.0/24 drop
  }
}
NFT
nft -f - <<NFT
table ip6 companies6 {
  chain input {
    type filter hook input priority 0;
    iifname "$lan" ip6 daddr fd40::/16 drop
  }
}
NFT
# A UDP service of ACME's (10.40.1.18:7007, an echo), for a UDP host
# learned through a tunnel (the scenarios' checks.rs).
python3 -c '
import socket
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind(("10.40.1.18", 7007))
while True:
    data, peer = s.recvfrom(2048)
    s.sendto(data, peer)
' &
# shop.acme.example and (for CI, whose gateway is named under
# dorhawk.software) packages.dorhawk.software have public addresses too and
# private ones here: split-horizon. intra.acme.example and (CI)
# www.dorhawk.software have public addresses too, and here another public
# one, ACME's own (192.0.2.40).
dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces --pid-file=/run/dnsmasq-acme.pid \
  --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11 \
  --address=/portal.acme.example/10.40.1.14 --address=/files.acme.example/10.40.1.15 \
  --address=/shop.acme.example/10.40.1.16 --address=/v6.acme.internal/fd40:1::10 \
  --address=/packages.dorhawk.software/10.40.1.16 \
  --address=/intra.acme.example/192.0.2.40 --address=/www.dorhawk.software/192.0.2.40
# dup.globex.internal shares an address with ACME's git: one address, two
# companies, which routing by address can't serve both ways at once.
dnsmasq --no-resolv --no-hosts --listen-address=10.41.0.2 --bind-interfaces --pid-file=/run/dnsmasq-globex.pid \
  --address=/git.globex.internal/10.41.1.10 --address=/dup.globex.internal/10.40.1.10
