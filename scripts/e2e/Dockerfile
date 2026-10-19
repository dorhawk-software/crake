# A stand-in for a company's gateway, for scripts/e2e-test.sh: ocserv
# (AnyConnect) and openvpn (with a browser sign-in, sso.py), and behind them a DNS server and a web server that only the tunnel
# reaches (10.40.0.0/16, ACME's internal network in the tests).
FROM ubuntu:26.04
RUN apt-get update \
 && apt-get install -y --no-install-recommends ocserv gnutls-bin dnsmasq-base busybox iproute2 nftables python3 python3-flask openvpn openssl curl ca-certificates \
 && rm -rf /var/lib/apt/lists/*
# The other protocols' gateways, as openconnect's own tests play them
# (Fortinet, GlobalProtect, F5, Juniper/Pulse): their sign-in and the
# settings they hand out, then a refusal at the tunnel stage. openconnect's
# (LGPL-2.1), taken from its release, pinned by checksum.
ARG OPENCONNECT=9.12
ARG OPENCONNECT_SHA256=a2bedce3aa4dfe75e36e407e48e8e8bc91d46def5335ac9564fbf91bd4b2413e
RUN mkdir -p /fake && cd /tmp \
 && curl -fsSL --retry 4 --retry-all-errors --connect-timeout 20 --max-time 120 -o oc.tar.gz "https://www.infradead.org/openconnect/download/openconnect-$OPENCONNECT.tar.gz" \
 && echo "$OPENCONNECT_SHA256  oc.tar.gz" | sha256sum -c - \
 && tar xzf oc.tar.gz "openconnect-$OPENCONNECT/tests" "openconnect-$OPENCONNECT/COPYING.LGPL" \
 && cp openconnect-$OPENCONNECT/tests/fake-fortinet-server.py openconnect-$OPENCONNECT/tests/fake-gp-server.py \
       openconnect-$OPENCONNECT/tests/fake-f5-server.py openconnect-$OPENCONNECT/tests/fake-juniper-server.py \
       openconnect-$OPENCONNECT/COPYING.LGPL /fake/ \
 && rm -rf /tmp/oc.tar.gz /tmp/openconnect-* \
 # Fortinet's, F5's and Juniper's session cookie is bytes, which today's
 # Werkzeug refuses (a 500, and openconnect asks for the password again and
 # again): text instead. GlobalProtect's makes it text itself.
 && sed -i 's/return base64.urlsafe_b64encode(dumps(jsonable).encode())$/&.decode()/' \
      /fake/fake-fortinet-server.py /fake/fake-f5-server.py /fake/fake-juniper-server.py \
 && grep -c 'encode()).decode()' /fake/fake-fortinet-server.py
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
COPY start.sh ci-start.sh client.sh relay.py sso.py /
CMD ["/bin/sh", "/start.sh"]
