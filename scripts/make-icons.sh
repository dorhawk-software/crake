#!/bin/sh
# Crake's app icon (assets/icons/app/crake.svg: the bird in the grass, as
# on dorhawk.software/crake) rendered for the icon themes and Windows:
# crake-<size>.png for hicolor, crake.ico for the .exe files. Run after
# changing the SVG; the results are committed. In a throwaway container.
set -eu
cd "$(dirname "$0")/.."
docker run --rm -v "$PWD/assets/icons/app:/icons" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" debian:13 sh -euc '
  apt-get update -qq >/dev/null && apt-get install -y -qq librsvg2-bin icoutils >/dev/null
  cd /icons
  for s in 16 24 32 48 64 128 256 512; do
    rsvg-convert -w $s -h $s crake.svg -o crake-$s.png
  done
  icotool -c -o crake.ico crake-16.png crake-24.png crake-32.png crake-48.png crake-64.png -r crake-256.png
  chown "$HOST_UID:$HOST_GID" crake-*.png crake.ico'
ls -la assets/icons/app
