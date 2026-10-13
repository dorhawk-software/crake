#!/bin/sh
# Signing and notarizing Crake for macOS on a CI runner, with the user's
# Developer ID (GitHub secrets; CLAUDE.md, macOS):
#   setup          a keychain of its own holding both certificates, from
#                  MACOS_APP_P12 / MACOS_INSTALLER_P12 (base64 .p12) and
#                  their passwords; prints the keychain's path (for
#                  CRAKE_KEYCHAIN, scripts/build-macos-pkg.sh)
#   notarize <pkg> Apple's notary service checks the package (App Store
#                  Connect API key: APPLE_API_KEY, APPLE_API_KEY_ID,
#                  APPLE_API_ISSUER_ID); its ticket is stapled to the
#                  package, which Gatekeeper must then accept
# Nothing secret is printed.
set -eu
case ${1:-} in
setup)
  dir=$(mktemp -d)
  keychain=$dir/crake-signing.keychain-db
  pass=$(openssl rand -hex 16)
  security create-keychain -p "$pass" "$keychain"
  security set-keychain-settings -lut 3600 "$keychain"
  security unlock-keychain -p "$pass" "$keychain"
  for kind in APP INSTALLER; do
    eval "p12=\$MACOS_${kind}_P12 p12pass=\$MACOS_${kind}_P12_PASSWORD"
    printf '%s' "$p12" | base64 --decode > "$dir/$kind.p12"
    security import "$dir/$kind.p12" -k "$keychain" -P "$p12pass" \
      -T /usr/bin/codesign -T /usr/bin/productbuild -T /usr/bin/productsign >/dev/null
    rm -f "$dir/$kind.p12"
  done
  # codesign and productbuild may use the keys without asking.
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$pass" "$keychain" >/dev/null
  # Searched besides the login keychain (codesign's chain needs Apple's
  # intermediate, which macOS has in the System keychain).
  security list-keychains -d user -s "$keychain" $(security list-keychains -d user | tr -d '"')
  security find-identity -v "$keychain" | grep -q "Developer ID Application" || {
    echo "the Developer ID Application certificate didn't import" >&2; exit 1; }
  echo "$keychain"
  ;;
notarize)
  pkg=$2
  key=$(mktemp)
  printf '%s\n' "$APPLE_API_KEY" > "$key"
  out=$(xcrun notarytool submit "$pkg" --key "$key" --key-id "$APPLE_API_KEY_ID" \
    --issuer "$APPLE_API_ISSUER_ID" --wait --timeout 30m --output-format json) || true
  echo "$out"
  id=$(printf '%s' "$out" | sed -n 's/.*"id":"\([^"]*\)".*/\1/p')
  if ! printf '%s' "$out" | grep -q '"status":"Accepted"'; then
    [ -z "$id" ] || xcrun notarytool log "$id" --key "$key" \
      --key-id "$APPLE_API_KEY_ID" --issuer "$APPLE_API_ISSUER_ID"
    rm -f "$key"
    echo "notarization failed" >&2
    exit 1
  fi
  rm -f "$key"
  xcrun stapler staple "$pkg"
  spctl --assess --type install --verbose=2 "$pkg"
  ;;
*)
  echo "usage: $0 setup | notarize <pkg>" >&2
  exit 2
  ;;
esac
