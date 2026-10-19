#!/bin/sh
# A FortiGate-VM (Fortinet's own, the free permanent trial licence) to try
# Crake against a real Fortinet gateway (CLAUDE.md, 7f, 7j). The images are
# the user's downloads from FortiCloud (FortiGate, KVM), unpacked as
# target/vm/fortigate/<version>/fortios.qcow2 and never changed: each
# version runs on an overlay of its own. 7.4: SSL-VPN, the tunnel mode
# openconnect speaks (7.6.3 dropped it); 8.0: IPsec, for FortiClient.
#
#   scripts/fortigate-vm.sh console [VERSION]   boot it (default 8.0), its
#                                     console in this terminal (the first
#                                     boot: password, licence); leave with
#                                     `execute shutdown`
#
# Its one network card is qemu's user network: out to the internet (the
# licence is fetched from FortiCare once), and in only on this computer's
# loopback: 127.0.0.1:10443 its SSL-VPN, :10444 its admin page, :10022 SSH.
set -eu
cd "$(dirname "$0")/.."
TOP=target/vm/fortigate
VERSION=${2:-8.0}
DIR=$TOP/$VERSION
[ -f "$DIR/fortios.qcow2" ] || { echo "no $DIR/fortios.qcow2: unpack the FortiGate $VERSION KVM image there" >&2; exit 1; }
# The licence allows one CPU and 2 GB.
MEMORY=2048
CPUS=1
LOG_DISK=4G
# FortiGate makes its serial from the VM's SMBIOS UUID, which qemu leaves
# all zeros unless told: the serial was then FGVMEV0000000000 and FortiCare
# answered "Invalid serial number" (2026-10-19). The licence is bound to
# the unit it was asked for, one per account, so the UUID is made once and
# every version boots as that same unit.
[ -f "$TOP/uuid" ] || cat /proc/sys/kernel/random/uuid > "$TOP/uuid"
UUID=$(cat "$TOP/uuid")
NET=user,id=wan,hostfwd=tcp:127.0.0.1:10443-:10443,hostfwd=tcp:127.0.0.1:10444-:443,hostfwd=tcp:127.0.0.1:10022-:22
# port2: a cable to the client VM only (a socket file, no network of this
# computer's), the client's side of the gateway at 192.0.2.1.
LAN=$TOP/lan.sock
case "${1:-}" in
  console)
    # Two at once would share the ports and be the same unit.
    if pgrep -f "qemu-system-x86_64.*-uuid $UUID" >/dev/null; then
      echo "a FortiGate is already running: \`execute shutdown\` in its console first" >&2
      exit 1
    fi
    [ -f "$DIR/fortigate.qcow2" ] ||
      qemu-img create -q -f qcow2 -b fortios.qcow2 -F qcow2 "$DIR/fortigate.qcow2"
    [ -f "$DIR/logs.qcow2" ] || qemu-img create -q -f qcow2 "$DIR/logs.qcow2" "$LOG_DISK"
    rm -f "$LAN"
    exec qemu-system-x86_64 -machine q35,accel=kvm -cpu host -m "$MEMORY" -smp "$CPUS" \
      -uuid "$UUID" -smbios "type=1,uuid=$UUID" \
      -drive "file=$DIR/fortigate.qcow2,if=virtio" -drive "file=$DIR/logs.qcow2,if=virtio" \
      -netdev "$NET" -device virtio-net-pci,netdev=wan \
      -netdev "stream,id=lan,server=on,addr.type=unix,addr.path=$LAN" \
      -device virtio-net-pci,netdev=lan \
      -nographic
    ;;
  client)
    # A throwaway Ubuntu VM on the other end of port2's cable: strongSwan
    # from Ubuntu's archive (its own user network, for apt only) connects
    # to the FortiGate over IKEv2 and tries 10.40.0.1, which answers only
    # through the tunnel (scripts/fortigate-client/run.sh). Nothing stays.
    [ -S "$LAN" ] || { echo "start the FortiGate first: $0 console" >&2; exit 1; }
    REL=26.04
    IMG=target/vm/ubuntu-$REL.img
    [ -f "$IMG" ] || curl -fsSL -o "$IMG" "https://cloud-images.ubuntu.com/releases/$REL/release/ubuntu-$REL-server-cloudimg-amd64.img"
    WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-fortigate.XXXXXX")
    trap 'rm -rf "$WORK"' EXIT
    qemu-img create -q -f qcow2 -b "$PWD/$IMG" -F qcow2 "$WORK/disk.qcow2" 8G
    mkdir -p "$WORK/seed" "$WORK/test"
    printf 'instance-id: crake-fortigate-client\nlocal-hostname: crake-client\n' > "$WORK/seed/meta-data"
    cat > "$WORK/seed/user-data" <<'CLOUD'
#cloud-config
package_update: false
runcmd:
  - [sh, -c, "mkdir -p /test && mount -o ro LABEL=CRAKETEST /test && sh /test/run.sh > /dev/ttyS1 2>&1; poweroff"]
CLOUD
    xorriso -as mkisofs -quiet -output "$WORK/seed.iso" -volid cidata -joliet -rock "$WORK/seed"
    cp scripts/fortigate-client/* "$WORK/test/"
    xorriso -as mkisofs -quiet -output "$WORK/test.iso" -volid CRAKETEST -joliet -rock "$WORK/test"
    echo "== the client VM (a few minutes)"
    timeout 900 qemu-system-x86_64 -machine q35,accel=kvm -cpu host -m 1024 -smp 2 \
      -display none -monitor none -no-reboot \
      -drive "file=$WORK/disk.qcow2,if=virtio" \
      -drive "file=$WORK/seed.iso,if=virtio,format=raw,readonly=on" \
      -drive "file=$WORK/test.iso,if=virtio,format=raw,readonly=on" \
      -nic user,model=virtio-net-pci \
      -netdev "stream,id=lan,server=off,addr.type=unix,addr.path=$LAN" \
      -device virtio-net-pci,netdev=lan,mac=52:54:00:c0:02:0a \
      -serial "file:$WORK/console.log" -serial "file:$WORK/test.log" || true
    cat "$WORK/test.log"
    grep -q '^CLIENT-RESULT: 0' "$WORK/test.log" || {
      echo "(the VM's console, last lines:)"; tail -n 20 "$WORK/console.log"; exit 1; }
    ;;
  *)
    echo "usage: $0 console [VERSION] | client" >&2
    exit 2
    ;;
esac
