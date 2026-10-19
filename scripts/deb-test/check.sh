#!/bin/sh
# Inside scripts/deb-test.sh's container (systemd running), one phase per
# call:
#   install  install the package with apt; craked runs as the service it
#            should, on the system bus, guarded by polkit
#   tunnel   (no network any more) a real tunnel through the installed
#            service: ocserv in its own namespace (gateway.sh), probing,
#            DNS through systemd-resolved, keeping names private; then an
#            OpenVPN one (openvpn, a password sign-in answered over D-Bus)
#   journey  the journey suite (the app and its craked) on this system
#   remove   remove and purge; nothing is left
# Prints each check; exits non-zero if any failed.
set -u
fail=0
ok() { echo "  ok    $1"; }
bad() { echo "  FAIL  $1"; fail=1; }
check() { name=$1; shift; if "$@" >/tmp/out 2>&1; then ok "$name"; else bad "$name"; sed 's/^/        /' /tmp/out; fi; }
call() { busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 "$@"; }
as_nobody() { setpriv --reuid=nobody --regid=$(id -g nobody) --clear-groups "$@"; }
export DEBIAN_FRONTEND=noninteractive
# Ubuntu and Debian (apt, the .deb), Fedora (dnf, the RPM) or Arch (pacman).
if command -v apt-get >/dev/null; then PM=apt
elif command -v dnf >/dev/null; then PM=dnf
else PM=pacman; fi

install() {
    if [ "$PM" = pacman ]; then
        check "installs with its depends only" pacman -U --noconfirm /pkg/crake.pkg.tar.zst
        # What its install message says to do: an engine, and the services
        # (Arch packages enable none themselves).
        check "openconnect, as the message says" pacman -S --noconfirm --needed openconnect openvpn
        check "and the services" systemctl enable --now systemd-resolved craked
    elif [ "$PM" = dnf ]; then
        check "installs with its Requires only" dnf install -y -q --setopt=install_weak_deps=False /pkg/crake.rpm
        # The test's own tool for the polkit check (dbus-send), after the
        # package, so its Requires are what's tested.
        dnf install -y -q dbus-tools >/dev/null 2>&1
    else
        apt-get update -qq >/dev/null
        check "installs with its Depends only" apt-get install -y -qq --no-install-recommends /pkg/crake.deb
    fi
    echo "  engine: $(openconnect --version 2>/dev/null | head -n1)"
    check "every library the app needs is there" sh -c '! ldd /usr/bin/crake | grep "not found"'
    if [ "$PM" = pacman ]; then
        check "the hook is where craked looks" sh -c 'test -x /usr/lib/crake/hook && grep -q "^hook_path = \"/usr/lib/crake/hook\"" /etc/crake/craked.toml'
    else
        check "the hook is where craked looks" test -x /usr/libexec/crake/hook
    fi
    check "the polkit action is known" sh -c 'pkaction | grep -qx software.dorhawk.crake.manage-tunnels'
    for _ in $(seq 1 20); do systemctl is-active -q craked && break; sleep 0.5; done
    check "craked is running" systemctl is-active craked
    if aa-enabled --quiet 2>/dev/null; then
        check "craked runs confined by AppArmor" sh -c 'grep -Eq "^craked \((enforce|complain)\)" /proc/$(systemctl show -p MainPID --value craked)/attr/current'
    fi
    check "craked answers on the system bus" call Settings
    check "root may add a tunnel" call AddTunnel 'a{ss}' 3 display_name Globex gateway vpn.globex.example protocol anyconnect
    check "it is listed" sh -c "busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 ListTunnels | grep -q globex"
    check "kept in craked's state directory" test -s /var/lib/crake/tunnels.json
    # Someone without an active session at this computer: craked asked
    # polkit, which said no (craked's own words, not the bus's).
    check "polkit refuses others" sh -c "setpriv --reuid=nobody --regid=$(id -g nobody) --clear-groups dbus-send --system --print-reply --dest=software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1.RemoveTunnel string:globex 2>&1 | grep 'not authorized to switch tunnels'"
    check "anyone may read" as_nobody busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 Settings
    # For the tunnel phase, while the archive can still be reached.
    if [ "$PM" = pacman ]; then
        check "the gateway's software" pacman -S --noconfirm --needed curl
    elif [ "$PM" = dnf ]; then
        check "the gateway's software" dnf install -y -q ocserv gnutls-utils dnsmasq busybox curl openssl python3
        check "openvpn, as Recommends would" dnf install -y -q openvpn
    else
        check "the gateway's software" apt-get install -y -qq --no-install-recommends ocserv gnutls-bin dnsmasq-base busybox curl openssl python3
        check "openvpn, as Recommends would" apt-get install -y -qq --no-install-recommends openvpn
    fi
    # systemd-resolved, when apt just pulled it in (24.04 and newer), started
    # before D-Bus read its policy and isn't on the bus; on a real desktop
    # it was there all along. Restarted, as a reboot would.
    systemctl restart systemd-resolved
}

