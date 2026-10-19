#!/bin/sh
# The gateway's container (no network): ACME's internal network, reachable
# only through a tunnel: its DNS (10.40.0.2) and two web servers (git,
# wiki); the wire to the client (relay.py) as the gateway's public side;
# a public web server (203.0.113.80) that only answers direct; ocserv,
# openvpn and strongSwan (IPsec) as ACME's gateways; openconnect's fake servers for the other
# protocols' sign-in.
set -e
ip link set lo up
ip link add acme type dummy
ip link set acme up
for a in 10.40.0.2 10.40.1.10 10.40.1.11 10.40.1.12 10.40.1.13 10.40.1.14; do ip addr add "$a/32" dev acme; done
ip link add inet type dummy
ip link set inet up
ip addr add 203.0.113.80/32 dev inet
python3 /relay.py lan 198.51.100.10/24 /link/gateway.sock /link/client.sock &
# openvpn and its sign-in page listen on the wire's address.
until ip -4 addr show lan 2>/dev/null | grep -q 198.51.100.10; do sleep 0.1; done
# From the client's side the internal network doesn't answer: only the
# tunnel reaches it, as with a real company.
nft -f - <<'NFT'
table ip lan {
  chain input {
    type filter hook input priority 0;
    iifname "lan" ip daddr 10.40.0.0/16 drop
    # The internet answers direct: never through a tunnel.
    iifname != "lan" iifname != "lo" ip daddr 203.0.113.80 drop
  }
}
NFT
dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces \
  --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11 \
  --address=/portal.acme.example/10.40.1.14
busybox httpd -p 80 -h /srv/www
cp /etc/openvpn/e2e/ca.crt /link/openvpn-ca.crt
openvpn --config /etc/openvpn/e2e/server.conf > /link/openvpn-server.log 2>&1 &
python3 /sso.py /run/openvpn-mgmt.sock 198.51.100.10 8080 &
# IPsec (IKEv2): charon, its tunnel ending on ipsec0 (if_id 7), where the
# pool's replies go back through.
ip link add ipsec0 type xfrm if_id 7
ip link set ipsec0 up
ip route add 10.40.9.0/24 dev ipsec0
/usr/lib/ipsec/charon > /dev/null 2>&1 &
until [ -S /var/run/charon.vici ]; do sleep 0.1; done
swanctl --load-all > /link/ipsec-load.log 2>&1
# The other protocols' sign-in (e2e_protocols.rs), one port each.
for gw in fortinet:4431 gp:4432 f5:4433 juniper:4434; do
  python3 "/fake/fake-${gw%%:*}-server.py" 198.51.100.10 "${gw##*:}" /etc/ocserv/cert.pem /etc/ocserv/key.pem \
    > "/link/fake-${gw%%:*}.log" 2>&1 &
done
exec ocserv --foreground --config /etc/ocserv/ocserv.conf
