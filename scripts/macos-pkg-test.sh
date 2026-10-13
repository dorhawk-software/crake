#!/bin/sh
# The macOS package on a throwaway Mac (CI job macos, GitHub's runner): it
# installs, launchd runs craked, which serves the app's socket; installing
# it again over the running one (an upgrade) brings craked back; the
# uninstaller leaves nothing. Usage: macos-pkg-test.sh <Crake-x.y.z.pkg>
set -eu
PKG=$1
LABEL=software.dorhawk.craked

fail() {
    echo "FAIL: $*" >&2
    sudo cat /var/log/craked.log >&2 2>/dev/null || true
    exit 1
}

# craked serving the app's socket, under launchd, within 30 s.
serving() {
    for _ in $(seq 30); do
        if [ -S /var/run/crake.sock ] && sudo launchctl print "system/$LABEL" | grep -q "state = running"; then
            return 0
        fi
        sleep 1
    done
    fail "craked isn't serving /var/run/crake.sock"
}

pid() {
    sudo launchctl print "system/$LABEL" | sed -n 's/^[[:space:]]*pid = //p'
}

sudo installer -pkg "$PKG" -target /
serving
APP=/Applications/Crake.app
test -x "$APP/Contents/MacOS/crake" || fail "no Crake.app"
plutil -lint "$APP/Contents/Info.plist" >/dev/null || fail "Info.plist"
codesign --verify --deep "$APP" || fail "Crake.app's signature"
test -x /usr/local/libexec/crake/hook || fail "no hook"
first=$(pid)

# An upgrade: installed again over the running craked.
sudo installer -pkg "$PKG" -target /
serving
[ "$(pid)" != "$first" ] || fail "craked wasn't restarted by the upgrade"

sudo /usr/local/libexec/crake/uninstall
if sudo launchctl print "system/$LABEL" >/dev/null 2>&1; then
    fail "craked still loaded"
fi
for f in "$APP" /usr/local/sbin/craked /usr/local/libexec/crake \
    /Library/LaunchDaemons/$LABEL.plist /var/run/crake.sock; do
    [ ! -e "$f" ] || fail "$f left behind"
done
if pkgutil --pkg-info software.dorhawk.crake >/dev/null 2>&1; then
    fail "the package receipt left behind"
fi
echo "macOS package: OK"
