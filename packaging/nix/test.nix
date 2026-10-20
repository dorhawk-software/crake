# The NixOS module, end to end: a gateway (ocserv, openvpn and strongSwan,
# ACME's network behind them: DNS 10.40.0.2, web servers 10.40.1.10 to .13)
# and a client with services.crake.enable, on the test's own network. The
# client signs in with openconnect, connects through craked over D-Bus, and
# its traffic and DNS go where Crake says; then an OpenVPN tunnel and an
# IPsec one (crake-charon.service) through the same craked. As
# scripts/deb-test/check.sh does for the packages.
self:
{ pkgs, lib, ... }:
let
  # A self-signed certificate and alice's password (hunter2), made once.
  secrets = pkgs.runCommand "crake-test-gateway" { nativeBuildInputs = [ pkgs.gnutls pkgs.openssl ]; } ''
    mkdir -p $out
    certtool --generate-privkey --outfile $out/key.pem --key-type=ecdsa
    printf 'cn = vpn.acme.example\nexpiration_days = 3650\ntls_www_server\nsigning_key\nencryption_key\n' > cert.cfg
    certtool --generate-self-signed --load-privkey $out/key.pem --template cert.cfg --outfile $out/cert.pem
    # ocserv's passwd file: user:group:crypt hash (ocpasswd's crypt fails
    # in the build sandbox).
    echo "alice:*:$(openssl passwd -6 -salt crakecrake hunter2)" > $out/passwd
    # OpenVPN's: a CA and the server's certificate (clients sign in instead).
    openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout ca.key -out $out/ca.crt -days 3650 -subj /CN=ACME-CA
    openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -keyout $out/server.key -out server.csr -subj /CN=vpn.acme.example
    printf 'extendedKeyUsage=serverAuth\nkeyUsage=digitalSignature,keyAgreement\n' > server.ext
    openssl x509 -req -in server.csr -CA $out/ca.crt -CAkey ca.key -CAcreateserial -out $out/server.crt -days 3650 -extfile server.ext
  '';
  # The e2e gateway's OpenVPN and IPsec settings (scripts/e2e/), listening
  # on every address of this VM rather than the e2e's, with this VM's files.
  openvpnConf = pkgs.runCommand "acme-openvpn.conf" { } ''
    sed -e '/^local /d' -e 's|/etc/openvpn/e2e/|${secrets}/|' ${../../scripts/e2e/openvpn.conf} > $out
  '';
  ipsecConf = pkgs.runCommand "acme-ipsec.conf" { } ''
    sed -e '/local_addrs/d' ${../../scripts/e2e/ipsec.conf} > $out
  '';
  # The gateway's charon: its vici socket named, its plugins listed (all of
  # them would take kernel-libipsec, as scripts/deb-test/gateway.sh says).
  gatewayCharon = pkgs.writeText "acme-strongswan.conf" ''
    charon {
      install_routes = no
      install_virtual_ip = no
      load = random nonce openssl pem pkcs1 pkcs8 x509 revocation constraints pubkey drbg kdf gmp kernel-netlink socket-default vici eap-identity eap-mschapv2
      plugins {
        vici {
          socket = unix:///run/acme-ipsec.vici
        }
      }
    }
  '';
  www = pkgs.writeTextDir "index.html" "crex";
