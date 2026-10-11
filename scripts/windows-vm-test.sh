#!/bin/sh
# Crake on Windows, end to end, in throwaway VMs (the Windows plan in
# CLAUDE.md, phase 4): a Windows 11 VM with Crake installed as a person
# would have it (the craked service, openconnect beside it), and a gateway
# VM (Ubuntu, ocserv, ACME's network behind it). Their only link is a
# socket file in a temporary folder: no network to this machine or beyond,
# except the gateway's user-mode card for Ubuntu's archive. The Windows
# test itself is crates/crake/src/windows_tests.rs, run inside Windows as
# the app's side; scripts/windows-vm-test/run.ps1 installs and checks
# around it. qemu with KVM, no display, no ports.
#
#   scripts/windows-vm-test.sh
#
# Once, cached in target/vm/: Windows 11 Enterprise's evaluation ISO
# (Microsoft's, about 8 GB) and a base image installed from it with no one
# at the keyboard (scripts/windows-vm-test/autounattend.xml, half an hour
# or so; the evaluation runs 90 days, delete target/vm/windows-11.qcow2 to
# install again). Every run gets temporary overlays of both images.
# Not in GitHub's CI: it needs KVM and the base image.
#
# The scripts run inside docker containers are single-quoted: no
# apostrophes in them.
set -eu
cd "$(dirname "$0")/.."
ISO=target/vm/windows-11-eval.iso
ISO_URL="https://go.microsoft.com/fwlink/?linkid=2382600&clcid=0x409&culture=en-us&country=us"
BASE=target/vm/windows-11.qcow2
GW_IMG=target/vm/ubuntu-26.04.img
GW_URL=https://cloud-images.ubuntu.com/releases/26.04/release/ubuntu-26.04-server-cloudimg-amd64.img
OVMF=/usr/share/OVMF
[ -e /dev/kvm ] || { echo "no /dev/kvm" >&2; exit 1; }
[ -f $OVMF/OVMF_CODE_4M.fd ] || { echo "no OVMF: sudo apt install ovmf" >&2; exit 1; }
WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-winvm.XXXXXX")
cleanup() {
  [ -f "$WORK/gateway.pid" ] && kill "$(cat "$WORK/gateway.pid")" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

# Press a key every half second for a while (Windows' disc asks for one
# before it boots), on qemu's monitor socket $1.
press_keys() {
  python3 -c '
import socket, sys, time
for _ in range(60):
    try:
        s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); break
    except OSError:
        time.sleep(0.5)
for _ in range(int(sys.argv[2])):
    s.sendall(b"sendkey ret\n"); time.sleep(0.5)
' "$1" "$2" >/dev/null 2>&1 || true
}

# --- The base image, once ---------------------------------------------------
if [ ! -f "$BASE" ]; then
  mkdir -p target/vm
  if [ ! -f "$ISO" ]; then
    echo "== Windows 11 evaluation ISO (about 8 GB, once)"
    curl -fsSL -o "$ISO.part" "$ISO_URL"
    mv "$ISO.part" "$ISO"
  fi
  echo "== Installing Windows into the base image (unattended, half an hour or so)"
  mkdir -p "$WORK/answer"
  cp scripts/windows-vm-test/autounattend.xml scripts/windows-vm-test/launch.ps1 "$WORK/answer/"
  xorriso -as mkisofs -quiet -output "$WORK/answer.iso" -volid ANSWER -joliet -rock "$WORK/answer"
  qemu-img create -q -f qcow2 "$BASE.part" 64G
  cp $OVMF/OVMF_VARS_4M.fd target/vm/windows-11.vars
  press_keys "$WORK/monitor.sock" 40 &
  timeout 5400 qemu-system-x86_64 -machine q35,accel=kvm -cpu host -m 4096 -smp 4 \
    -display none -nic none \
    -drive "if=pflash,format=raw,readonly=on,file=$OVMF/OVMF_CODE_4M.fd" \
    -drive "if=pflash,format=raw,file=target/vm/windows-11.vars" \
    -drive "file=$BASE.part,if=none,id=disk" -device ide-hd,drive=disk,bus=ide.0,bootindex=1 \
    -drive "file=$ISO,if=none,id=setup,media=cdrom,readonly=on" -device ide-cd,drive=setup,bus=ide.1,bootindex=0 \
    -drive "file=$WORK/answer.iso,if=none,id=answer,media=cdrom,readonly=on" -device ide-cd,drive=answer,bus=ide.2 \
    -monitor "unix:$WORK/monitor.sock,server=on,wait=off" \
    -serial "file:$WORK/install.log"
  mv "$BASE.part" "$BASE"
  echo "   base image ready: $BASE"
fi

