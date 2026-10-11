#!/bin/sh
# Inside scripts/deb-test.sh's container, cut off from every network: ACME's
# gateway (ocserv, scripts/e2e/ocserv.conf) in a network namespace of its
# own, wired to this one as the LAN (198.51.100.0/24), with ACME's internal
# network behind it as in scripts/e2e/start.sh.
set -eu
mkdir -p /etc/ocserv /srv/www
certtool --generate-privkey --outfile /etc/ocserv/key.pem --key-type=ecdsa 2>/dev/null
printf 'cn = vpn.acme.example\nexpiration_days = 30\ntls_www_server\nsigning_key\nencryption_key\n' > /tmp/cert.cfg
certtool --generate-self-signed --load-privkey /etc/ocserv/key.pem --template /tmp/cert.cfg \
  --outfile /etc/ocserv/cert.pem 2>/dev/null
printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd alice
sed 's#^socket-file = .*#socket-file = /run/ocserv-gw.socket#' /e2e/ocserv.conf > /etc/ocserv/ocserv.conf
echo crex > /srv/www/index.html

ip netns add gw
ip link add lan type veth peer name lan netns gw
ip addr add 198.51.100.20/24 dev lan
ip link set lan up
ip -n gw link set lo up
ip -n gw addr add 198.51.100.10/24 dev lan
ip -n gw link set lan up
ip -n gw link add acme type dummy
ip -n gw link set acme up
for a in 10.40.0.2 10.40.1.10 10.40.1.11; do ip -n gw addr add "$a/32" dev acme; done
ip netns exec gw nft -f - <<'NFT'
table ip lan {
  chain input {
    type filter hook input priority 0;
    iifname "lan" ip daddr 10.40.0.0/16 drop
  }
}
NFT
# The LAN's router: it doesn't know ACME's network either.
ip route add 10.40.0.0/16 via 198.51.100.10
ip netns exec gw dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces \
  --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11
ip netns exec gw busybox httpd -p 80 -h /srv/www
ip netns exec gw ocserv --config /etc/ocserv/ocserv.conf
