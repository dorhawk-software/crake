#!/bin/sh
# Moves the base layer of Nix's dependency builds (flake.nix, crake-base;
# packaging/nix/package.nix) to main's latest commit, or to the commit
# given: CI then builds the base once from scratch (~45 min) and the layer
# on top is quick again. Worth it when that layer has grown (many
# dependency updates since the base). Commit flake.lock afterwards.
# Without Nix here, it runs in the nixos/nix container (store in the
# crake-nix volume, as scripts/nix-test.sh).
#   scripts/nix-base.sh [COMMIT]
set -eu
cd "$(dirname "$0")/.."
REF=github:dorhawk-software/crake${1:+/$1}
if command -v nix >/dev/null; then
    NIX_CONFIG="experimental-features = nix-command flakes" \
        nix flake lock --override-input crake-base "$REF"
else
    docker run --rm -v "$PWD:/src" -v crake-nix:/nix -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
        -e REF="$REF" nixos/nix sh -euc '
        git config --global --add safe.directory /src
        trap "chown -R $HOST_UID:$HOST_GID /src/.git; chown $HOST_UID:$HOST_GID /src/flake.lock" EXIT
        cd /src
        nix --extra-experimental-features "nix-command flakes" flake lock --override-input crake-base "$REF"'
fi
git --no-pager diff --stat flake.lock
