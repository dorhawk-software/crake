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
echo crex > /srv/www/index.html

ip link add acme type dummy
ip link set acme up
for a in 10.40.0.2 10.40.1.10 10.40.1.11; do ip addr add "$a/32" dev acme; done
nft -f - <<NFT
table ip lan {
  chain input {
    type filter hook input priority 0;
    iifname "$lan" ip daddr 10.40.0.0/16 drop
  }
}
NFT
dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces \
    --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11
busybox httpd -p 80 -h /srv/www
systemctl stop ocserv.socket ocserv.service 2>/dev/null || true
ocserv --config /etc/ocserv/ocserv.conf
say "GATEWAY-READY"
