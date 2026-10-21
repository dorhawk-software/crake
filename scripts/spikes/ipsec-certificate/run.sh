#!/bin/sh
# Spike (ipsec-certificates.html, 5): an untrusted gateway's certificate read
# from the client charon's "enc 3" dump, then pinned. Runs inside the e2e
# gateway image with no network: two netns joined by a veth pair.
set -eu
S=/spike  # this folder, mounted (see README)
FRAG=${FRAG:-1400}
cd $S
rm -rf work && mkdir -p work && cd work
# IT's authority and the gateway's certificate (RSA 4096: a long one).
openssl req -x509 -newkey rsa:4096 -nodes -keyout ca.key -out ca.crt -days 30 -subj "/CN=ACME Root CA" 2>/dev/null
openssl req -newkey rsa:4096 -nodes -keyout gw.key -out gw.csr -subj "/CN=vpn.acme.example" 2>/dev/null
printf 'subjectAltName=DNS:vpn.acme.example\nextendedKeyUsage=serverAuth\n' > ext
openssl x509 -req -in gw.csr -CA ca.crt -CAkey ca.key -CAcreateserial -out gw.crt -days 30 -extfile ext 2>/dev/null
openssl x509 -in gw.crt -outform der -out gw.der
# Another authority the client knows (the system's bundle stands in): it
# makes the client ask for a certificate (CERTREQ); a gateway that sends
# its certificate only when asked sends none otherwise.
openssl req -x509 -newkey rsa:2048 -nodes -keyout other.key -out other.crt -days 30 -subj "/CN=Some Public Root" 2>/dev/null
for side in gw cl; do mkdir -p $side/swanctl/x509 $side/swanctl/x509ca $side/swanctl/private $side/swanctl/conf.d $side/run; done
cp gw.crt gw/swanctl/x509/; cp gw.key gw/swanctl/private/
[ "${NO_OTHER:-}" ] || cp other.crt cl/swanctl/x509ca/
cat > gw/swanctl/swanctl.conf <<C
connections {
  acme {
    version = 2
    local_addrs = 192.0.2.1
    pools = p
    fragmentation = yes
    local {
      auth = pubkey
      certs = gw.crt
      id = vpn.acme.example
    }
    remote {
      auth = eap-mschapv2
      eap_id = %any
    }
    children {
      acme {
        local_ts = 10.40.0.0/16
      }
    }
  }
}
pools {
  p {
    addrs = 10.40.9.10-10.40.9.20
  }
}
secrets {
  eap-alice {
    id = alice
    secret = hunter2
  }
}
C
cat > cl.conf <<C
connections {
  acme {
    version = 2
    remote_addrs = 192.0.2.1
    vips = 0.0.0.0
    fragmentation = yes
    local {
      auth = eap-mschapv2
      eap_id = alice
    }
    remote {
      auth = pubkey
      id = vpn.acme.example
    }
    children {
      acme {
        remote_ts = 10.40.0.0/16
      }
    }
  }
}
secrets {
  eap-alice {
    id = alice
    secret = hunter2
  }
}
C
cp cl.conf cl/swanctl/swanctl.conf
for side in gw cl; do
  enc=1; [ $side = cl ] && enc=3
  cat > $side.sc <<C
