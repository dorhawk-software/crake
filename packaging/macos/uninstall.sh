#!/bin/sh
# Crake taken off this Mac: sudo /usr/local/libexec/crake/uninstall
# craked stops first (switching every tunnel off takes its routes, DNS
# settings and pf rules with it), then its files go, its state and log in
# /var/db/crake too. The app's own settings in your home folder stay.
set -u
if [ "$(id -u)" != 0 ]; then
    echo "Run it with sudo: sudo $0" >&2
    exit 1
fi
LABEL=software.dorhawk.craked
launchctl bootout "system/$LABEL" 2>/dev/null
rm -rf /Applications/Crake.app /usr/local/libexec/crake /var/db/crake /var/run/crake
rm -f "/Library/LaunchDaemons/$LABEL.plist" /usr/local/sbin/craked \
    /var/log/craked.log /var/run/crake.sock
pkgutil --forget software.dorhawk.crake >/dev/null 2>&1
echo "Crake is removed."
