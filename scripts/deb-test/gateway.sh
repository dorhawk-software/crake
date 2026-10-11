#!/bin/sh
# Inside scripts/deb-test.sh's container, cut off from every network: ACME's
# gateway (ocserv, scripts/e2e/ocserv.conf) in a network namespace of its
# own, wired to this one as the LAN (198.51.100.0/24), with ACME's internal
# network behind it as in scripts/e2e/start.sh.
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
for a in 10.40.0.2 10.40.1.10 10.40.1.11; do nsenter --net=/run/netns/gw ip addr add "$a/32" dev acme; done
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
