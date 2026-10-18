# A stand-in for a company's gateway, for scripts/e2e-test.sh: ocserv
# (AnyConnect) and openvpn (with a browser sign-in, sso.py), and behind them a DNS server and a web server that only the tunnel
# reaches (10.40.0.0/16, ACME's internal network in the tests).
FROM ubuntu:26.04
RUN apt-get update \
 && apt-get install -y --no-install-recommends ocserv gnutls-bin dnsmasq-base busybox iproute2 nftables python3 openvpn openssl \
 && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /etc/ocserv /srv/www \
 && certtool --generate-privkey --outfile /etc/ocserv/key.pem --key-type=ecdsa \
 && printf 'cn = vpn.acme.example\nexpiration_days = 3650\ntls_www_server\nsigning_key\nencryption_key\n' > /tmp/cert.cfg \
 && certtool --generate-self-signed --load-privkey /etc/ocserv/key.pem --template /tmp/cert.cfg --outfile /etc/ocserv/cert.pem \
 && printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd alice \
 && echo crex > /srv/www/index.html
# OpenVPN's: a CA and the server's certificate (clients sign in instead).
RUN mkdir -p /etc/openvpn/e2e && cd /etc/openvpn/e2e \
 && openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout ca.key -out ca.crt -days 3650 -subj /CN=ACME-CA \
 && openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout server.key -out server.csr -subj /CN=vpn.acme.example \
 && printf 'extendedKeyUsage=serverAuth\nkeyUsage=digitalSignature,keyAgreement\n' > server.ext \
 && openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 3650 -extfile server.ext
COPY ocserv.conf /etc/ocserv/ocserv.conf
COPY openvpn.conf /etc/openvpn/e2e/server.conf
COPY start.sh client.sh relay.py sso.py /
CMD ["/bin/sh", "/start.sh"]
