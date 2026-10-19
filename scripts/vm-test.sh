#!/bin/sh
# The package test (scripts/deb-test/) in a throwaway Ubuntu VM instead of a
# container: a kernel of its own, so craked's AppArmor profile is loaded and
# enforced there (a container would load it into this machine's kernel).
# qemu with KVM, no display, no ports: user-mode networking for apt only.
# The VM's disk is a temporary overlay of Ubuntu's cloud image (downloaded
# once into target/vm/); nothing else stays.
#
#   scripts/vm-test.sh [--complain] [release]     # default 26.04; or fedora:44
#                                                 # (the RPM, SELinux enforcing)
#
# --complain loads craked's profiles (its own, its charon's) in complain
# mode and lists what they would have refused.
set -eu
cd "$(dirname "$0")/.."
COMPLAIN=
[ "${1:-}" = --complain ] && { COMPLAIN=1; shift; }
REL=${1:-26.04}
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
case "$REL" in
  fedora:*)
    N=${REL#fedora:}
    PKG=target/rpm/crake-${VERSION}-1.x86_64.rpm; AS=crake.rpm
    IMG=target/vm/fedora-$N.qcow2
    DIR="https://download.fedoraproject.org/pub/fedora/linux/releases/$N/Cloud/x86_64/images"
    URL=$(curl -fsSL "$DIR/" | grep -o 'Fedora-Cloud-Base-Generic[^"<]*\.qcow2' | sort -u | tail -n1)
    URL="$DIR/$URL" ;;
  *)
    PKG=target/deb/crake_${VERSION}_amd64.deb; AS=crake.deb
    IMG=target/vm/ubuntu-$REL.img
    URL="https://cloud-images.ubuntu.com/releases/$REL/release/ubuntu-$REL-server-cloudimg-amd64.img" ;;
esac
[ -f "$PKG" ] || { echo "no $PKG: run scripts/build-deb.sh (and build-rpm.sh)" >&2; exit 1; }
if [ ! -f "$IMG" ]; then
  mkdir -p target/vm
  curl -fsSL -o "$IMG.part" "$URL"
  mv "$IMG.part" "$IMG"
fi
WORK=$(mktemp -d "${TMPDIR:-/tmp}/crake-vm.XXXXXX")
trap 'rm -rf "$WORK"' EXIT

qemu-img create -q -f qcow2 -b "$PWD/$IMG" -F qcow2 "$WORK/disk.qcow2" 12G
# cloud-init: run the test at first boot, then power off.
mkdir -p "$WORK/seed" "$WORK/test"
printf 'instance-id: crake-vm-test\nlocal-hostname: crake-test\n' > "$WORK/seed/meta-data"
cat > "$WORK/seed/user-data" <<'CLOUD'
#cloud-config
package_update: false
runcmd:
  - [sh, -c, "mkdir -p /test && mount -o ro LABEL=CRAKETEST /test && sh /test/run.sh > /dev/ttyS1 2>&1; poweroff"]
CLOUD
xorriso -as mkisofs -quiet -output "$WORK/seed.iso" -volid cidata -joliet -rock "$WORK/seed"
cp "$PKG" "$WORK/test/$AS"
cp -r scripts/deb-test scripts/e2e "$WORK/test/"
cp scripts/vm-test/run.sh "$WORK/test/"
[ -z "$COMPLAIN" ] || touch "$WORK/test/complain"
xorriso -as mkisofs -quiet -output "$WORK/test.iso" -volid CRAKETEST -joliet -rock "$WORK/test"

echo "== VM: Ubuntu $REL (a few minutes)"
timeout 1200 qemu-system-x86_64 -machine q35,accel=kvm -cpu host -m 2048 -smp 2 \
  -display none -monitor none -no-reboot \
  -drive "file=$WORK/disk.qcow2,if=virtio" \
  -drive "file=$WORK/seed.iso,if=virtio,format=raw,readonly=on" \
  -drive "file=$WORK/test.iso,if=virtio,format=raw,readonly=on" \
  -nic user,model=virtio-net-pci \
  -serial "file:$WORK/console.log" -serial "file:$WORK/test.log" || true
cp "$WORK/test.log" target/vm/last-test.log
cat "$WORK/test.log"
grep -q '^VM-TEST-RESULT: 0' "$WORK/test.log" || {
  echo "(the VM's console, last lines:)"; tail -n 20 "$WORK/console.log"; exit 1; }
