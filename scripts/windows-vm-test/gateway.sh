#!/bin/sh
# Inside scripts/windows-vm-test.sh's gateway VM (Ubuntu), run by cloud-init
# as root: ACME's gateway, as scripts/deb-test/gateway.sh builds it, but on a
# machine of its own. Its first network card reaches Ubuntu's archive (for
# ocserv; nothing is forwarded); the second is the wire to the Windows VM,
# the LAN 198.51.100.0/24, where it is also the router, one that doesn't
# know ACME's network: only the tunnel reaches 10.40.0.0/16.
set -eu
say() { echo "$*" > /dev/ttyS1; }
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
    ocserv gnutls-bin dnsmasq-base busybox nftables > /dev/null
lan=$(ip -o link | awk -F': ' '/52:54:00:c0:ff:10/ {print $2}')
ip addr add 198.51.100.10/24 dev "$lan"
ip link set "$lan" up

mkdir -p /etc/ocserv /srv/www
certtool --generate-privkey --outfile /etc/ocserv/key.pem --key-type=ecdsa 2>/dev/null
printf 'cn = vpn.acme.example\nexpiration_days = 30\ntls_www_server\nsigning_key\nencryption_key\n' > /tmp/cert.cfg
certtool --generate-self-signed --load-privkey /etc/ocserv/key.pem --template /tmp/cert.cfg \
    --outfile /etc/ocserv/cert.pem 2>/dev/null
printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd alice
cp /test/e2e/ocserv.conf /etc/ocserv/ocserv.conf
# IPv6 too: an address for the client, and ACME's IPv6 network behind.
printf 'ipv6-network = fd40:ff::/64\nipv6-subnet-prefix = 128\nroute = fd40:1::/64\n' \
    >> /etc/ocserv/ocserv.conf
echo crex > /srv/www/index.html

ip link add acme type dummy
ip link set acme up
for a in 10.40.0.2 10.40.1.10 10.40.1.11 10.40.1.12; do ip addr add "$a/32" dev acme; done
ip -6 addr add fd40:1::10/128 dev acme nodad
nft -f - <<NFT
table ip lan {
  chain input {
    type filter hook input priority 0;
    iifname "$lan" ip daddr 10.40.0.0/16 drop
  }
}
NFT
# ACME's DNS, which also knows a name outside its domain: files.acme.example.
dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces \
    --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11 \
    --address=/files.acme.example/10.40.1.12 --address=/v6.acme.internal/fd40:1::10
# The LAN's own (public) DNS, which knows no name at all.
dnsmasq --no-resolv --no-hosts --listen-address=198.51.100.10 --bind-interfaces --local=/#/
busybox httpd -p 80 -h /srv/www
systemctl stop ocserv.socket ocserv.service 2>/dev/null || true
ocserv --config /etc/ocserv/ocserv.conf
say "GATEWAY-READY"
