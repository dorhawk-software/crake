#!/bin/sh
# The gateway's container (no network): ACME's internal network and
# Globex's, reachable only through a tunnel (companies.sh); the wire to the
# client (relay.py) as the gateway's public side; a public web server
# (203.0.113.80) that only answers direct, and the network's public DNS
# (203.0.113.53), which knows a few public names (the gateway's own,
# vpn.acme.example, and two more of ACME's) and logs every question (so a
# test sees what left the client); ocserv, openvpn and strongSwan (IPsec)
# as the companies' gateways; openconnect's fake servers for the other
# protocols' sign-in.
set -e
ip link set lo up
ip link add inet type dummy
ip link set inet up
ip addr add 203.0.113.80/32 dev inet
ip addr add 203.0.113.53/32 dev inet
python3 /relay.py lan 198.51.100.10/24 /link/gateway.sock /link/client.sock &
# openvpn and its sign-in page listen on the wire's address.
until ip -4 addr show lan 2>/dev/null | grep -q 198.51.100.10; do sleep 0.1; done
# The companies' networks, which only a tunnel reaches.
sh /companies.sh lan
nft -f - <<'NFT'
table ip lan {
  chain input {
    type filter hook input priority 0;
    # The internet answers direct: never through a tunnel.
    iifname != "lan" iifname != "lo" ip daddr { 203.0.113.53, 203.0.113.80 } drop
  }
}
NFT
dnsmasq --no-resolv --no-hosts --listen-address=203.0.113.53 --bind-interfaces \
  --pid-file=/run/dnsmasq-public.pid --log-queries --log-facility=/link/public-dns.log \
  --address=/one.one.one.one/203.0.113.80 --address=/vpn.acme.example/198.51.100.10 \
  --address=/shop.acme.example/203.0.113.80 --address=/www.acme.example/203.0.113.80 --address=/#/
busybox httpd -p 80 -h /srv/www
cp /etc/openvpn/e2e/ca.crt /link/openvpn-ca.crt
# carol's certificate, for the client's certificate sign-in.
cp /etc/ocserv/carol.p12 /link/acme-user.p12
chmod a+r /link/acme-user.p12
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
# ACME's gateway again on 8444, behind a front that admits only clients
# calling themselves AnyConnect (7d).
python3 /picky.py 198.51.100.10 8444 /etc/ocserv/cert.pem /etc/ocserv/key.pem 443 \
  > /link/picky.log 2>&1 &
exec ocserv --foreground --log-stderr --config /etc/ocserv/ocserv.conf > /link/ocserv.log 2>&1
