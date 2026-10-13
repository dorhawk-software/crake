#!/bin/sh
# Crake for macOS as an installer package, on a Mac (CI job macos):
# Crake.app in /Applications (the menu bar app), craked as a launchd daemon
# (/usr/local/sbin, /Library/LaunchDaemons), the hook and the uninstaller in
# /usr/local/libexec/crake (the system's /usr is sealed). Apple Silicon only
# for now. openconnect isn't bundled: Homebrew's is used.
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
BIN=target/$PROFILE
[ "$PROFILE" != dev ] || BIN=target/debug
cargo build --profile "$PROFILE" --locked -p crake -p craked
rm -rf "${OUT:?}"
ROOT=$OUT/root
APP=$ROOT/Applications/Crake.app
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$ROOT/usr/local/sbin" \
  "$ROOT/usr/local/libexec/crake" "$ROOT/Library/LaunchDaemons"
cp "$BIN/crake" "$APP/Contents/MacOS/crake"
cp assets/icons/app/crake.icns "$APP/Contents/Resources/Crake.icns"
sed "s/@VERSION@/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
cp "$BIN/craked" "$ROOT/usr/local/sbin/craked"
cp "$BIN/crake-hook" "$ROOT/usr/local/libexec/crake/hook"
if [ -n "${CRAKE_KEYCHAIN:-}" ]; then
  sign() {
    codesign --force --options runtime --timestamp --keychain "$CRAKE_KEYCHAIN" \
      --sign "Developer ID Application" "$@"
  }
  sign --identifier software.dorhawk.craked "$ROOT/usr/local/sbin/craked"
  sign --identifier software.dorhawk.crake-hook "$ROOT/usr/local/libexec/crake/hook"
  sign "$APP"
  set -- --sign "Developer ID Installer" --keychain "$CRAKE_KEYCHAIN" --timestamp
else
  codesign --force --sign - "$APP"
  set --
fi
cp packaging/macos/uninstall.sh "$ROOT/usr/local/libexec/crake/uninstall"
cp packaging/macos/software.dorhawk.craked.plist "$ROOT/Library/LaunchDaemons/"
# Installed where it says, never "relocated" to another copy of Crake.app
# macOS happens to know of.
pkgbuild --analyze --root "$ROOT" "$OUT/components.plist"
plutil -replace 0.BundleIsRelocatable -bool NO "$OUT/components.plist"
pkgbuild --root "$ROOT" --component-plist "$OUT/components.plist" \
  --scripts packaging/macos/scripts --identifier software.dorhawk.crake \
  --version "$VERSION" --install-location / "$OUT/crake.pkg"
sed "s/@VERSION@/$VERSION/g" packaging/macos/distribution.xml > "$OUT/distribution.xml"
productbuild --distribution "$OUT/distribution.xml" --resources packaging/macos/resources \
  --package-path "$OUT" "$@" "$OUT/Crake-$VERSION.pkg"
echo "$OUT/Crake-$VERSION.pkg"