charon {
  load_modular = yes
  plugins {
    include /etc/strongswan.d/charon/*.conf
  }
  install_routes = no
  install_virtual_ip = no
  fragment_size = $FRAG
  filelog {
    f {
      path = $S/work/$side.log
      default = 1
      ike = 2
      enc = $enc
    }
  }
}
C
done
ip netns add gw; ip netns add cl
ip link add vgw type veth peer name vcl
ip link set vgw netns gw; ip link set vcl netns cl
ip -n gw addr add 192.0.2.1/24 dev vgw; ip -n gw link set vgw up; ip -n gw link set lo up
ip -n cl addr add 192.0.2.2/24 dev vcl; ip -n cl link set vcl up; ip -n cl link set lo up
start() { # side
  ip netns exec $1 unshare -m sh -c "echo \$\$ > $S/work/$1.pid; mount -t tmpfs tmpfs /run && mount --bind $S/work/$1/swanctl /etc/swanctl && export STRONGSWAN_CONF=$S/work/$1.sc; exec /usr/local/bin/charon-spike" > $S/work/$1.out 2>&1 &
}
swan() { # side args...
  side=$1; shift
  # The charon's own mount namespace (its /run and /etc/swanctl).
  nsenter -t "$(cat $S/work/$side.pid)" -m -n swanctl "$@"
}
cp /usr/lib/ipsec/charon /usr/local/bin/charon-spike
start gw; start cl
sleep 2
swan gw --load-all >/dev/null
swan cl --load-all >/dev/null
echo "== 1: no authority, nothing pinned"
swan cl --initiate --child acme --timeout 15 > init1.log 2>&1 && echo "connected (unexpected)" || echo "refused, as expected"
grep -E "received end entity cert|no trusted|constraint" cl.log | head -5
python3 -I $S/extract.py cl.log gw.der
echo "== 2: the extracted certificate pinned (x509/, no authority)"
cp got.der cl/swanctl/x509/pinned.der
swan cl --load-creds >/dev/null
swan cl --initiate --child acme --timeout 15 > init2.log 2>&1 && echo "connected with the pin" || { echo "refused with the pin"; tail -5 init2.log; }
grep -c "plain" cl.log >/dev/null
echo "== secrets in the dump?"
python3 -I $S/secrets.py cl.log
swan cl --terminate --ike acme >/dev/null 2>&1 || true
echo "== 3: another key under the same name pinned instead"
openssl req -x509 -newkey rsa:2048 -nodes -keyout fake.key -out fake.crt -days 30 -subj "/CN=vpn.acme.example" -addext "subjectAltName=DNS:vpn.acme.example" 2>/dev/null
rm cl/swanctl/x509/pinned.der; cp fake.crt cl/swanctl/x509/pinned.crt
swan cl --load-creds --clear >/dev/null 2>&1 || swan cl --load-creds >/dev/null
swan cl --initiate --child acme --timeout 15 > init3.log 2>&1 && echo "connected (WRONG: the pin let another key in)" || echo "refused, as it should"
echo "== 4: the right pin, but the gateway expected under another name"
rm cl/swanctl/x509/pinned.crt; cp got.der cl/swanctl/x509/pinned.der
sed 's/id = vpn.acme.example/id = vpn.globex.example/' cl.conf > cl/swanctl/swanctl.conf
swan cl --load-all --clear >/dev/null 2>&1 || swan cl --load-all >/dev/null
: > cl.log.mark; n0=$(wc -l < cl.log)
swan cl --initiate --child acme --timeout 15 > init4.log 2>&1 && echo "connected (WRONG)" || echo "refused, as it should"
tail -n +$n0 cl.log > cl4.log
grep -E "no trusted|constraint|received end entity" cl4.log | head -3
python3 -I $S/extract.py cl4.log gw.der | tail -1
echo "== 5: the dump off again at run time (reload-settings)"
cp cl.conf cl/swanctl/swanctl.conf; swan cl --load-all >/dev/null 2>&1
sed -i 's/enc = 3/enc = 1/' cl.sc
swan cl --reload-settings >/dev/null
n1=$(wc -l < cl.log)
swan cl --initiate --child acme --timeout 15 > init5.log 2>&1 && echo "connected" || echo "refused"
echo "plain dumps after the reload: $(tail -n +$n1 cl.log | grep -c 'plain =>')"
swan cl --terminate --ike acme >/dev/null 2>&1 || true
echo "== 4b: fetching the certificate without naming the gateway (remote.id = %any), nothing pinned"
rm -f cl/swanctl/x509/pinned.der
sed -i 's/enc = 1/enc = 3/' cl.sc; swan cl --reload-settings >/dev/null
sed 's/id = vpn.acme.example/id = %any/' cl.conf > cl/swanctl/swanctl.conf
swan cl --load-all --clear >/dev/null 2>&1 || swan cl --load-all >/dev/null
n2=$(wc -l < cl.log)
swan cl --initiate --child acme --timeout 15 > init4b.log 2>&1 && echo "connected (WRONG)" || echo "refused, as it should (untrusted)"
tail -n +$n2 cl.log > cl4b.log
grep -E "received end entity|no trusted" cl4b.log | head -2
python3 -I $S/extract.py cl4b.log gw.der | tail -1
openssl x509 -in got.der -inform der -noout -subject -ext subjectAltName 2>/dev/null
