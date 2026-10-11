#!/bin/sh
# Build Crake with Nix and run the NixOS VM test (flake.nix:
# checks.x86_64-linux.vm: a gateway and a client with services.crake on
# their own network), in the nixos/nix container: Nix isn't installed on
# this machine. /dev/kvm goes in for the test's VMs; the Nix store is kept
# in a Docker volume (crake-nix) between runs.
#
#   scripts/nix-test.sh [lock]     # lock: (re)write flake.lock first
set -eu
cd "$(dirname "$0")/.."
LOCK=${1:-}
docker run --rm -v "$PWD:/src" -v crake-nix:/nix --device /dev/kvm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" -e LOCK="$LOCK" nixos/nix sh -euc '
    git config --global --add safe.directory /src
    export NIX_CONFIG="experimental-features = nix-command flakes
system-features = kvm nixos-test benchmark big-parallel"
    cd /src
    if [ -n "$LOCK" ] || [ ! -f flake.lock ]; then
      nix flake lock && chown "$HOST_UID:$HOST_GID" flake.lock
    fi
    nix build --print-build-logs --no-link .#default
    nix build --print-build-logs --no-link .#checks.x86_64-linux.vm'
