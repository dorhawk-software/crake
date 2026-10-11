#!/bin/sh
# The gateway's container (no network): ACME's internal network, reachable
# only through the tunnel: its DNS (10.40.0.2) and two web servers (git,
# wiki); the wire to the client (relay.py) as the gateway's public side.
set -e
ip link set lo up
ip link add acme type dummy
ip link set acme up
for a in 10.40.0.2 10.40.1.10 10.40.1.11; do ip addr add "$a/32" dev acme; done
python3 /relay.py lan 198.51.100.10/24 /link/gateway.sock /link/client.sock &
# From the client's side the internal network doesn't answer: only the
# tunnel reaches it, as with a real company.
nft -f - <<'NFT'
table ip lan {
  chain input {
    type filter hook input priority 0;
    iifname "lan" ip daddr 10.40.0.0/16 drop
  }
}
NFT
dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces \
  --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11
busybox httpd -p 80 -h /srv/www
exec ocserv --foreground --config /etc/ocserv/ocserv.conf
