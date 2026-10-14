#!/bin/sh
# A release must not rebuild Nix's dependency builds (crane's crake-deps and
# crake-journey-deps: ~45 min in CI's nixos job). They mustn't depend on
# Crake's version: this evaluates them (builds nothing) for this tree and for
# the same tree with another version, and fails if they differ.
# Without Nix here, the evaluations run in the nixos/nix container (store in
# the crake-nix volume, as scripts/nix-test.sh).
set -eu
cd "$(dirname "$0")/.."
TMP=$(mktemp -d "${TMPDIR:-/tmp}/crake-nix-deps.XXXXXX")
trap 'rm -rf "$TMP"' EXIT
mkdir "$TMP/now" "$TMP/next"
git ls-files -z --cached --others --exclude-standard | xargs -0 tar -cf - | tar -xf - -C "$TMP/now"
git ls-files -z --cached --others --exclude-standard | xargs -0 tar -cf - | tar -xf - -C "$TMP/next"
# The other version, where a release puts it (scripts/release.sh): the
# workspace's, Cargo.lock's entries for Crake's own crates (those without a
# source), packaging/nix/package.nix's.
OTHER=99.0.0
cd "$TMP/next"
NOW=$(sed -n '/^\[workspace.package\]/,/^\[/ s/^version = "\(.*\)"/\1/p' Cargo.toml)
# Crake's own crates are the entries without a source, at its version.
awk -v from="$NOW" -v to="$OTHER" '
  function flush(  i, own) {
    own = 1
    for (i = 1; i <= n; i++) if (block[i] ~ /^source = /) own = 0
    for (i = 1; i <= n; i++) {
      if (own && block[i] == "version = \"" from "\"") block[i] = "version = \"" to "\""
      print block[i]
    }
    n = 0
  }
  /^\[\[package\]\]/ { flush() }
  { block[++n] = $0 }
  END { flush() }
' Cargo.lock > Cargo.lock.new
mv Cargo.lock.new Cargo.lock
sed -i "/^\[workspace.package\]/,/^\[/ s/^version = \".*\"/version = \"$OTHER\"/" Cargo.toml
sed -i "s/^    version = \".*\";/    version = \"$OTHER\";/" packaging/nix/package.nix
cd - >/dev/null
# Each of Crake's own crates moved; the vendored GPUI crate didn't.
[ "$(grep -c "^version = \"$OTHER\"" "$TMP/next/Cargo.lock")" -ge 3 ]
grep -A1 '^name = "gpui-pre-linux"' "$TMP/next/Cargo.lock" | grep -qv "$OTHER"
drv() {
  if command -v nix >/dev/null; then
    NIX_CONFIG="experimental-features = nix-command flakes" \
      nix eval --raw "path:$1#packages.x86_64-linux.$2.drvPath"
  else
    docker run --rm -v "$TMP:$TMP:ro" -v crake-nix:/nix nixos/nix \
      nix --extra-experimental-features "nix-command flakes" \
      eval --raw "path:$1#packages.x86_64-linux.$2.drvPath"
  fi
}
fail=0
for deps in default.cargoArtifacts journey.cargoArtifacts; do
  a=$(drv "$TMP/now" "$deps")
  b=$(drv "$TMP/next" "$deps")
  if [ "$a" = "$b" ]; then
    echo "$deps: the same at version $OTHER"
  else
    echo "$deps: rebuilt for another version ($a, then $b)"
    fail=1
  fi
done
# Crake's own build does carry the version.
[ "$(drv "$TMP/now" default)" != "$(drv "$TMP/next" default)" ] || {
  echo "Crake's own build doesn't follow the version"
  fail=1
}
exit $fail
