# The NixOS module, end to end: a gateway (ocserv, ACME's network behind it:
# DNS 10.40.0.2, web servers 10.40.1.10 and .11) and a client with
# services.crake.enable, on the test's own network. The client signs in with
# openconnect, connects through craked over D-Bus, and its traffic and DNS
# go where Crake says. As scripts/deb-test/check.sh does for the packages.
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
  '';
  www = pkgs.writeTextDir "index.html" "crex";
in {
  name = "crake";

  nodes.gateway = { pkgs, ... }: {
    networking.firewall.enable = false;
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
        for a in 10.40.0.2 10.40.1.10 10.40.1.11; do ip addr add "$a/32" dev acme; done
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
        client.succeed("setpriv --reuid=nobody --regid=$(id -g nobody) --clear-groups "
                       + "dbus-send --system --print-reply --dest=software.dorhawk.Crake1 /software/dorhawk/Crake1 "
                       + "software.dorhawk.Crake1.RemoveTunnel string:globex 2>&1 | grep 'not authorized to switch tunnels'")

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