in {
  name = "crake";

  nodes.gateway = { pkgs, ... }: {
    virtualisation.memorySize = 2048;
    networking.firewall.enable = false;
    # ocserv's tunnel devices are ocserv's: dhcpcd took vpns0 as a client
    # connected ("waiting for 3rd party to configure IP address"), and the
    # journey's first connection through it was reset (CLAUDE.md, Not now).
    networking.dhcpcd.denyInterfaces = [ "vpns*" ];
    networking.nftables.enable = true;
    # From the LAN, ACME's network doesn't answer: only the tunnel reaches it.
    networking.nftables.tables.lan = {
      family = "ip";
      content = ''
        chain input {
          type filter hook input priority 0;
          iifname "eth1" ip daddr 10.40.0.0/16 drop
        }
      '';
    };
    systemd.services.acme-net = {
      description = "ACME's internal network";
      wantedBy = [ "multi-user.target" ];
      before = [ "dnsmasq.service" "ocserv.service" ];
      path = [ pkgs.iproute2 ];
      serviceConfig.Type = "oneshot";
      serviceConfig.RemainAfterExit = true;
      script = ''
        ip link add acme type dummy
        ip link set acme up
        for a in 10.40.0.2 10.40.1.10 10.40.1.11 10.40.1.12 10.40.1.13; do ip addr add "$a/32" dev acme; done
        # IPsec's tunnel ends here (if_id 7, as in ipsec.conf); its pool's
        # replies go back through it.
        ip link add ipsec0 type xfrm if_id 7
        ip link set ipsec0 up
        ip route add 10.40.9.0/24 dev ipsec0
      '';
    };
    services.dnsmasq = {
      enable = true;
      resolveLocalQueries = false;
      settings = {
        no-resolv = true;
        no-hosts = true;
        listen-address = "10.40.0.2";
        bind-interfaces = true;
        address = [ "/git.acme.internal/10.40.1.10" "/wiki.acme.internal/10.40.1.11" ];
      };
    };
    systemd.services.acme-www = {
      wantedBy = [ "multi-user.target" ];
      after = [ "acme-net.service" ];
      serviceConfig.ExecStart = "${pkgs.busybox}/bin/busybox httpd -f -p 80 -h ${www}";
    };
    # ACME's OpenVPN gateway, its sign-in answered by sso.py over the
    # management interface (alice, hunter2).
    systemd.services.acme-openvpn = {
      wantedBy = [ "multi-user.target" ];
      after = [ "acme-net.service" ];
      serviceConfig.ExecStart = "${pkgs.openvpn}/bin/openvpn --config ${openvpnConf}";
    };
    systemd.services.acme-openvpn-sign-in = {
      wantedBy = [ "multi-user.target" ];
      after = [ "acme-openvpn.service" ];
      serviceConfig.ExecStart = "${pkgs.python3}/bin/python3 ${../../scripts/e2e/sso.py} /run/openvpn-mgmt.sock 127.0.0.1 8080 /run/openvpn-seen";
    };
    # ACME's IPsec (IKEv2) gateway: strongSwan, the key, alice's password
    # over EAP-MSCHAPv2.
    systemd.services.acme-ipsec = {
      wantedBy = [ "multi-user.target" ];
      after = [ "acme-net.service" ];
      environment.STRONGSWAN_CONF = "${gatewayCharon}";
      serviceConfig.ExecStart = "${pkgs.strongswan}/libexec/ipsec/charon";
      postStart = ''
        for _ in $(seq 1 50); do [ -S /run/acme-ipsec.vici ] && break; sleep 0.1; done
        ${pkgs.strongswan}/bin/swanctl --load-all --uri unix:///run/acme-ipsec.vici --file ${ipsecConf}
      '';
    };
    services.ocserv = {
      enable = true;
      config = ''
        auth = "plain[passwd=${secrets}/passwd]"
        tcp-port = 443
        udp-port = 443
        run-as-user = nobody
        run-as-group = nogroup
        socket-file = /run/ocserv-socket
        server-cert = ${secrets}/cert.pem
        server-key = ${secrets}/key.pem
        isolate-workers = false
        max-clients = 4
        max-same-clients = 4
        keepalive = 30
        dpd = 10
        try-mtu-discovery = false
        cisco-client-compat = true
        device = vpns
        predictable-ips = true
        ipv4-network = 10.40.255.0/24
        dns = 10.40.0.2
        default-domain = acme.internal
        route = 10.40.0.0/16
        ping-leases = false
      '';
    };
  };

  nodes.client = { nodes, pkgs, ... }: {
    imports = [ self.nixosModules.default ];
    services.crake.enable = true;
    services.crake.ipsec = true;
    # The client runs craked, openconnect and the whole GPUI journey
    # (software-rendered): the test VM's 1 vCPU / 1 GB default is cramped.
    virtualisation.cores = 2;
    virtualisation.memorySize = 4096;
    networking.firewall.enable = false;
    # The journey runs as root with the system's own tools.
    environment.systemPackages = [ pkgs.curl pkgs.nftables pkgs.dbus pkgs.util-linux ];
    # An upgrade: the same system with craked's unit changed, switched to
    # while ACME is up.
    specialisation.upgraded.configuration = {
      systemd.services.craked.environment.CRAKE_TEST_GENERATION = "2";
    };
    # The LAN's router doesn't know ACME's network either.
    networking.interfaces.eth1.ipv4.routes = [{
      address = "10.40.0.0"; prefixLength = 16;
      via = nodes.gateway.networking.primaryIPAddress;
    }];
  };

  testScript = { nodes, ... }:
    let gw = nodes.gateway.networking.primaryIPAddress; in ''
    import re
    start_all()
    gateway.wait_for_unit("ocserv.service")
    gateway.wait_for_unit("dnsmasq.service")
    gateway.wait_for_unit("acme-www.service")
    client.wait_for_unit("craked.service")

    call = "busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 "
    with subtest("nothing reaches ACME's network without the tunnel"):
        client.fail("curl -s -m 2 http://10.40.1.10/")

    with subtest("polkit refuses someone without a session here"):
        # (dbus-send fails on the refusal; the driver's shell has pipefail)
        client.succeed("(setpriv --reuid=nobody --regid=$(id -g nobody) --clear-groups "
                       + "dbus-send --system --print-reply --dest=software.dorhawk.Crake1 /software/dorhawk/Crake1 "
                       + "software.dorhawk.Crake1.RemoveTunnel string:globex 2>&1 || true) | grep 'not authorized to switch tunnels'")

    with subtest("signs in and connects through craked"):
        client.succeed(call + "AddTunnel 'a{ss}' 4 display_name ACME gateway https://${gw} protocol anyconnect user alice")
        out = client.succeed("printf 'yes\\nhunter2\\n' | openconnect --authenticate --user=alice https://${gw} 2>/dev/null")
        s = dict(re.findall(r"^(\w+)='(.*)'$", out, re.M))
        client.succeed(call + "ConnectWithSession 'sa{ss}' acme 4 "
                       + f"cookie '{s['COOKIE']}' connect_url '{s['CONNECT_URL']}' "
                       + f"fingerprint '{s['FINGERPRINT']}' resolve '{s['RESOLVE']}'")
        client.wait_until_succeeds(call + "GetTunnel s acme | grep -q '\"connected\"'", timeout=30)
        client.succeed("ip -4 -o addr show crake-acme | grep -q 'inet 10.40.255.'")
        client.fail("ip -4 route show table all | grep -q '10.40.0.0/16 dev crake-acme'")

    with subtest("a host that doesn't answer directly gets through"):
        client.succeed("curl -s -m 15 http://10.40.1.10/ | grep -q crex")

    with subtest("a name only ACME's DNS knows is routed before the first packet"):
        client.succeed("resolvectl query wiki.acme.internal | grep -q 10.40.1.11")
        client.succeed("curl -s -m 1 http://10.40.1.11/ | grep -q crex")

    with subtest("an upgrade keeps ACME signed in"):
        client.succeed("/run/current-system/specialisation/upgraded/bin/switch-to-configuration test")
        client.wait_until_succeeds("grep -q 'Crake was updated. Your tunnels are reconnecting with your sign-ins.' /var/lib/crake/log", timeout=30)
        client.wait_until_succeeds(call + "GetTunnel s acme | grep -q '\"connected\"'", timeout=30)
        client.succeed("curl -s -m 3 http://10.40.1.11/ | grep -q crex")
        client.fail("test -e /run/crake/resume.json")

    with subtest("switched off, ACME's names are refused at once"):
        client.succeed(call + "Disconnect s acme")
        client.wait_until_succeeds("ip link show crake-hush")
        # (resolvectl fails on a refusal; the driver's shell has pipefail)
        client.wait_until_succeeds("(resolvectl query wiki.acme.internal 2>&1 || true) | grep -q REFUSED", timeout=10)

    def answer(tunnel, kind, text):
        """craked's question about a tunnel's sign-in, answered as the
        app's sign-in window does (D-Bus Asking and Answer)."""
        out = client.wait_until_succeeds(call + f"Asking s {tunnel} | grep '\"kind\" \"{kind}\"'", timeout=30)
        asked = re.search(r'"id" "(\d+)"', out).group(1)
        client.succeed(call + f"Answer sus {tunnel} {asked} {text}")

    with subtest("an OpenVPN tunnel, its password asked over D-Bus"):
        gateway.wait_for_unit("acme-openvpn-sign-in.service")
        client.succeed("printf 'client\\ndev tun\\nproto udp\\nremote ${gw} 1194\\nnobind\\nauth-user-pass\\n"
                       + "remote-cert-tls server\\n<ca>\\n%s\\n</ca>\\n' \"$(cat ${secrets}/ca.crt)\" > /tmp/initech.ovpn")
        client.succeed(call + "AddTunnel 'a{ss}' 3 display_name Initech protocol openvpn profile \"$(cat /tmp/initech.ovpn)\"")
        client.succeed("test \"$(stat -c %a /var/lib/crake/profiles/initech.ovpn)\" = 600")
        client.succeed(call + "Connect s initech")
        answer("initech", "user", "alice")
        answer("initech", "password", "hunter2")
        client.wait_until_succeeds(call + "GetTunnel s initech | grep -q '\"connected\"'", timeout=30)
        client.succeed("ip -4 -o addr show crake-initech | grep -q 'inet 10.40.254.'")
        client.succeed("curl -s -m 15 http://10.40.1.12/ | grep -q crex")
        client.succeed(call + "Disconnect s initech")
        client.wait_until_fails("pgrep -f '[o]penvpn-initech'", timeout=10)
        client.succeed(call + "RemoveTunnel s initech")
        client.fail("test -e /var/lib/crake/profiles/initech.ovpn")

    with subtest("an IPsec tunnel, its charon Crake's own unit"):
        # Where the app looks for strongSwan here (New tunnel's IPsec card).
        client.succeed("test -x /run/current-system/sw/libexec/ipsec/charon")
        client.succeed("test -f /run/current-system/sw/lib/ipsec/plugins/libstrongswan-vici.so")
        gateway.wait_for_unit("acme-ipsec.service")
        client.succeed(call + "AddTunnel 'a{ss}' 8 display_name Nordbank protocol ipsec gateway ${gw} "
                       + "gateway_key hunter2hunter2 gateway_id vpn.acme.example user alice domains acme.internal sign_in password")
        client.succeed("test \"$(stat -c %a /var/lib/crake/profiles/nordbank.ipsec)\" = 600")
        client.succeed(call + "Connect s nordbank")
        answer("nordbank", "password", "hunter2")
        client.wait_until_succeeds(call + "GetTunnel s nordbank | grep -q '\"connected\"'", timeout=30)
        client.succeed("systemctl is-active crake-charon")
        client.succeed("ip -4 -o addr show crake-nordbank | grep -q 'inet 10.40.9.'")
        client.succeed("curl -s -m 15 http://10.40.1.13/ | grep -q crex")
        client.succeed(call + "Disconnect s nordbank")
        client.wait_until_fails("ip link show crake-nordbank", timeout=10)
        client.succeed(call + "RemoveTunnel s nordbank")
        client.fail("test -e /var/lib/crake/profiles/nordbank.ipsec")

    with subtest("nothing went wrong in craked's journal"):
        client.fail("journalctl -u craked -o cat | grep -E 'ERROR|panicked'")

    with subtest("the journey (the app and its own craked)"):
        client.succeed("systemctl stop craked")
        client.succeed("mkdir -p /tmp/journey && env CRAKE_E2E=1 TMPDIR=/tmp/journey "
                       + "CRAKE_E2E_HOOK=${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/libexec/crake/hook "
                       + "CRAKE_E2E_GATEWAY=https://${gw} "
                       + "${self.packages.${pkgs.stdenv.hostPlatform.system}.journey}/bin/crake-journey journey_tests --test-threads=1 >&2")
  '';
}
