#!/bin/sh
# The package repository as users meet it (scripts/build-repo.sh), checked
# by apt (Debian 13, Ubuntu 22.04) and dnf (Fedora 44) in throwaway
# containers: the index's signature, the package's checksum, the RPM's own
# signature, the version offered.
#   scripts/repo-test.sh            the .deb and RPM in target/, signed with
#                                   a throwaway key, served from a folder to
#                                   containers with no network; a tampered
#                                   index must be refused
#   scripts/repo-test.sh --live URL what's published there (release.yml,
#                                   after publishing)
set -eu
cd "$(dirname "$0")/.."

# One client per system: apt or dnf told about the repository the way the
# install instructions say, then made to fetch crake. $1: image, $2: where
# the repository is (file:///repo or https://...), $3: docker's network.
apt_client() {
    docker run --rm --network "$3" ${REPO:+-v "$REPO:/repo:ro"} -e URL="$2" "$1" sh -euc '
      fetch() { case "$URL" in file://*) cat "${URL#file://}/$1" ;; *) curl -fsSL "$URL/$1" ;; esac; }
      if ! command -v curl >/dev/null && [ "${URL#file://}" = "$URL" ]; then
        apt-get update -qq >/dev/null && apt-get install -y -qq curl ca-certificates >/dev/null 2>&1
      fi
      install -d /etc/apt/keyrings /tmp/src
      fetch key.asc > /etc/apt/keyrings/dorhawk.asc
      fetch apt/dorhawk.sources > /tmp/src/dorhawk.sources
      only="-o Dir::Etc::SourceList=/dev/null -o Dir::Etc::SourceParts=/tmp/src"
      apt-get $only update 2>&1 | tail -3
      apt-get $only update 2>&1 | grep -q "^[EW]:" && exit 1
      cd /tmp && apt-get $only download crake >/dev/null
      printf "%s: crake %s\n" "$(. /etc/os-release; echo $PRETTY_NAME)" "$(dpkg-deb -f /tmp/crake_*.deb Version)"'
}

dnf_client() {
    docker run --rm --network "$3" ${REPO:+-v "$REPO:/repo:ro"} -e URL="$2" "$1" sh -euc '
      case "$URL" in
        file://*) cp "${URL#file://}/rpm/dorhawk.repo" /etc/yum.repos.d/ ;;
        *) curl -fsSL -o /etc/yum.repos.d/dorhawk.repo "$URL/rpm/dorhawk.repo" ;;
      esac
      dnf -y -q --disablerepo="*" --enablerepo=dorhawk makecache
      dnf -y -q --disablerepo="*" --enablerepo=dorhawk download --destdir /tmp crake
      case "$URL" in
        file://*) cp "${URL#file://}/key.asc" /tmp/key.asc ;;
        *) curl -fsSL -o /tmp/key.asc "$URL/key.asc" ;;
      esac
      rpm --import /tmp/key.asc
      rpm --checksig /tmp/crake-*.rpm | grep -q "signatures OK"
      printf "%s: crake %s\n" "$(. /etc/os-release; echo $PRETTY_NAME)" "$(rpm -qp --qf "%{VERSION}-%{RELEASE}" /tmp/crake-*.rpm)"'
}

if [ "${1:-}" = --live ]; then
    URL=$2
    REPO=
    apt_client debian:13 "$URL" bridge
    apt_client ubuntu:22.04 "$URL" bridge
    dnf_client fedora:44 "$URL" bridge
    echo "$URL: OK"
    exit 0
fi

WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-repo.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
REPO=$WORK/repo
mkdir -p "$REPO/apt/pool/main" "$REPO/rpm" "$WORK/key"
cp target/deb/crake_*.deb "$REPO/apt/pool/main/"
cp target/rpm/crake-*.rpm "$REPO/rpm/"
# A key for this test only: the real one stays in CI's secrets.
docker run --rm -v "$WORK/key:/out" -e HOST_UID="$(id -u)" debian:13 sh -euc '
  apt-get update -qq >/dev/null && apt-get install -y -qq gnupg >/dev/null 2>&1
  export GNUPGHOME=$(mktemp -d)
  gpg --batch --quiet --passphrase "" --quick-gen-key "repo test <test@example.org>" rsa3072 sign never 2>/dev/null
  gpg --armor --export-secret-keys > /out/private.asc
  chown "$HOST_UID" /out/private.asc'
PACKAGES_GPG_KEY=$(cat "$WORK/key/private.asc")
export PACKAGES_GPG_KEY
scripts/sign-rpm.sh "$REPO/rpm" >/dev/null
scripts/build-repo.sh "$REPO" file:///repo
apt_client debian:13 file:///repo none
apt_client ubuntu:22.04 file:///repo none
dnf_client fedora:44 file:///repo none
# Tampered with after signing (both forms: apt reads the compressed one):
# apt must refuse the index.
INDEX=$REPO/apt/dists/stable/main/binary-amd64
echo "Description: tampered" >> "$INDEX/Packages"
gzip -9cn "$INDEX/Packages" > "$INDEX/Packages.gz"
if apt_client debian:13 file:///repo none >/dev/null 2>&1; then
    echo "FAIL: apt took an index that doesn't match its signed Release" >&2
    exit 1
fi
echo "a tampered index: refused"
echo "repository: OK"
