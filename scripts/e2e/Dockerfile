# A stand-in for a company's AnyConnect gateway, for scripts/e2e-test.sh:
# ocserv, and behind it a DNS server and a web server that only the tunnel
# reaches (10.40.0.0/16, ACME's internal network in the tests).
FROM ubuntu:26.04
RUN apt-get update \
 && apt-get install -y --no-install-recommends ocserv gnutls-bin dnsmasq-base busybox iproute2 nftables python3-minimal \
 && rm -rf /var/lib/apt/lists/*
RUN mkdir -p /etc/ocserv /srv/www \
 && certtool --generate-privkey --outfile /etc/ocserv/key.pem --key-type=ecdsa \
 && printf 'cn = vpn.acme.example\nexpiration_days = 3650\ntls_www_server\nsigning_key\nencryption_key\n' > /tmp/cert.cfg \
 && certtool --generate-self-signed --load-privkey /etc/ocserv/key.pem --template /tmp/cert.cfg --outfile /etc/ocserv/cert.pem \
 && printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd alice \
 && echo crex > /srv/www/index.html
COPY ocserv.conf /etc/ocserv/ocserv.conf
COPY start.sh client.sh relay.py /
CMD ["/bin/sh", "/start.sh"]
