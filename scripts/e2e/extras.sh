#!/bin/sh
# ACME's gateway twice more, for start.sh and ci-start.sh ($1: the address
# the front listens on): with groups to choose from, and behind a front
# that admits only clients calling themselves AnyConnect. Run before the
# main ocserv starts (the front reaches it at each connection).
set -e
# ACME's gateway with groups on 8445, its own ocserv: signing in, a user
# chooses one (erin is in Engineering and Sales). Its own pool, beside the
# first one's.
sed -e 's/^tcp-port = 443$/tcp-port = 8445/' -e 's/^udp-port = 443$/udp-port = 8445/' \
  -e 's|^socket-file = .*|socket-file = /run/ocserv-groups.socket|' -e 's/^device = vpns$/device = vpng/' \
  -e 's|^ipv4-network = .*|ipv4-network = 10.40.253.0/24|' \
  -e 's|^auth = .*|auth = "plain[passwd=/etc/ocserv/passwd-groups]"|' \
  -e '/^enable-auth/d' -e '/^ca-cert/d' -e '/^cert-user-oid/d' /etc/ocserv/ocserv.conf > /run/ocserv-groups.conf
printf 'select-group = Engineering\nselect-group = Sales\npid-file = /run/ocserv-groups.pid\nuse-occtl = false\n' \
  >> /run/ocserv-groups.conf
ocserv --log-stderr --config /run/ocserv-groups.conf > /link/ocserv-groups.log 2>&1
# ACME's gateway again on 8444, behind a front that admits only clients
# calling themselves AnyConnect (7d).
python3 /picky.py "$1" 8444 /etc/ocserv/cert.pem /etc/ocserv/key.pem 443 \
  > /link/picky.log 2>&1 &
