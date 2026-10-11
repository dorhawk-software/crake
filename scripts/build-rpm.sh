#!/bin/sh
# Build crake's RPM for Fedora from the files scripts/build-deb.sh staged in
# target/deb/root: the same binaries as the .deb (built on Ubuntu 22.04, so
# they run on any newer glibc), packed with packaging/rpm/crake.spec in a
# Fedora container. The result is target/rpm/crake-<version>-1.x86_64.rpm.
# Test it with scripts/deb-test.sh fedora:43 fedora:44 fedora:45.
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
[ -d target/deb/root/usr/sbin ] || scripts/build-deb.sh >/dev/null
mkdir -p target/rpm
docker run --rm -v "$PWD/packaging/rpm:/spec:ro" -v "$PWD/target/deb/root:/root-tree:ro" \
  -v "$PWD/target/rpm:/out" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" fedora:43 sh -euc '
    dnf install -q -y rpm-build >/dev/null
    rpmbuild -bb --quiet /spec/crake.spec \
      --define "crake_version '"$VERSION"'" --define "crake_root /root-tree" \
      --define "_rpmdir /tmp/rpms" --define "_build_name_fmt %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" \
      --define "dist %{nil}"
    cp /tmp/rpms/*.rpm /out/ && chown "$HOST_UID:$HOST_GID" /out/*.rpm
    rpm -qp --requires /out/*.rpm | grep -v "^rpmlib" | sed "s/^/  requires: /"'
ls target/rpm/*.rpm