# A lookup craked's own DNS answered REFUSED, within a second (not one
# that failed because nothing answered).
refused_at_once() {
    start=$(date +%s%N)
    out=$(resolvectl query "$1" 2>&1) && return 1
    took=$(( ($(date +%s%N) - start) / 1000000 ))
    echo "$out (${took} ms)"
    echo "$out" | grep -q REFUSED && [ "$took" -lt 1000 ]
}

session_arg() { sed -n "s/^$1='\(.*\)'$/\1/p" /tmp/auth; }

tunnel() {
    check "the gateway is up" sh /deb-test/gateway.sh
    check "ACME added" call AddTunnel 'a{ss}' 4 display_name ACME gateway https://198.51.100.10 protocol anyconnect user alice
    # The app's sign-in, by hand: trust the certificate, the password.
    check "signed in" sh -c "printf 'yes\nhunter2\n' | openconnect --authenticate --user=alice https://198.51.100.10 > /tmp/auth"
    check "connects with that session" call ConnectWithSession 'sa{ss}' acme 4 \
        cookie "$(session_arg COOKIE)" connect_url "$(session_arg CONNECT_URL)" \
        fingerprint "$(session_arg FINGERPRINT)" resolve "$(session_arg RESOLVE)"
    for _ in $(seq 1 40); do call GetTunnel s acme | grep -q '"connected"' && break; sleep 0.5; done
    check "connected" sh -c "busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 GetTunnel s acme | grep -q '\"connected\"'"
    if aa-enabled --quiet 2>/dev/null; then
        check "openconnect runs confined too" sh -c 'ps -eo label,comm | grep -Eq "^craked \((enforce|complain)\) +openconnect"'
    fi
    check "its address on crake-acme" sh -c 'ip -4 -o addr show crake-acme | grep -q "inet 10.40.255."'
    check "the gateway's network isn't routed" sh -c '! ip -4 route show table all | grep -q "10.40.0.0/16 dev crake-acme"'
    # Probing: no answer directly, so tried through ACME; the retry gets in.
    check "a host that doesn't answer directly gets through" sh -c 'curl -s -m 15 http://10.40.1.10/ | grep -q crex'
    # DNS: resolved asks ACME's DNS for its domain; routed before the first packet.
    check "resolved asks ACME's DNS for acme.internal" sh -c 'resolvectl query wiki.acme.internal | grep -q 10.40.1.11'
    check "the name's address is routed at once" sh -c 'curl -s -m 1 http://10.40.1.11/ | grep -q crex'
    check "both learned" sh -c "nft list set inet crake learned_acme | grep -q 10.40.1.10 && nft list set inet crake learned_acme | grep -q 10.40.1.11"
    # A report (Settings → Diagnostics → "Save a report…"), the service's side: resolved's
    # view of ACME's link (resolvectl, run under craked's profile), ACME's
    # DNS route marked as Crake's, and Crake's own table.
    check "a report has resolved's view, ACME's route and Crake's table" sh -c 'busctl --system --timeout=90 call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 Report > /tmp/report && grep -q "Link [0-9]* (crake-acme)" /tmp/report && grep -q "Crake: .* DNS" /tmp/report && grep -q "table inet crake" /tmp/report || { cat /tmp/report; false; }'
    # Another program rewriting the tunnel's DNS settings (openvpn3 does,
    # for all its devices, when one of its sessions reconnects): its domain
    # as a search domain, every name to it. craked puts its own back.
    check "another program rewrites ACME's DNS settings" sh -c 'resolvectl domain crake-acme acme.internal "~." && resolvectl default-route crake-acme yes'
    # craked sets the domains and the default route in two calls: wait for
    # both (waiting for the domains alone once caught it between them).
    restored='resolvectl domain crake-acme | grep -q "): ~acme.internal\$" && resolvectl default-route crake-acme | grep -q "): no\$"'
    for _ in $(seq 1 20); do sh -c "$restored" && break; sleep 0.5; done
    check "craked puts its own back" sh -c "$restored"
    # A package upgrade (reinstalling runs the same scripts): ACME is
    # handed to the new craked, stays signed in and comes back by itself.
    if [ "$PM" = pacman ]; then
        check "upgrades" pacman -U --noconfirm /pkg/crake.pkg.tar.zst
    elif [ "$PM" = dnf ]; then
        check "upgrades" dnf reinstall -y -q /pkg/crake.rpm
    else
        check "upgrades" dpkg -i /pkg/crake.deb
    fi
    for _ in $(seq 1 40); do call GetTunnel s acme | grep -q '"connected"' && break; sleep 0.5; done
    check "ACME is back after the upgrade" sh -c "busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 GetTunnel s acme | grep -q '\"connected\"'"
    check "the log says Crake was updated" grep -q "Crake was updated. Your tunnels are reconnecting with your sign-ins." /var/lib/crake/log
    check "what it learned still goes through" sh -c 'curl -s -m 3 http://10.40.1.11/ | grep -q crex'
    check "no sessions left lying around" test ! -e /run/crake/resume.json
    check "switches off" call Disconnect s acme
    sleep 2
    # Keeping names private: with ACME off, its names are answered at once
    # by craked's own DNS (port 53 on crake-hush), never asked publicly.
    check "crake-hush is up" ip link show crake-hush
    check "ACME's names are refused at once" refused_at_once wiki.acme.internal
    # Another VPN client serving ACME's domain (AnyConnect beside Crake, as
    # on a colleague's Mac): its names are left to it, not refused.
    check "another VPN client serves acme.internal" sh -c 'ip link add other0 type dummy && ip link set other0 up && resolvectl dns other0 10.40.0.2 && resolvectl domain other0 "~acme.internal"'
    for _ in $(seq 1 20); do resolvectl domain crake-hush 2>/dev/null | grep -q acme.internal || break; sleep 0.5; done
    check "Crake leaves acme.internal to it" sh -c '! resolvectl domain crake-hush 2>/dev/null | grep -q acme.internal'
    check "the other client goes" ip link del other0
    for _ in $(seq 1 20); do resolvectl domain crake-hush 2>/dev/null | grep -q acme.internal && break; sleep 0.5; done
    check "acme.internal is kept private again" sh -c 'resolvectl domain crake-hush | grep -q acme.internal'
    openvpn_tunnel
    check "nothing went wrong in craked's journal" sh -c '! journalctl -b -u craked --no-pager -o cat | grep -E "ERROR|panicked"'
    if aa-enabled --quiet 2>/dev/null; then
        # Complain mode (scripts/vm-test.sh --complain) lists what the
        # profile would have refused: ALLOWED instead of DENIED.
        check "AppArmor refused craked nothing" sh -c '! journalctl -k --no-pager -o cat | grep -E "apparmor=\"(DENIED|ALLOWED)\"" | grep "craked"'
    fi
    if [ "$(getenforce 2>/dev/null)" = Enforcing ]; then
        check "SELinux refused craked nothing" sh -c '! journalctl --no-pager -o cat | grep "avc:  denied" | grep -E "comm=\"(craked|openconnect|nft|ip|hook|sh)\""'
        check "craked runs as a confined-or-unconfined service" sh -c 'ps -eZ | grep -E " craked$"'
    fi
    echo "  craked's journal:"
    journalctl -b -u craked --no-pager -o cat | sed 's/^/        /'
}

