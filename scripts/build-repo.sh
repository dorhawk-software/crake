#!/bin/sh
# The package repository at packages.dorhawk.software, built in DIR, which
# mirrors the bucket: DIR/apt/pool/main/*.deb and DIR/rpm/*.rpm (signed:
# scripts/sign-rpm.sh) are what it serves; this writes the indexes beside
# them and signs them with PACKAGES_GPG_KEY:
#   apt: DIR/apt/dists/stable/{Release,InRelease,Release.gpg} and
#        main/binary-amd64/Packages(.gz); DIR/apt/dorhawk.sources for users
#   dnf: DIR/rpm/repodata (repomd.xml signed: repomd.xml.asc);
#        DIR/rpm/dorhawk.repo for users
#   DIR/key.asc: the public key both check against
#   DIR/index.html: the start page (packaging/repo/index.html; served at /
#        by a Cloudflare rewrite rule, as R2 serves no index of its own)
# In throwaway containers (Debian's apt-ftparchive, Fedora's createrepo_c).
#   PACKAGES_GPG_KEY=... scripts/build-repo.sh DIR [BASE_URL]
set -eu
DIR=$(cd "$1" && pwd)
URL=${2:-https://packages.dorhawk.software}
[ -n "${PACKAGES_GPG_KEY:-}" ] || { echo "PACKAGES_GPG_KEY is empty" >&2; exit 1; }
mkdir -p "$DIR/apt/pool/main" "$DIR/rpm"
cp "$(dirname "$0")/../packaging/repo/index.html" "$DIR/index.html"
docker run --rm -v "$DIR:/repo" -e PACKAGES_GPG_KEY -e URL="$URL" -e HOST_UID="$(id -u)" debian:13 sh -euc '
  apt-get update -qq >/dev/null && apt-get install -y -qq --no-install-recommends apt-utils gnupg >/dev/null 2>&1
  export GNUPGHOME=$(mktemp -d)
  printf "%s\n" "$PACKAGES_GPG_KEY" | gpg --batch --quiet --import
  fpr=$(gpg --list-secret-keys --with-colons | awk -F: "/^fpr/ { print \$10; exit }")
  gpg --armor --export "$fpr" > /repo/key.asc
  cd /repo/apt
  rm -rf dists && mkdir -p dists/stable/main/binary-amd64
  apt-ftparchive packages pool > dists/stable/main/binary-amd64/Packages
  gzip -9kn dists/stable/main/binary-amd64/Packages
  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin=dorhawk.software \
    -o APT::FTPArchive::Release::Label=dorhawk.software \
    -o APT::FTPArchive::Release::Suite=stable \
    -o APT::FTPArchive::Release::Codename=stable \
    -o APT::FTPArchive::Release::Architectures=amd64 \
    -o APT::FTPArchive::Release::Components=main \
    -o "APT::FTPArchive::Release::Description=dorhawk.software apps" \
    release dists/stable > /tmp/Release
  mv /tmp/Release dists/stable/Release
  gpg --batch --yes --local-user "$fpr" --clearsign -o dists/stable/InRelease dists/stable/Release
  gpg --batch --yes --local-user "$fpr" -abs -o dists/stable/Release.gpg dists/stable/Release
  printf "Types: deb\nURIs: %s/apt\nSuites: stable\nComponents: main\nArchitectures: amd64\nSigned-By: /etc/apt/keyrings/dorhawk.asc\n" "$URL" > dorhawk.sources
  chown -R "$HOST_UID" /repo'
docker run --rm -v "$DIR:/repo" -e PACKAGES_GPG_KEY -e URL="$URL" -e HOST_UID="$(id -u)" fedora:44 sh -euc '
  dnf install -y -q createrepo_c gnupg2 rpm >/dev/null
  export GNUPGHOME=$(mktemp -d)
  printf "%s\n" "$PACKAGES_GPG_KEY" | gpg --batch --quiet --import
  fpr=$(gpg --list-secret-keys --with-colons | awk -F: "/^fpr/ { print \$10; exit }")
  rpm --import /repo/key.asc
  # Only signed packages are served: dnf checks each one.
  for f in /repo/rpm/*.rpm; do
    [ -e "$f" ] || continue
    rpm --checksig "$f" | grep -q "signatures OK" || { echo "$f is not signed with the key" >&2; exit 1; }
  done
  rm -rf /repo/rpm/repodata
  createrepo_c --quiet /repo/rpm
  gpg --batch --yes --local-user "$fpr" --detach-sign --armor /repo/rpm/repodata/repomd.xml
  printf "[dorhawk]\nname=dorhawk.software\nbaseurl=%s/rpm\nenabled=1\ngpgcheck=1\nrepo_gpgcheck=1\ngpgkey=%s/key.asc\n" "$URL" "$URL" > /repo/rpm/dorhawk.repo
  chown -R "$HOST_UID" /repo'
