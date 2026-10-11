#!/bin/sh
# Inside scripts/build-deb.sh's container: /src (read-only), /out.
set -eu
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -qq -y --no-install-recommends \
  ca-certificates curl build-essential pkg-config cmake \
  libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libfontconfig-dev libfreetype-dev dpkg-dev >/dev/null
[ -x /root/.cargo/bin/cargo ] || \
  curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable >/dev/null
. /root/.cargo/env
rustup update stable >/dev/null 2>&1 || true

# No incremental files: nothing here is rebuilt a line at a time, and they'd
# bloat the cache CI keeps.
export CARGO_INCREMENTAL=0
CARGO_TARGET_DIR=/target CARGO_PROFILE_RELEASE_STRIP=symbols \
  cargo build --release --locked --manifest-path /src/Cargo.toml -p crake -p craked

# The journey's test binary, built here like the packages so it runs on
# every system they're tested on (scripts/deb-test.sh, its journey phase).
# Without debug info: smaller and quicker, and it's never debugged here.
CARGO_TARGET_DIR=/target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --locked --manifest-path /src/Cargo.toml -p crake \
  --no-run --message-format=json 2>/dev/null \
  | sed -n 's/.*"executable":"\([^"]*\/deps\/crake-[^"]*\)".*/\1/p' | head -n1 > /tmp/journey-bin
cp "$(cat /tmp/journey-bin)" /out/journey && strip /out/journey && chown "$HOST_UID:$HOST_GID" /out/journey

root=$(mktemp -d)/crake
install -Dm755 /target/release/crake "$root/usr/bin/crake"
install -Dm755 /target/release/craked "$root/usr/sbin/craked"
install -Dm755 /target/release/crake-hook "$root/usr/libexec/crake/hook"
install -Dm644 /src/packaging/craked.service "$root/usr/lib/systemd/system/craked.service"
install -Dm644 /src/packaging/software.dorhawk.Crake1.conf "$root/usr/share/dbus-1/system.d/software.dorhawk.Crake1.conf"
install -Dm644 /src/packaging/software.dorhawk.crake.policy "$root/usr/share/polkit-1/actions/software.dorhawk.crake.policy"
install -Dm644 /src/packaging/deb/craked.toml "$root/etc/crake/craked.toml"
install -d "$root/etc/crake/managed.d"
install -Dm644 /src/packaging/apparmor/usr.sbin.craked "$root/etc/apparmor.d/usr.sbin.craked"
install -Dm644 /src/packaging/deb/crake.desktop "$root/usr/share/applications/software.dorhawk.crake.desktop"
# The app icon (assets/icons/app: the bird in the grass), for the icon themes.
install -Dm644 /src/assets/icons/app/crake.svg "$root/usr/share/icons/hicolor/scalable/apps/software.dorhawk.crake.svg"
for s in 16 24 32 48 64 128 256 512; do
  install -Dm644 "/src/assets/icons/app/crake-$s.png" "$root/usr/share/icons/hicolor/${s}x$s/apps/software.dorhawk.crake.png"
done
doc="$root/usr/share/doc/crake"
install -Dm644 /src/packaging/deb/copyright "$doc/copyright"
install -Dm644 /src/packaging/craked.toml.example "$doc/craked.toml.example"
install -m644 /src/assets/fonts/OFL-*.txt "$doc/"

# The C libraries the binaries link, as dpkg-shlibdeps finds them.
work=$(mktemp -d)
mkdir -p "$work/debian"
printf 'Source: crake\n\nPackage: crake\nArchitecture: amd64\n' > "$work/debian/control"
shlibs=$(cd "$work" && dpkg-shlibdeps -O "$root/usr/bin/crake" "$root/usr/sbin/craked" "$root/usr/libexec/crake/hook" \
  | sed -n 's/^shlibs:Depends=//p')

install -d "$root/DEBIAN"
sed -e "s/@VERSION@/$VERSION/" -e "s/@SHLIBS@/$shlibs/" /src/packaging/deb/control.in > "$root/DEBIAN/control"
echo "Installed-Size: $(du -sk --exclude=DEBIAN "$root" | cut -f1)" >> "$root/DEBIAN/control"
printf '/etc/crake/craked.toml\n/etc/apparmor.d/usr.sbin.craked\n' > "$root/DEBIAN/conffiles"
install -m755 /src/packaging/deb/postinst /src/packaging/deb/prerm /src/packaging/deb/postrm "$root/DEBIAN/"
# The same tree for the RPM (scripts/build-rpm.sh): one build, two packages.
rm -rf /out/root && cp -a "$root" /out/root && rm -rf /out/root/DEBIAN
chown -R "$HOST_UID:$HOST_GID" /out/root
deb="/out/crake_${VERSION}_amd64.deb"
dpkg-deb --root-owner-group -Zxz --build "$root" "$deb" >/dev/null
chown "$HOST_UID:$HOST_GID" "$deb"
dpkg-deb --info "$deb" | sed -n '/Package:/,$p'
