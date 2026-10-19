#!/bin/sh
# Inside scripts/vm-test.sh's VM, run by cloud-init at first boot as root:
# the package test (scripts/deb-test/check.sh) on a real Ubuntu kernel,
# with AppArmor on. Its output goes to the second serial port; the last
# line says how it went.
set -u
ln -sfn /test/deb-test /deb-test
ln -sfn /test/e2e /e2e
mkdir -p /pkg
for f in /test/crake.deb /test/crake.rpm /test/journey; do [ -f "$f" ] && ln -sf "$f" /pkg/; done
fail=0
echo "== $(. /etc/os-release; echo "$NAME $VERSION_ID"), kernel $(uname -r), AppArmor $(aa-enabled 2>/dev/null || echo no), SELinux $(getenforce 2>/dev/null || echo no)"
sh /deb-test/check.sh install || fail=1
if [ -f /test/complain ] && command -v apparmor_parser >/dev/null; then
    echo "  (craked's profiles in complain mode: its own and its charon's)"
    apparmor_parser -r -C /etc/apparmor.d/usr.sbin.craked && apparmor_parser -r -C /etc/apparmor.d/crake-charon \
        && systemctl restart craked
fi
sh /deb-test/check.sh tunnel || fail=1
echo "  AppArmor's log for craked and its charon:"
journalctl -k --no-pager -o cat | grep 'apparmor=' | grep -E 'craked|crake-charon' | sed 's/^/        /'
echo "  SELinux's denials:"
journalctl --no-pager -o cat | grep 'avc:  denied' | sed 's/^/        /'
sh /deb-test/check.sh journey || fail=1
sh /deb-test/check.sh remove || fail=1
echo "VM-TEST-RESULT: $fail"
