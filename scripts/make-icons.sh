#!/bin/sh
# Crake's app icon (assets/icons/app/crake.svg: the bird in the grass, as
# on dorhawk.software/crake) rendered for the icon themes, Windows and
# macOS: crake-<size>.png for hicolor, crake.ico for the .exe files,
# crake.icns for Crake.app (the rounded square at 824 of macOS' 1024-point
# grid, the margin macOS' own icons keep). Run after
# changing the SVG; the results are committed. In a throwaway container.
set -eu
cd "$(dirname "$0")/.."
docker run --rm -v "$PWD/assets/icons/app:/icons" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" debian:13 sh -euc '
  apt-get update -qq >/dev/null && apt-get install -y -qq librsvg2-bin icoutils icnsutils >/dev/null
  cd /icons
  for s in 16 24 32 48 64 128 256 512; do
    rsvg-convert -w $s -h $s crake.svg -o crake-$s.png
  done
  icotool -c -o crake.ico crake-16.png crake-24.png crake-32.png crake-48.png crake-64.png -r crake-256.png
  mac=$(mktemp -d)
  # The same drawing, nested at 824 points with a 100-point margin.
  inner=$(sed -e "s/^<svg[^>]*>//" -e "s#</svg>\$##" crake.svg)
  printf "%s" "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1024\" height=\"1024\"><svg x=\"100\" y=\"100\" width=\"824\" height=\"824\" viewBox=\"0 0 24 24\">$inner</svg></svg>" > $mac/mac.svg
  for s in 16 32 128 256 512 1024; do
    rsvg-convert -w $s -h $s $mac/mac.svg -o $mac/$s.png
  done
  png2icns crake.icns $mac/16.png $mac/32.png $mac/128.png $mac/256.png $mac/512.png $mac/1024.png >/dev/null
  chown "$HOST_UID:$HOST_GID" crake-*.png crake.ico crake.icns'
ls -la assets/icons/app
