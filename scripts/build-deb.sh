#!/bin/sh
# Build crake's .deb in an Ubuntu 22.04 container (the oldest release
# supported: one .deb for 22.04 and everything newer), with current Rust
# from rustup. Nothing is installed on this machine; the result is
# target/deb/crake_<version>_amd64.deb. Test it with scripts/deb-test.sh.
#
# The container downloads rustup, the crates and Ubuntu's build packages;
# both are kept in Docker volumes (crake-deb-*) between runs.
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
mkdir -p target/deb
docker run --rm -v "$PWD:/src:ro" -v "$PWD/target/deb:/out" \
  -v crake-deb-cargo:/root/.cargo -v crake-deb-rustup:/root/.rustup \
  -v crake-deb-target:/target \
  -e VERSION="$VERSION" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  ubuntu:22.04 sh -eu /src/packaging/deb/build-inside.sh
echo "target/deb/crake_${VERSION}_amd64.deb"
