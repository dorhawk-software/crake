"""The gateway's certificate, out of the decrypted IKE_AUTH reply."""
import sys, collections
sys.path.insert(0, sys.argv[0].rsplit("/", 1)[0])
from dumps import dumps
log, want = sys.argv[1], open(sys.argv[2], "rb").read()
ds = dumps(log)
print("dumps:", collections.Counter(l for _, l, _ in ds))
# Pair each "plain" with the "assoc" before it (IKE header + SK/SKF header).
msgs = collections.defaultdict(list)
assoc = None
for _, label, data in ds:
    if label == "assoc":
        assoc = data
    elif label == "plain" and assoc is not None and len(assoc) >= 32:
        exch, flags, mid = assoc[18], assoc[19], assoc[20:24]
        frag = (int.from_bytes(assoc[32:34], "big"), int.from_bytes(assoc[34:36], "big")) if assoc[16] == 53 else (1, 1)
        # strongSwan dumps the padding apart: "plain" is the payloads alone.
        msgs[(mid, exch, flags)].append((frag, assoc[28], data))
        assoc = None
got = None
for (mid, exch, flags), parts in msgs.items():
    parts.sort()
    first = [p for p in parts if p[0][0] == 1][0][1]
    buf = b"".join(p[2] for p in parts)
    kind, o, seen = first, 0, []
    while o + 4 <= len(buf) and kind:
        nxt, ln = buf[o], int.from_bytes(buf[o + 2 : o + 4], "big")
        seen.append(kind)
        if kind == 37 and got is None and buf[o + 4] == 4:
            got = buf[o + 5 : o + ln]
        kind, o = nxt, o + ln
    print(f"message {mid.hex()} exchange {exch} {'response' if flags & 0x20 else 'request'}, "
          f"{len(parts)} part(s), payloads {seen}")
if got is None:
    print("no certificate found"); sys.exit(1)
open("got.der", "wb").write(got)
print("certificate:", len(got), "bytes;", "the gateway's, byte for byte" if got == want else "DIFFERENT from the gateway's")
