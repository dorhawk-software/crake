#!/bin/sh
# Inside scripts/vm-test.sh's VM, run by cloud-init at first boot as root:
# the package test (scripts/deb-test/check.sh) on a real Ubuntu kernel,
# with AppArmor on. Its output goes to the second serial port; the last
# line says how it went.
set -u
ln -sfn /test/deb-test /deb-test
ln -sfn /test/e2e /e2e
mkdir -p /pkg
ln -sf /test/crake.deb /pkg/crake.deb
fail=0
echo "== Ubuntu $(. /etc/os-release; echo "$VERSION_ID"), kernel $(uname -r), AppArmor $(aa-enabled 2>&1)"
sh /deb-test/check.sh install || fail=1
if [ -f /test/complain ]; then
    echo "  (craked's profile in complain mode)"
    apparmor_parser -r -C /etc/apparmor.d/usr.sbin.craked && systemctl restart craked
fi
sh /deb-test/check.sh tunnel || fail=1
echo "  AppArmor's log for craked:"
journalctl -k --no-pager -o cat | grep 'apparmor=' | grep craked | sed 's/^/        /'
sh /deb-test/check.sh remove || fail=1
echo "VM-TEST-RESULT: $fail"
