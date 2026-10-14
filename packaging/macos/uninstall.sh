#!/bin/sh
# Crake taken off this Mac: sudo /usr/local/libexec/crake/uninstall
# craked stops first (switching every tunnel off takes its routes, DNS
# settings and pf rules with it), then its files go, its state and log in
# /var/db/crake too; with Crake.app gone, macOS drops its background items
# (craked and "Start at login", both registered by the app). An older
# Crake's own launchd daemon goes too. The app's own settings in your home
# folder stay.
set -u
if [ "$(id -u)" != 0 ]; then
    echo "Run it with sudo: sudo $0" >&2
    exit 1
fi
LABEL=software.dorhawk.crake.daemon
# 0.6 and before: craked's own launchd daemon, and the login agent file.
OLD_LABEL=software.dorhawk.craked
launchctl bootout "system/$LABEL" 2>/dev/null
launchctl bootout "system/$OLD_LABEL" 2>/dev/null
user=$(stat -f %Su /dev/console 2>/dev/null)
if [ -n "$user" ] && [ "$user" != root ]; then
    uid=$(id -u "$user")
    pkill -x -U "$uid" crake 2>/dev/null
    launchctl bootout "gui/$uid/software.dorhawk.crake.login" 2>/dev/null
    launchctl bootout "gui/$uid/software.dorhawk.crake" 2>/dev/null
fi
rm -rf /Applications/Crake.app /usr/local/libexec/crake /var/db/crake /var/run/crake
rm -f "/Library/LaunchDaemons/$OLD_LABEL.plist" /usr/local/sbin/craked \
    /var/log/craked.log /var/run/crake.sock
pkgutil --forget software.dorhawk.crake >/dev/null 2>&1
echo "Crake is removed."
