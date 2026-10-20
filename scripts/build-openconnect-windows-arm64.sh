#!/bin/sh
# openconnect for Windows on ARM64, as scripts/build-openconnect-windows.sh
# builds it for x64, but natively: in MSYS2's CLANGARM64 environment on a
# Windows ARM64 runner (CI's windows-arm64-openconnect job; Fedora's MinGW,
# which the x64 build uses, has no ARM64). The same openconnect, the same
# one change (scripts/windows/openconnect-unbuffered.c), Wintun's ARM64
# DLL beside it, and the DLLs it needs from MSYS2, in
# target/windows-arm64/openconnect/.
#
#   scripts/build-openconnect-windows-arm64.sh   (in an MSYS2 CLANGARM64 shell)
set -eu
cd "$(dirname "$0")/.."
REPO=$PWD
VERSION=${OPENCONNECT_VERSION:-9.12}
OUT=$REPO/target/windows-arm64/openconnect
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
rm -rf "$OUT"
mkdir -p "$OUT"
cd "$WORK"
curl -fsSL -o wintun.zip https://www.wintun.net/builds/wintun-0.14.1.zip
unzip -q wintun.zip
curl -fsSL "https://www.infradead.org/openconnect/download/openconnect-$VERSION.tar.gz" | tar xz
cd "openconnect-$VERSION"
cp ../wintun/include/wintun.h .
# The one change Crake makes (scripts/windows/openconnect-unbuffered.c).
cat "$REPO/scripts/windows/openconnect-unbuffered.c" >> main.c
# openconnect 9.12 predates today's compilers, which made these errors
# (in CPPFLAGS, so configure keeps its own CFLAGS).
LENIENT="-Wno-error=incompatible-pointer-types -Wno-error=int-conversion -Wno-error=implicit-function-declaration"
# compat.c includes sec_api/stdlib_s.h, which today's mingw-w64 dropped
# (its declarations are in stdlib.h now): a stand-in that includes that.
mkdir -p shim/sec_api
echo '#include <stdlib.h>' > shim/sec_api/stdlib_s.h
# openconnect's config.guess predates MSYS2's ARM64 environments: named.
./configure --build=aarch64-w64-mingw32 --host=aarch64-w64-mingw32 CC=clang CPPFLAGS="-I$PWD/shim $LENIENT" --disable-nls --without-libpskc --without-stoken --without-libproxy --without-lz4 \
  --with-vpnc-script=crake-hook.js >/dev/null
make -j"$(nproc)" >/dev/null
# libtool leaves a shared build in .libs and a static one beside the
# sources (natively in MSYS2 it linked statically; cross-built, shared).
if [ -f .libs/openconnect.exe ]; then
  cp .libs/openconnect.exe "$OUT/"
elif [ -f openconnect.exe ]; then
  cp openconnect.exe "$OUT/"
else
  ls -la . .libs >&2
  echo "no openconnect.exe built" >&2
  exit 1
fi
for dll in .libs/*.dll; do [ -f "$dll" ] && cp "$dll" "$OUT/"; done
cp ../wintun/bin/arm64/wintun.dll "$OUT/"
# Their licences, and where the source is (LGPL-2.1: shipped beside them).
cp COPYING.LGPL "$OUT/openconnect-LICENSE.txt"
cp ../wintun/LICENSE.txt "$OUT/wintun-LICENSE.txt"
printf "openconnect %s for ARM64, built by Crake from https://www.infradead.org/openconnect/download/openconnect-%s.tar.gz\nwith one change: https://github.com/dorhawk-software/crake/blob/main/scripts/windows/openconnect-unbuffered.c\nWith MSYS2's CLANGARM64 libraries (https://packages.msys2.org).\nWintun 0.14.1 (https://www.wintun.net), unmodified.\n" "$VERSION" "$VERSION" > "$OUT/SOURCE.txt"
# The DLLs it needs from MSYS2, followed through their own needs.
SYS=$MINGW_PREFIX/bin
todo=$(ls "$OUT"/*.exe "$OUT"/libopenconnect*.dll 2>/dev/null)
while [ -n "$todo" ]; do
  next=
  for f in $todo; do
    for dll in $(llvm-objdump -p "$f" | sed -n "s/.*DLL Name: //p"); do
      if [ -f "$SYS/$dll" ] && [ ! -f "$OUT/$dll" ]; then
        cp "$SYS/$dll" "$OUT/"
        next="$next $OUT/$dll"
      fi
    done
  done
  todo=$next
done
ls "$OUT"