# craked's question about TUNNEL's sign-in, once it asks for KIND: answered
# with TEXT, as the app's sign-in window does (D-Bus Asking and Answer).
answer() {
    for _ in $(seq 1 40); do
        asked=$(call Asking s "$1" 2>&1)
        echo "$asked" | grep -q "\"kind\" \"$2\"" && break
        sleep 0.5
    done
    id=$(echo "$asked" | grep -o '"id" "[0-9]*"' | grep -o '[0-9][0-9]*')
    call Answer sus "$1" "$id" "$3"
}

# An OpenVPN tunnel through the installed service (CLAUDE.md, 7i): craked
# runs this system's openvpn (under its AppArmor profile, where there is
# one) against ACME's OpenVPN gateway, with a password sign-in. ACME's
# openconnect tunnel is off by now: both push the same DNS server.
openvpn_tunnel() {
    ca=/tmp/openvpn-ca.crt
    [ -e /link/openvpn-ca.crt ] && ca=/link/openvpn-ca.crt
    # Here (not Arch, whose gateway is a container of its own): its
    # openvpn has to be up first, or nothing after says much.
    if [ -e /tmp/openvpn-gw.log ]; then
        for _ in $(seq 1 20); do grep -q "Initialization Sequence Completed" /tmp/openvpn-gw.log && break; sleep 0.5; done
        check "ACME's OpenVPN gateway is up" sh -c 'grep -q "Initialization Sequence Completed" /tmp/openvpn-gw.log || { cat /tmp/openvpn-gw.log; false; }'
    fi
    printf 'client\ndev tun\nproto udp\nremote 198.51.100.10 1194\nnobind\nauth-user-pass\nremote-cert-tls server\n<ca>\n%s\n</ca>\n' \
        "$(cat "$ca")" > /tmp/initech.ovpn
    check "Initech (OpenVPN) added" call AddTunnel 'a{ss}' 3 display_name Initech protocol openvpn profile "$(cat /tmp/initech.ovpn)"
    check "its profile is root's alone" sh -c 'test "$(stat -c %a /var/lib/crake/profiles/initech.ovpn)" = 600'
    check "switched on" call Connect s initech
    check "asked for the username" answer initech user alice
    check "asked for the password" answer initech password hunter2
    for _ in $(seq 1 40); do call GetTunnel s initech | grep -q '"connected"' && break; sleep 0.5; done
    check "Initech connected" sh -c "busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 GetTunnel s initech | grep -q '\"connected\"'"
    if aa-enabled --quiet 2>/dev/null; then
        check "openvpn runs confined too" sh -c 'ps -eo label,comm | grep -Eq "^craked \((enforce|complain)\) +openvpn"'
    fi
    check "its address on crake-initech" sh -c 'ip -4 -o addr show crake-initech | grep -q "inet 10.40.254."'
    check "a host that doesn't answer directly gets through Initech" sh -c 'curl -s -m 15 http://10.40.1.12/ | grep -q crex'
    check "Initech switches off" call Disconnect s initech
    sleep 2
    check "its openvpn stopped" sh -c '! ps -eo args | grep -v grep | grep -q "openvpn-initech.conf"'
    check "Initech removed" call RemoveTunnel s initech
    check "its profile went with it" test ! -e /var/lib/crake/profiles/initech.ovpn
}

