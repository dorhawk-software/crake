#!/bin/sh
# GPUI kept current with Crake's patch on it (vendor/gpui-pre-linux.patch,
# vendor/gpui-pre-linux/CRAKE-PATCH.md). Finds the newest gpui-kit on
# crates.io and the gpui-pre-linux its GPUI pins, takes that version's
# pristine source, applies the patch, vendors the result and moves
# Cargo.lock to that gpui-kit. Run weekly by .github/workflows/gpui.yml,
# which opens a pull request (CI then tests it) or, when the patch no
# longer applies, an issue.
#
#   scripts/update-gpui.sh [gpui-kit version]
#
# Exits 0 when it updated or nothing was newer (it says which), 2 when the
# patch doesn't apply to the new GPUI.
set -eu
cd "$(dirname "$0")/.."
UA="crake (https://github.com/dorhawk-software/crake)"
api() { curl -fsSL -A "$UA" "https://crates.io/api/v1/crates/$1"; }
pin() { api "$1/$2/dependencies" | jq -r --arg c "$3" '.dependencies[] | select(.crate_id == $c) | .req' | head -n1 | tr -d '=^ '; }

KIT=${1:-$(api gpui-kit | jq -r .crate.max_stable_version)}
# gpui-kit pins gpui-pre-platform, which pins Linux's backend.
PRE=$(pin gpui-kit "$KIT" gpui-pre-platform)
LINUX=$(pin gpui-pre-platform "$PRE" gpui-pre-linux)
[ -n "$PRE" ] && [ -n "$LINUX" ] || { echo "gpui-kit $KIT: no gpui-pre-linux found" >&2; exit 1; }
VENDORED=$(sed -n 's/^version = "\(.*\)"/\1/p' vendor/gpui-pre-linux/Cargo.toml | head -n1)
LOCKED=$(grep -A1 '^name = "gpui-kit"$' Cargo.lock | sed -n 's/^version = "\(.*\)"/\1/p')
if [ "$LINUX" = "$VENDORED" ] && [ "$KIT" = "$LOCKED" ]; then
  echo "GPUI is current: gpui-kit $KIT, gpui-pre-linux $LINUX (patched)."
  exit 0
fi
echo "gpui-kit $LOCKED -> $KIT, gpui-pre-linux $VENDORED -> $LINUX"
# Crake's requirement follows a new minor version (0.7 -> 0.8); Cargo.lock
# moves to that gpui-kit; the patched copy must be what's built.
use_kit() {
  MINOR=$(echo "$KIT" | cut -d. -f1-2)
  sed -i -E "s/^(gpui-kit = (\\{ version = )?)\"[0-9]+\\.[0-9]+\"/\\1\"$MINOR\"/" crates/crake/Cargo.toml
  cargo update -q -p gpui-kit --precise "$KIT"
  scripts/check-gpui-patch.sh
}
if [ "$LINUX" = "$VENDORED" ]; then
  # The same GPUI: the vendored, patched copy stays.
  use_kit
  echo "Updated: gpui-kit $KIT; gpui-pre-linux $LINUX, patched, as before."
  exit 0
fi

WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-gpui.XXXXXX")
trap 'rm -rf "$WORK"' EXIT
api "gpui-pre-linux/$LINUX/download" | tar xz -C "$WORK"
NEW=$WORK/gpui-pre-linux-$LINUX
# Upstream took Crake's change: the patch applies backwards. No vendored
# copy needed any more: the [patch] and vendor/ go.
if (cd "$NEW" && patch -p1 -R --dry-run -s < "$OLDPWD/vendor/gpui-pre-linux.patch") >/dev/null 2>&1; then
  rm -rf vendor/gpui-pre-linux vendor/gpui-pre-linux.patch
  sed -i '/^\[patch.crates-io\]$/,/^$/d' Cargo.toml
  use_kit
  echo "Upstream has Crake's change in gpui-pre-linux $LINUX: the vendored copy and its [patch] are gone."
  exit 0
fi
if ! (cd "$NEW" && patch -p1 --forward --no-backup-if-mismatch -s < "$OLDPWD/vendor/gpui-pre-linux.patch"); then
  echo "Crake's patch doesn't apply to gpui-pre-linux $LINUX: re-apply it by hand" >&2
  echo "(vendor/gpui-pre-linux/CRAKE-PATCH.md says what it does), then regenerate" >&2
  echo "vendor/gpui-pre-linux.patch against the pristine source." >&2
  exit 2
fi
rm -f "$NEW/.cargo_vcs_info.json" "$NEW/.cargo-ok"
cp vendor/gpui-pre-linux/CRAKE-PATCH.md "$NEW/"
sed -i "1s/.*/# gpui-pre-linux $LINUX, patched for Crake/" "$NEW/CRAKE-PATCH.md"
rm -rf vendor/gpui-pre-linux
cp -r "$NEW" vendor/gpui-pre-linux
use_kit
echo "Updated: gpui-kit $KIT with gpui-pre-linux $LINUX, Crake's patch applied."
