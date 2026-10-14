#!/bin/sh
# The macOS package on a throwaway Mac (CI job macos, GitHub's runner).
# craked is Crake.app's own service, registered by the app (SMAppService)
# and switched on once by the user in System Settings: a runner can't
# click that switch, so what's checked is everything around it.
# - From the release before (CRAKE_PREVIOUS_PKG, when given: craked was a
#   launchd daemon of its own then): installing over it stops that daemon
#   and takes its files away.
# - The app, with craked and both launchd files inside, signed.
# - Crake registers its service from inside the app: macOS's background
#   items record names the launchd file in Crake.app.
# - craked runs from inside the app and serves the app's socket.
# - Its windows open in front; installing again works; the uninstaller
#   leaves nothing.
# Usage: macos-pkg-test.sh <Crake-x.y.z.pkg>
set -eu
PKG=$1
LABEL=software.dorhawk.craked
APP=/Applications/Crake.app
OLD=/Library/LaunchDaemons/$LABEL.plist
fail() {
    echo "FAIL: $*" >&2
    sudo cat /var/log/craked.log >&2 2>/dev/null || true
    exit 1
}
# The app's socket served, within 30 s.
serving() {
    for _ in $(seq 30); do
        [ -S /var/run/crake.sock ] && return 0
        sleep 1
    done
    return 1
}
if [ -n "${CRAKE_KEYCHAIN:-}" ]; then
    pkgutil --check-signature "$PKG" | grep -q "Developer ID Installer" ||
        fail "the package isn't signed with the Developer ID"
fi
if [ -n "${CRAKE_PREVIOUS_PKG:-}" ]; then
    sudo installer -pkg "$CRAKE_PREVIOUS_PKG" -target /
    # Up to 0.6 craked was a launchd daemon of its own, running at once.
    if [ -e "$OLD" ]; then
        serving || fail "the release before didn't run its craked"
    fi
fi
sudo installer -pkg "$PKG" -target /
[ ! -e "$OLD" ] || fail "the older craked's launchd file is still there"
[ ! -e /usr/local/sbin/craked ] || fail "the older craked is still there"
if sudo launchctl print "system/$LABEL" 2>/dev/null | grep -q "$OLD"; then
    fail "the older craked still runs"
fi
test -x "$APP/Contents/MacOS/crake" || fail "no Crake.app"
test -x "$APP/Contents/MacOS/craked" || fail "no craked in Crake.app"
for f in "$APP/Contents/Library/LaunchDaemons/$LABEL.plist" \
    "$APP/Contents/Library/LaunchAgents/software.dorhawk.crake.plist" "$APP/Contents/Info.plist"; do
    plutil -lint "$f" >/dev/null || fail "$f"
done
test -x /usr/local/libexec/crake/hook || fail "no hook"
codesign --verify --deep --strict "$APP" || fail "Crake.app's signature"
# Built universal (CRAKE_UNIVERSAL): every binary for both kinds of Mac.
if [ -n "${CRAKE_UNIVERSAL:-}" ]; then
    for f in "$APP/Contents/MacOS/crake" "$APP/Contents/MacOS/craked" /usr/local/libexec/crake/hook; do
        archs=$(lipo -archs "$f")
        for arch in arm64 x86_64; do
            case " $archs " in
                *" $arch "*) ;;
                *) fail "$f is $archs, not universal" ;;
            esac
        done
    done
fi
if [ -n "${CRAKE_KEYCHAIN:-}" ]; then
    for f in "$APP" "$APP/Contents/MacOS/craked" /usr/local/libexec/crake/hook; do
        codesign -dv --verbose=2 "$f" 2>&1 | grep -q "Authority=Developer ID Application" ||
            fail "$f isn't signed with the Developer ID"
        codesign -dv --verbose=2 "$f" 2>&1 | grep -q "runtime" ||
            fail "$f lacks the hardened runtime"
    done
fi
# Its windows open in front of the app that's in front (and the app has run).
sh "$(dirname "$0")/macos-front-test.sh" || fail "Settings opened behind"
# Registered from inside the app: Crake, started as it starts for a user
# (the window check above ran it in demo mode, which touches nothing),
# registers craked, and macOS's record of it names the launchd file in
# Crake.app (System Settings lists it under Crake).
pkill -x crake 2>/dev/null || true
CRAKE_DEBUG=1 "$APP/Contents/MacOS/crake" >/tmp/crake-app.log 2>&1 &
registered=no
for _ in $(seq 30); do
    if sudo sfltool dumpbtm 2>/dev/null | grep -q "Crake.app/Contents/Library/LaunchDaemons/$LABEL.plist"; then
        registered=yes
        break
    fi
    sleep 1
done
pkill -x crake 2>/dev/null || true
echo "macOS's records of Crake's items:"
sudo sfltool dumpbtm 2>/dev/null | grep -B4 -A14 -i "dorhawk\|Crake.app" || true
if [ "$registered" != yes ]; then
    echo "Crake's own output:"
    grep -i "smappservice\|craked" /tmp/crake-app.log | tail -20
    fail "Crake didn't register its service from inside the app"
fi
# craked from inside the app serves the app's socket (launchd starts it
# there once the user switches it on; here it's started by hand).
sudo "$APP/Contents/MacOS/craked" 2>/tmp/craked-test.log &
serving || { cat /tmp/craked-test.log >&2; fail "craked from Crake.app isn't serving /var/run/crake.sock"; }
sudo pkill -TERM -x craked
for _ in $(seq 10); do pgrep -x craked >/dev/null || break; sleep 1; done
# Installed again over itself.
sudo installer -pkg "$PKG" -target /
test -x "$APP/Contents/MacOS/craked" || fail "no craked after installing again"
sudo /usr/local/libexec/crake/uninstall
for f in "$APP" /usr/local/sbin/craked /usr/local/libexec/crake "$OLD" /var/run/crake.sock; do
    [ ! -e "$f" ] || fail "$f left behind"
done
if pkgutil --pkg-info software.dorhawk.crake >/dev/null 2>&1; then
    fail "the package receipt left behind"
fi
echo "macOS package: OK"
