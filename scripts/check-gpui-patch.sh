#!/bin/sh
# Crake's patched GPUI is the one in use: Cargo.lock's gpui-pre-linux is the
# vendored copy (no `source`, it's a path), at the version GPUI asks for.
# Otherwise Cargo quietly drops the [patch] (a warning, not an error) and
# builds the unpatched crate from crates.io: the tray panel's X11 placement
# would be gone with every test still green. Run by scripts/lint.sh.
set -eu
cd "$(dirname "$0")/.."
# Once upstream has the change, nothing is vendored and nothing to check.
[ -d vendor/gpui-pre-linux ] || exit 0
entry=$(awk '/^\[\[/ {p=0} /^name = "gpui-pre-linux"$/ {p=1} p' Cargo.lock)
if [ -z "$entry" ]; then
  echo "gpui-pre-linux isn't in Cargo.lock"
  exit 1
fi
if echo "$entry" | grep -q '^source = '; then
  echo "Cargo.lock takes gpui-pre-linux from crates.io, not vendor/gpui-pre-linux:"
  echo "Crake's patch isn't applied. Run scripts/update-gpui.sh."
  exit 1
fi
locked=$(echo "$entry" | sed -n 's/^version = "\(.*\)"/\1/p')
vendored=$(sed -n 's/^version = "\(.*\)"/\1/p' vendor/gpui-pre-linux/Cargo.toml | head -n1)
if [ "$locked" != "$vendored" ]; then
  echo "Cargo.lock has gpui-pre-linux $locked, vendor/ has $vendored"
  exit 1
fi
