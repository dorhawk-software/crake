#!/bin/sh
# Inside scripts/deb-test.sh's container, cut off from every network: ACME's
# gateways (ocserv, scripts/e2e/ocserv.conf, and openvpn with its sign-in,
# scripts/e2e/openvpn.conf and sso.py) in a network namespace of their
# own, wired to this one as the LAN (198.51.100.0/24), with ACME's internal
# network behind them as in scripts/e2e/start.sh.
set -eu
# Arch: the gateway is a second container (scripts/deb-test.sh); this one
# only joins the wire to it.
if [ -d /link ]; then
    python3 /e2e/relay.py lan 198.51.100.20/24 /link/client.sock /link/gateway.sock >/dev/null 2>&1 &
    sleep 1
    ip route add 10.40.0.0/16 via 198.51.100.10
    exit 0
fi
mkdir -p /etc/ocserv /srv/www
certtool --generate-privkey --outfile /etc/ocserv/key.pem --key-type=ecdsa 2>/dev/null
printf 'cn = vpn.acme.example\nexpiration_days = 30\ntls_www_server\nsigning_key\nencryption_key\n' > /tmp/cert.cfg
certtool --generate-self-signed --load-privkey /etc/ocserv/key.pem --template /tmp/cert.cfg \
  --outfile /etc/ocserv/cert.pem 2>/dev/null
printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd alice
# Ubuntu's nogroup is Fedora's nobody: whatever nobody's group is here.
sed -e 's#^socket-file = .*#socket-file = /run/ocserv-gw.socket#' \
    -e "s#^run-as-group = .*#run-as-group = $(id -gn nobody)#" /e2e/ocserv.conf > /etc/ocserv/ocserv.conf
echo crex > /srv/www/index.html

# Into the gateway's namespace with nsenter, which (unlike ip netns exec)
# doesn't remount /sys: SELinux on Fedora refuses that to a test script.
ip netns add gw
ip link add lan type veth peer name lan netns gw
ip addr add 198.51.100.20/24 dev lan
ip link set lan up
nsenter --net=/run/netns/gw ip link set lo up
nsenter --net=/run/netns/gw ip addr add 198.51.100.10/24 dev lan
nsenter --net=/run/netns/gw ip link set lan up
nsenter --net=/run/netns/gw ip link add acme type dummy
nsenter --net=/run/netns/gw ip link set acme up
# 10.40.1.12 and .13: ones only the OpenVPN and IPsec phases try (craked
# probes a host again only after 10 minutes).
for a in 10.40.0.2 10.40.1.10 10.40.1.11 10.40.1.12 10.40.1.13; do nsenter --net=/run/netns/gw ip addr add "$a/32" dev acme; done
nsenter --net=/run/netns/gw nft -f - <<'NFT'
table ip lan {
  chain input {
    type filter hook input priority 0;
    iifname "lan" ip daddr 10.40.0.0/16 drop
  }
}
NFT
# The LAN's router: it doesn't know ACME's network either.
ip route add 10.40.0.0/16 via 198.51.100.10
nsenter --net=/run/netns/gw dnsmasq --no-resolv --no-hosts --listen-address=10.40.0.2 --bind-interfaces \
  --address=/git.acme.internal/10.40.1.10 --address=/wiki.acme.internal/10.40.1.11
nsenter --net=/run/netns/gw busybox httpd -p 80 -h /srv/www
nsenter --net=/run/netns/gw ocserv --config /etc/ocserv/ocserv.conf
# OpenVPN's: a CA and the server's certificate (clients sign in instead),
# its CA where the test's profile takes it from.
mkdir -p /etc/openvpn/e2e
cd /etc/openvpn/e2e
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout ca.key -out ca.crt -days 30 -subj /CN=ACME-CA 2>/dev/null
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout server.key -out server.csr -subj /CN=vpn.acme.example 2>/dev/null
printf 'extendedKeyUsage=serverAuth\nkeyUsage=digitalSignature,keyAgreement\n' > server.ext
openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 30 -extfile server.ext 2>/dev/null
cp ca.crt /tmp/openvpn-ca.crt
# In the background, not --daemon (which never returned in the AppArmor VM).
nsenter --net=/run/netns/gw openvpn --config /e2e/openvpn.conf > /tmp/openvpn-gw.log 2>&1 &
nsenter --net=/run/netns/gw python3 /e2e/sso.py /run/openvpn-mgmt.sock 198.51.100.10 8080 /tmp/openvpn-seen >/dev/null 2>&1 &
# IPsec's (IKEv2): strongSwan as the gateway (scripts/e2e/ipsec.conf), its
# tunnel ending on ipsec0 (if_id 7), where the pool's replies go back
# through. Its charon runs in this container's own /run, as a system
# strongSwan would: craked's (crake-charon.service) has to start beside it.
# A copy of charon, at a path no AppArmor profile names: on a computer
# with strongSwan installed, the kernel attaches Ubuntu's charon profile
# to /usr/lib/ipsec/charon in containers too, and it refuses this
# gateway's settings (seen 2026-10-19). Still a system strongSwan to
# Crake's: it holds this container's /run/charon.pid.
for c in /usr/lib/ipsec/charon /usr/libexec/strongswan/charon /usr/lib/strongswan/charon; do
    [ -x "$c" ] && cp "$c" /usr/local/sbin/gw-charon && break
done
charon=/usr/local/sbin/gw-charon
# swanctl too (Ubuntu's profile for it allows /run/charon.vici only).
cp "$(command -v swanctl)" /usr/local/sbin/gw-swanctl
nsenter --net=/run/netns/gw ip link add ipsec0 type xfrm if_id 7
nsenter --net=/run/netns/gw ip link set ipsec0 up
nsenter --net=/run/netns/gw ip route add 10.40.9.0/24 dev ipsec0
# Its vici socket named: each distribution builds in another place. Its
# plugins named too: all of them, as without a list, would include
# kernel-libipsec, which takes the gateway's traffic (ocserv went quiet).
printf 'charon {\n  install_routes = no\n  install_virtual_ip = no\n  load = random nonce openssl aes sha1 sha2 hmac gmp kdf curve25519 pem pkcs1 pkcs8 x509 revocation constraints pubkey drbg kernel-netlink socket-default vici eap-identity eap-mschapv2\n  plugins {\n    vici {\n      socket = unix:///tmp/ipsec-gw.vici\n    }\n  }\n  filelog {\n    gw {\n      path = /tmp/ipsec-gw.log\n      default = 1\n    }\n  }\n}\n' > /tmp/ipsec-gw.conf
STRONGSWAN_CONF=/tmp/ipsec-gw.conf nsenter --net=/run/netns/gw "$charon" >/dev/null 2>&1 &
for _ in $(seq 1 50); do [ -S /tmp/ipsec-gw.vici ] && break; sleep 0.1; done
gw-swanctl --load-all --uri unix:///tmp/ipsec-gw.vici --file /e2e/ipsec.conf > /tmp/ipsec-gw-load.log 2>&1
