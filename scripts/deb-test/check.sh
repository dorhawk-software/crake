#!/bin/sh
# Inside scripts/deb-test.sh's container (systemd running), one phase per
# call:
#   install  install the package with apt; craked runs as the service it
#            should, on the system bus, guarded by polkit
#   tunnel   (no network any more) a real tunnel through the installed
#            service: ocserv in its own namespace (gateway.sh), probing,
#            DNS through systemd-resolved, keeping names private
#   remove   remove and purge; nothing is left
# Prints each check; exits non-zero if any failed.
set -u
fail=0
ok() { echo "  ok    $1"; }
bad() { echo "  FAIL  $1"; fail=1; }
check() { name=$1; shift; if "$@" >/tmp/out 2>&1; then ok "$name"; else bad "$name"; sed 's/^/        /' /tmp/out; fi; }
call() { busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 "$@"; }
as_nobody() { setpriv --reuid=nobody --regid=nogroup --clear-groups "$@"; }
export DEBIAN_FRONTEND=noninteractive

install() {
    apt-get update -qq >/dev/null
    check "installs with its Depends only" apt-get install -y -qq --no-install-recommends /pkg/crake.deb
    echo "  engine: $(openconnect --version 2>/dev/null | head -n1)"
    check "every library the app needs is there" sh -c '! ldd /usr/bin/crake | grep "not found"'
    check "the hook is where craked looks" test -x /usr/libexec/crake/hook
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
    check "polkit refuses others" sh -c "setpriv --reuid=nobody --regid=nogroup --clear-groups dbus-send --system --print-reply --dest=software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1.RemoveTunnel string:globex 2>&1 | grep 'not authorized to switch tunnels'"
    check "anyone may read" as_nobody busctl --system call software.dorhawk.Crake1 /software/dorhawk/Crake1 software.dorhawk.Crake1 Settings
    # For the tunnel phase, while the archive can still be reached.
    check "the gateway's software" apt-get install -y -qq --no-install-recommends ocserv gnutls-bin dnsmasq-base busybox curl
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
    # A package upgrade (reinstalling runs the same scripts): ACME is
    # handed to the new craked, stays signed in and comes back by itself.
    check "upgrades" dpkg -i /pkg/crake.deb
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
    check "nothing went wrong in craked's journal" sh -c '! journalctl -b -u craked --no-pager -o cat | grep -E "ERROR|panicked"'
    if aa-enabled --quiet 2>/dev/null; then
        # Complain mode (scripts/vm-test.sh --complain) lists what the
        # profile would have refused: ALLOWED instead of DENIED.
        check "AppArmor refused craked nothing" sh -c '! journalctl -k --no-pager -o cat | grep -E "apparmor=\"(DENIED|ALLOWED)\"" | grep "craked"'
    fi
    echo "  craked's journal:"
    journalctl -b -u craked --no-pager -o cat | sed 's/^/        /'
}

remove() {
    check "removes" apt-get remove -y -qq crake
    check "craked stopped" sh -c '! systemctl is-active -q craked'
    check "and stopped without an error" sh -c '! journalctl -b -u craked --no-pager -o cat | grep -E "ERROR|panicked"'
    check "no crake links left" sh -c '! ip -o link | grep -q crake-'
    check "no nftables table left" sh -c '! nft list tables 2>/dev/null | grep -q crake'
    check "purges" apt-get purge -y -qq crake
    check "its state is gone" test ! -e /var/lib/crake
}

"$1"
exit $fail
