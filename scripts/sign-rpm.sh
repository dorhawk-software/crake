#!/bin/sh
# The RPMs in DIR signed in place with the package repository's key
# (PACKAGES_GPG_KEY: the armored private key; CLAUDE.md, Releases), in a
# throwaway Fedora container, so dnf's gpgcheck accepts them from
# packages.dorhawk.software and the GitHub release carries them signed.
#   PACKAGES_GPG_KEY=... scripts/sign-rpm.sh DIR
set -eu
DIR=$(cd "$1" && pwd)
[ -n "${PACKAGES_GPG_KEY:-}" ] || { echo "PACKAGES_GPG_KEY is empty" >&2; exit 1; }
docker run --rm -v "$DIR:/rpms" -e PACKAGES_GPG_KEY -e HOST_UID="$(id -u)" fedora:44 sh -euc '
  dnf install -y -q rpm-sign gnupg2 >/dev/null
  export GNUPGHOME=$(mktemp -d)
  printf "%s\n" "$PACKAGES_GPG_KEY" | gpg --batch --quiet --import
  fpr=$(gpg --list-secret-keys --with-colons | awk -F: "/^fpr/ { print \$10; exit }")
  for f in /rpms/*.rpm; do
    rpmsign --define "_gpg_name $fpr" --addsign "$f" >/dev/null
  done
  gpg --armor --export "$fpr" > /tmp/key.asc
  rpm --import /tmp/key.asc
  rpm --checksig /rpms/*.rpm
  chown "$HOST_UID" /rpms/*.rpm'
