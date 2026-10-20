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
    ocserv gnutls-bin dnsmasq-base busybox nftables openvpn openssl python3 > /dev/null
lan=$(ip -o link | awk -F': ' '/52:54:00:c0:ff:10/ {print $2}')
ip addr add 198.51.100.10/24 dev "$lan"
ip -6 addr add fd00:1a::10/64 dev "$lan" nodad
ip link set "$lan" up
# A host on the internet, reached over IPv6 by the LAN's router (this one).
ip link add net type dummy
ip link set net up
ip -6 addr add 2001:db8:77::1/128 dev net nodad

mkdir -p /etc/ocserv /srv/www
certtool --generate-privkey --outfile /etc/ocserv/key.pem --key-type=ecdsa 2>/dev/null
printf 'cn = vpn.acme.example\nexpiration_days = 30\ntls_www_server\nsigning_key\nencryption_key\n' > /tmp/cert.cfg
certtool --generate-self-signed --load-privkey /etc/ocserv/key.pem --template /tmp/cert.cfg \
    --outfile /etc/ocserv/cert.pem 2>/dev/null
printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd alice
cp /test/e2e/ocserv.conf /etc/ocserv/ocserv.conf
# IPv6 too: an address for the client, and ACME's IPv6 network behind;
# and a DNS server reached over IPv6, listed first (asked first).
printf 'ipv6-network = fd40:ff::/64\nipv6-subnet-prefix = 128\nroute = fd40:1::/64\n' \
    >> /etc/ocserv/ocserv.conf
sed -i 's/^dns = 10.40.0.2$/dns = fd40::2\ndns = 10.40.0.2/' /etc/ocserv/ocserv.conf
echo crex > /srv/www/index.html

ip link add acme type dummy
ip link set acme up
for a in 10.40.0.2 10.40.1.10 10.40.1.11 10.40.1.12; do ip addr add "$a/32" dev acme; done
ip -6 addr add fd40:1::10/128 dev acme nodad
ip -6 addr add fd40::2/128 dev acme nodad
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
# ACME's DNS reached over IPv6, which also knows dns6.acme.internal (the
# IPv4 one doesn't: a name that only resolves if the IPv6 one is asked).
dnsmasq --no-resolv --no-hosts --listen-address=fd40::2 --bind-interfaces \
    --pid-file=/run/dnsmasq-acme6.pid \
    --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11 \
    --address=/files.acme.example/10.40.1.12 --address=/v6.acme.internal/fd40:1::10 \
    --address=/dns6.acme.internal/10.40.1.10
# The LAN's own (public) DNS, which knows no name at all.
dnsmasq --no-resolv --no-hosts --listen-address=198.51.100.10 --bind-interfaces --local=/#/
busybox httpd -p 80 -h /srv/www
systemctl stop ocserv.socket ocserv.service 2>/dev/null || true
ocserv --config /etc/ocserv/ocserv.conf
# A second gateway on 8443 that hands out no IPv6 (a full tunnel without
# it refuses IPv6 rather than send it around the tunnel).
sed -e 's/^tcp-port = 443$/tcp-port = 8443/' -e 's/^udp-port = 443$/udp-port = 8443/' \
    -e 's|^socket-file = .*|socket-file = /run/ocserv-b.socket|' -e 's/^device = vpns$/device = vpnb/' \
    -e 's|^ipv4-network = .*|ipv4-network = 10.41.255.0/24|' /test/e2e/ocserv.conf > /etc/ocserv/b.conf
printf 'pid-file = /run/ocserv-b.pid\nuse-occtl = false\n' >> /etc/ocserv/b.conf
ocserv --config /etc/ocserv/b.conf
# ACME's OpenVPN gateway too (scripts/e2e/openvpn.conf, its sign-in
# sso.py: alice's password), its CA on the web server for the test.
mkdir -p /etc/openvpn/e2e
cd /etc/openvpn/e2e
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout ca.key -out ca.crt -days 30 -subj /CN=ACME-CA 2>/dev/null
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout server.key -out server.csr -subj /CN=vpn.acme.example 2>/dev/null
printf 'extendedKeyUsage=serverAuth\nkeyUsage=digitalSignature,keyAgreement\n' > server.ext
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 30 -extfile server.ext 2>/dev/null
cp ca.crt /srv/www/openvpn-ca.crt
cd /
systemctl stop openvpn.service 2>/dev/null || true
openvpn --config /test/e2e/openvpn.conf > /tmp/openvpn-gw.log 2>&1 &
python3 /test/e2e/sso.py /run/openvpn-mgmt.sock 198.51.100.10 8080 /tmp/openvpn-seen > /dev/null 2>&1 &
say "GATEWAY-READY"
