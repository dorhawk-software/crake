# A stand-in for a company's gateway, for scripts/e2e-test.sh: ocserv
# (AnyConnect), openvpn (with a browser sign-in, sso.py) and strongSwan
# (IPsec, IKEv2, as a FortiGate signs users in), and behind them a DNS server and a web server that only the tunnel
# reaches (10.40.0.0/16, ACME's internal network in the tests).
FROM ubuntu:26.04
RUN apt-get update \
 && apt-get install -y --no-install-recommends ocserv gnutls-bin dnsmasq-base busybox iproute2 nftables python3 python3-flask python3-cryptography openvpn openssl curl ca-certificates \
    strongswan-charon strongswan-swanctl libcharon-extauth-plugins libcharon-extra-plugins \
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
 && printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd bob \
 && printf 'hunter2\nhunter2\n' | ocpasswd -c /etc/ocserv/passwd-groups -g Engineering,Sales erin \
 && echo crex > /srv/www/index.html
# ACME's users' CA and carol's certificate (a PKCS#12 file whose password
# is hunter2), for signing in with a certificate.
RUN cd /etc/ocserv \
 && openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout users-ca.key -out users-ca.pem -days 3650 -subj "/CN=ACME users" \
 && openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout carol.key -out carol.csr -subj /CN=carol \
 && printf 'extendedKeyUsage=clientAuth\nkeyUsage=digitalSignature\n' > carol.ext \
 && openssl x509 -req -in carol.csr -CA users-ca.pem -CAkey users-ca.key -CAcreateserial -out carol.pem -days 3650 -extfile carol.ext \
 && openssl pkcs12 -export -inkey carol.key -in carol.pem -name carol -passout pass:hunter2 -out carol.p12 \
 && rm carol.key carol.csr
# OpenVPN's: a CA and the server's certificate (clients sign in instead).
RUN mkdir -p /etc/openvpn/e2e && cd /etc/openvpn/e2e \
 && openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout ca.key -out ca.crt -days 3650 -subj /CN=ACME-CA \
 && openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout server.key -out server.csr -subj /CN=vpn.acme.example \
 && printf 'extendedKeyUsage=serverAuth\nkeyUsage=digitalSignature,keyAgreement\n' > server.ext \
 && openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out server.crt -days 3650 -extfile server.ext
COPY ocserv.conf ocserv-companies.conf /etc/ocserv/
COPY ocserv-users /etc/ocserv/users
RUN sed -i '/^default-domain/d' /etc/ocserv/ocserv.conf \
 && cat /etc/ocserv/ocserv-companies.conf >> /etc/ocserv/ocserv.conf
COPY openvpn.conf /etc/openvpn/e2e/server.conf
COPY ipsec.conf /etc/swanctl/conf.d/acme.conf
COPY ipsec-cert.conf /etc/swanctl/conf.d/acme-cert.conf
# The IPsec gateway that proves itself with a certificate (acme-cert,
# ipsec-cert.conf), signed by an authority of ACME's own that the client's
# system doesn't know: IT hands its certificate out (start.sh, /link).
# alice's own certificate, from the same authority, for the connections
# that take one (acme-certs, acme-certpw): a .p12 with her password, as IT
# hands one out (start.sh puts it in /link).
RUN cd /etc/swanctl \
 && openssl req -x509 -newkey rsa:3072 -nodes -keyout /tmp/ca.key -out x509ca/acme-ipsec-ca.pem -days 3650 -subj "/CN=ACME IPsec Root" \
 && openssl req -newkey rsa:3072 -nodes -keyout private/vpn-cert.key -out /tmp/gw.csr -subj /CN=vpn-cert.acme.example \
 && printf 'subjectAltName=DNS:vpn-cert.acme.example,DNS:vpn-certs.acme.example,DNS:vpn-certpw.acme.example\nextendedKeyUsage=serverAuth\n' > /tmp/gw.ext \
 && openssl x509 -req -in /tmp/gw.csr -CA x509ca/acme-ipsec-ca.pem -CAkey /tmp/ca.key -CAserial /tmp/ca.srl -CAcreateserial \
      -out x509/vpn-cert.pem -days 3650 -extfile /tmp/gw.ext \
 && openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout /tmp/alice.key -out /tmp/alice.csr -subj /CN=alice \
 && printf 'extendedKeyUsage=clientAuth\n' > /tmp/alice.ext \
 && openssl x509 -req -in /tmp/alice.csr -CA x509ca/acme-ipsec-ca.pem -CAkey /tmp/ca.key -CAserial /tmp/ca.srl \
      -out /tmp/alice.crt -days 3650 -extfile /tmp/alice.ext \
 && mkdir -p /etc/ipsec-e2e \
 && openssl pkcs12 -export -in /tmp/alice.crt -inkey /tmp/alice.key -out /etc/ipsec-e2e/alice.p12 -passout pass:hunter2 \
 && rm /tmp/ca.key /tmp/ca.srl /tmp/gw.csr /tmp/gw.ext /tmp/alice.*
COPY strongswan.conf /etc/strongswan.d/crake-e2e.conf
COPY start.sh ci-start.sh companies.sh extras.sh client.sh relay.py sso.py picky.py cisco_sso.py /
CMD ["/bin/sh", "/start.sh"]