# The journey (crates/crake/src/journey_tests.rs): the real app and its own
# craked against this system's openconnect, nft and ip, with the packaged
# hook, through the gateway the tunnel phase started. The service steps
# aside meanwhile (one craked at a time).
journey() {
    [ -x /pkg/journey ] || { echo "  skip  the journey: no target/deb/journey (scripts/build-deb.sh)"; return; }
    hook=/usr/libexec/crake/hook
    [ "$PM" = pacman ] && hook=/usr/lib/crake/hook
    systemctl stop craked
    mkdir -p /tmp/journey
    check "a day with Crake (the journey)" env CRAKE_E2E=1 CRAKE_E2E_HOOK="$hook" TMPDIR=/tmp/journey \
        /pkg/journey journey_tests --test-threads=1
    systemctl start craked
}

remove() {
    if [ "$PM" = pacman ]; then
        check "removes" pacman -R --noconfirm crake
    elif [ "$PM" = dnf ]; then
        check "removes" dnf remove -y -q crake
    else
        check "removes" apt-get remove -y -qq crake
    fi
    check "craked stopped" sh -c '! systemctl is-active -q craked'
    check "and stopped without an error" sh -c '! journalctl -b -u craked --no-pager -o cat | grep -E "ERROR|panicked"'
    check "no crake links left" sh -c '! ip -o link | grep -q crake-'
    check "no nftables table left" sh -c '! nft list tables 2>/dev/null | grep -q crake'
    # rpm has no purge: removing takes craked's state too.
    [ "$PM" != apt ] || check "purges" apt-get purge -y -qq crake
    check "its state is gone" test ! -e /var/lib/crake
}

"$1"
exit $fail
