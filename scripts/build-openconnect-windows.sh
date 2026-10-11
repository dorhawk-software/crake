#!/bin/sh
# openconnect for Windows, cross-built in a Fedora container with MinGW (as
# openconnect's own Windows builds are): openconnect.exe and the DLLs it
# needs in target/windows/openconnect/, with Wintun's DLL (wintun.net, its
# prebuilt-binaries licence: shipped unmodified) beside them. For the VM tests now, the installer later
# (the Windows plan in CLAUDE.md, phase 5). openconnect is LGPL-2.1.
set -eu
cd "$(dirname "$0")/.."
VERSION=${OPENCONNECT_VERSION:-9.12}
OUT=$PWD/target/windows/openconnect
mkdir -p "$OUT"
docker run --rm -v "$OUT:/out" -e VERSION="$VERSION" -e HOST_UID="$(id -u)" fedora:44 sh -euc '
  dnf install -y -q mingw64-gcc mingw64-gnutls mingw64-libxml2 mingw64-zlib \
    mingw64-gettext mingw64-winpthreads-static make pkgconf curl xz tar gettext unzip >/dev/null
  cd /tmp
  curl -fsSL -o wintun.zip https://www.wintun.net/builds/wintun-0.14.1.zip
  unzip -q wintun.zip
  curl -fsSL "https://www.infradead.org/openconnect/download/openconnect-$VERSION.tar.gz" | tar xz
  cd "openconnect-$VERSION"
  cp /tmp/wintun/include/wintun.h .
  # openconnect 9.12 predates GCC 14, which made these errors.
  LENIENT="-Wno-error=incompatible-pointer-types -Wno-error=int-conversion -Wno-error=implicit-function-declaration"
  mingw64-configure CFLAGS="$(rpm --eval %{mingw64_cflags}) $LENIENT" --disable-nls --without-libpskc --without-stoken --without-libproxy --without-lz4 \
    --with-vpnc-script=crake-hook.js >/dev/null
  make -j"$(nproc)" >/dev/null
  cp .libs/openconnect.exe .libs/libopenconnect-5.dll /tmp/wintun/bin/amd64/wintun.dll /out/
  # The DLLs it needs from MinGW, followed through their own needs.
  SYS=/usr/x86_64-w64-mingw32/sys-root/mingw/bin
  todo="/out/openconnect.exe /out/libopenconnect-5.dll"
  while [ -n "$todo" ]; do
    next=
    for f in $todo; do
      for dll in $(x86_64-w64-mingw32-objdump -p "$f" | sed -n "s/.*DLL Name: //p"); do
        if [ -f "$SYS/$dll" ] && [ ! -f "/out/$dll" ]; then
          cp "$SYS/$dll" /out/
          next="$next /out/$dll"
        fi
      done
    done
    todo=$next
  done
  chown -R "$HOST_UID" /out'
ls "$OUT"
