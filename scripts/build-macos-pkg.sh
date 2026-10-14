#!/bin/sh
# Crake for macOS as an installer package, on a Mac (CI job macos):
# Crake.app in /Applications (the menu bar app), with craked and the
# launchd files the app registers itself (SMAppService: craked's in
# Contents/Library/LaunchDaemons, the login agent's in .../LaunchAgents),
# so System Settings lists them under Crake; the hook and the uninstaller
# in /usr/local/libexec/crake (the system's /usr is sealed). openconnect isn't
# bundled: Homebrew's is used. CRAKE_UNIVERSAL=1 builds for Apple Silicon
# and Intel and joins each binary into one (lipo; releases do); otherwise
# for this Mac's own architecture only (CI, quicker).
# Signed with the Developer ID when a keychain holds it
# (scripts/macos-signing.sh setup; CRAKE_KEYCHAIN names that keychain):
# every binary with the hardened runtime and a secure timestamp, as
# notarization requires, and the package with the installer certificate.
# Without one, ad hoc (as the linker signs every binary on Apple Silicon).
# Notarizing is a step of its own (scripts/macos-signing.sh notarize).
# Writes target/macos/Crake-<version>.pkg. CRAKE_PROFILE=dev packs the
# development build instead (CI: the one its tests just built, so the
# package costs a minute, not a second, optimised build of everything).
set -eu
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
OUT=target/macos
PROFILE=${CRAKE_PROFILE:-release}
DIR=$PROFILE
[ "$PROFILE" != dev ] || DIR=debug
ARCHS="arm64"
if [ -n "${CRAKE_UNIVERSAL:-}" ]; then
  ARCHS="arm64,x86_64"
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    cargo build --profile "$PROFILE" --locked --target "$t" -p crake -p craked
  done
  BIN=target/universal/$DIR
  mkdir -p "$BIN"
  for f in crake craked crake-hook; do
    lipo -create -output "$BIN/$f" \
      "target/aarch64-apple-darwin/$DIR/$f" "target/x86_64-apple-darwin/$DIR/$f"
  done
else
  cargo build --profile "$PROFILE" --locked -p crake -p craked
  BIN=target/$DIR
fi
rm -rf "${OUT:?}"
ROOT=$OUT/root
APP=$ROOT/Applications/Crake.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Library/LaunchDaemons" \
  "$APP/Contents/Library/LaunchAgents" "$ROOT/usr/local/libexec/crake"
cp "$BIN/crake" "$APP/Contents/MacOS/crake"
cp "$BIN/craked" "$APP/Contents/MacOS/craked"
cp packaging/macos/software.dorhawk.craked.plist "$APP/Contents/Library/LaunchDaemons/"
cp packaging/macos/software.dorhawk.crake.plist "$APP/Contents/Library/LaunchAgents/"
cp assets/icons/app/crake.icns "$APP/Contents/Resources/Crake.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
cp "$BIN/crake-hook" "$ROOT/usr/local/libexec/crake/hook"
if [ -n "${CRAKE_KEYCHAIN:-}" ]; then
  sign() {
    codesign --force --options runtime --timestamp --keychain "$CRAKE_KEYCHAIN" \
      --sign "Developer ID Application" "$@"
  }
  # Inside out: craked first, then the app, whose signature seals it.
  sign --identifier software.dorhawk.craked "$APP/Contents/MacOS/craked"
  sign --identifier software.dorhawk.crake-hook "$ROOT/usr/local/libexec/crake/hook"
  sign "$APP"
  set -- --sign "Developer ID Installer" --keychain "$CRAKE_KEYCHAIN" --timestamp
else
  codesign --force --sign - --identifier software.dorhawk.craked "$APP/Contents/MacOS/craked"
  codesign --force --sign - "$APP"
  set --
fi
cp packaging/macos/uninstall.sh "$ROOT/usr/local/libexec/crake/uninstall"
# Installed where it says, never "relocated" to another copy of Crake.app
# macOS happens to know of.
pkgbuild --analyze --root "$ROOT" "$OUT/components.plist"
plutil -replace 0.BundleIsRelocatable -bool NO "$OUT/components.plist"
pkgbuild --root "$ROOT" --component-plist "$OUT/components.plist" \
  --scripts packaging/macos/scripts --identifier software.dorhawk.crake \
  --version "$VERSION" --install-location / "$OUT/crake.pkg"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@ARCHS@/$ARCHS/g" packaging/macos/distribution.xml > "$OUT/distribution.xml"
productbuild --distribution "$OUT/distribution.xml" --resources packaging/macos/resources \
  --package-path "$OUT" "$@" "$OUT/Crake-$VERSION.pkg"
echo "$OUT/Crake-$VERSION.pkg"
