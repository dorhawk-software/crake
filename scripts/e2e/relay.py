"""One end of a private wire between two containers that have no network:
a tun device whose packets go to the other end's datagram socket in a shared
folder, and back. Nothing else can reach it. While the file `block` exists,
UDP to or from port 443 is dropped, as on networks that block it (CLAUDE.md, 7b).

    relay.py <device> <address/prefix> <own socket> <other end's socket> [block]
"""
import fcntl, os, select, socket, struct, subprocess, sys

dev, addr, own, other = sys.argv[1:5]
block = sys.argv[5] if len(sys.argv) > 5 else None


def blocked(packet):
    """UDP on port 443 (DTLS), while `block` exists."""
    if not block or len(packet) < 28 or packet[0] >> 4 != 4 or packet[9] != 17:
        return False
    at = (packet[0] & 0x0F) * 4
    ports = struct.unpack("!HH", packet[at:at + 4])
    return 443 in ports and os.path.exists(block)


tun = os.open("/dev/net/tun", os.O_RDWR)
fcntl.ioctl(tun, 0x400454CA, struct.pack("16sH", dev.encode(), 0x0001 | 0x1000))  # TUNSETIFF, IFF_TUN|IFF_NO_PI
subprocess.run(["ip", "addr", "add", addr, "dev", dev], check=True)
subprocess.run(["ip", "link", "set", dev, "up"], check=True)
if os.path.exists(own):
    os.unlink(own)
sock = socket.socket(socket.AF_UNIX, socket.SOCK_DGRAM)
sock.bind(own)
os.chmod(own, 0o600)
while True:
    ready, _, _ = select.select([tun, sock], [], [])
    if tun in ready:
        packet = os.read(tun, 65535)
        if blocked(packet):
            continue
        try:
            sock.sendto(packet, other)
        except OSError:
            pass  # the other end isn't up yet: dropped, as on a cable
    if sock in ready:
        packet = sock.recv(65535)
        if not blocked(packet):
            os.write(tun, packet)