# --- Crake for Windows ------------------------------------------------------
echo "== Building Crake for Windows (MinGW, in a container)"
[ -x target/windows/openconnect/openconnect.exe ] || scripts/build-openconnect-windows.sh
OUT=$PWD/target/windows/vm
mkdir -p "$OUT"
docker build -q -t crake-windows-build scripts/windows >/dev/null
docker run --rm -v "$PWD:/src:ro" -v "$OUT:/out" -v crake-win-cargo:/usr/local/cargo/registry \
  -v crake-win-target:/target -e CARGO_TARGET_DIR=/target -e HOST_UID="$(id -u)" \
  -e CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
  crake-windows-build sh -euc '
    cd /src
    T=x86_64-pc-windows-gnu
    cargo build -q --locked --release --target $T -p craked
    cp /target/$T/release/craked.exe /target/$T/release/crake-hook.exe /out/
    TEST=$(cargo test -q --locked --target $T -p crake --no-run --message-format json 2>/dev/null |
      sed -n "s/.*\"executable\":\"\([^\"]*crake-[0-9a-f]*\.exe\)\".*/\1/p" | tail -n1)
    x86_64-w64-mingw32-strip -o /out/crake-tests.exe "$TEST"
    chown "$HOST_UID" /out/*.exe'

# --- The test disc ----------------------------------------------------------
mkdir -p "$WORK/test/crake/openconnect" "$WORK/test/e2e"
cp "$OUT/craked.exe" "$OUT/crake-hook.exe" packaging/windows/crake-hook.js "$WORK/test/crake/"
cp target/windows/openconnect/* "$WORK/test/crake/openconnect/"
cp "$OUT/crake-tests.exe" scripts/windows-vm-test/run.ps1 "$WORK/test/"
cp scripts/e2e/ocserv.conf "$WORK/test/e2e/"
cp scripts/windows-vm-test/gateway.sh "$WORK/test/"
xorriso -as mkisofs -quiet -output "$WORK/test.iso" -volid CRAKETEST -joliet -rock "$WORK/test"

# --- The gateway VM ---------------------------------------------------------
if [ ! -f "$GW_IMG" ]; then
  curl -fsSL -o "$GW_IMG.part" "$GW_URL"
  mv "$GW_IMG.part" "$GW_IMG"
fi
qemu-img create -q -f qcow2 -b "$PWD/$GW_IMG" -F qcow2 "$WORK/gateway.qcow2" 8G
mkdir -p "$WORK/seed"
printf 'instance-id: crake-winvm-gateway\nlocal-hostname: gateway\n' > "$WORK/seed/meta-data"
cat > "$WORK/seed/user-data" <<'CLOUD'
#cloud-config
package_update: false
runcmd:
  - [sh, -c, "mkdir -p /test && mount -o ro LABEL=CRAKETEST /test && sh /test/gateway.sh > /dev/ttyS2 2>&1 || echo GATEWAY-FAILED > /dev/ttyS1"]
CLOUD
# Ubuntu's archive on the first card only; the LAN is gateway.sh's.
cat > "$WORK/seed/network-config" <<'NET'
version: 2
ethernets:
  archive:
    match: {macaddress: "52:54:00:c0:ff:01"}
    dhcp4: true
NET
xorriso -as mkisofs -quiet -output "$WORK/seed.iso" -volid cidata -joliet -rock "$WORK/seed"
echo "== Gateway VM (Ubuntu 26.04, ocserv)"
qemu-system-x86_64 -machine q35,accel=kvm -cpu host -m 1024 -smp 2 \
  -display none -monitor none -daemonize -pidfile "$WORK/gateway.pid" \
  -drive "file=$WORK/gateway.qcow2,if=virtio" \
  -drive "file=$WORK/seed.iso,if=virtio,format=raw,readonly=on" \
  -drive "file=$WORK/test.iso,if=virtio,format=raw,readonly=on" \
  -nic user,model=virtio-net-pci,mac=52:54:00:c0:ff:01 \
  -netdev "stream,id=lan,server=on,addr.type=unix,addr.path=$WORK/lan.sock" \
  -device virtio-net-pci,netdev=lan,mac=52:54:00:c0:ff:10 \
  -serial "file:$WORK/gateway-console.log" -serial "file:$WORK/gateway.log" \
  -serial "file:$WORK/gateway-setup.log"

# --- The Windows VM ---------------------------------------------------------
echo "== Windows VM (Crake installed, then the test; ten minutes or so)"
qemu-img create -q -f qcow2 -b "$PWD/$BASE" -F qcow2 "$WORK/windows.qcow2"
cp target/vm/windows-11.vars "$WORK/windows.vars"
timeout 2400 qemu-system-x86_64 -machine q35,accel=kvm -cpu host -m 4096 -smp 4 \
  -display none -no-reboot \
  -drive "if=pflash,format=raw,readonly=on,file=$OVMF/OVMF_CODE_4M.fd" \
  -drive "if=pflash,format=raw,file=$WORK/windows.vars" \
  -drive "file=$WORK/windows.qcow2,if=none,id=disk" -device ide-hd,drive=disk,bus=ide.0,bootindex=0 \
  -drive "file=$WORK/test.iso,if=none,id=test,media=cdrom,readonly=on" -device ide-cd,drive=test,bus=ide.1 \
  -netdev "stream,id=lan,server=off,addr.type=unix,addr.path=$WORK/lan.sock" \
  -device e1000e,netdev=lan,mac=52:54:00:c0:ff:20 \
  -monitor none -serial "file:$WORK/test.log" || true

mkdir -p target/vm
cp "$WORK/test.log" target/vm/last-windows-test.log
cat "$WORK/test.log"
grep -q '^VM-TEST-RESULT: 0' "$WORK/test.log" || {
  echo "(the gateway: $(cat "$WORK/gateway.log" 2>/dev/null); its setup, last lines:)"
  tail -n 20 "$WORK/gateway-setup.log" 2>/dev/null
  exit 1
}
