#!/bin/sh
# ACME's gateways for CI's Mac and Windows jobs (CLAUDE.md, 7j;
# scripts/ci-gateways.sh): the same internal network as start.sh behind
# ocserv and openvpn, but the container has a network of its own (on the
# Linux runner) and Cloudflare Tunnel carries clients to it, so openvpn
# listens over TCP (the tunnel carries nothing else) and the internal
# network answers only through a tunnel.
set -e
ip link add acme type dummy
ip link set acme up
for a in 10.40.0.2 10.40.1.10 10.40.1.11 10.40.1.12; do ip addr add "$a/32" dev acme; done
nft -f - <<'NFT'
table ip lan {
  chain input {
    type filter hook input priority 0;
    iifname "eth0" ip daddr 10.40.0.0/16 drop
  }
}
NFT
dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces \
  --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11
busybox httpd -p 80 -h /srv/www
cp /etc/openvpn/e2e/ca.crt /link/openvpn-ca.crt
sed -e 's/^proto udp$/proto tcp-server/' -e '/^local /d' /etc/openvpn/e2e/server.conf > /run/server.conf
openvpn --config /run/server.conf > /link/openvpn-server.log 2>&1 &
python3 /sso.py /run/openvpn-mgmt.sock 0.0.0.0 8080 /link/openvpn-seen &
# The other protocols' sign-in (e2e_protocols.rs), one port each, as start.sh.
for gw in fortinet:4431 gp:4432 f5:4433 juniper:4434; do
  python3 "/fake/fake-${gw%%:*}-server.py" 0.0.0.0 "${gw##*:}" /etc/ocserv/cert.pem /etc/ocserv/key.pem \
    > "/link/fake-${gw%%:*}.log" 2>&1 &
done
exec ocserv --foreground --config /etc/ocserv/ocserv.conf
