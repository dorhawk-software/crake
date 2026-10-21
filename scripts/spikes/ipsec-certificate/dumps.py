"""Chunk dumps (%B) in a strongSwan file log: (label, bytes) in order."""
import re
HEAD = re.compile(r"\[(\w+)\] (.*?) => (\d+) bytes @ (0x[0-9a-f]+)")
ROW = re.compile(r"^\s*(\d+): ((?:[0-9A-F]{2} ){1,16})")

def dumps(path):
    out, cur, want = [], None, 0
    for line in open(path, errors="replace"):
        m = HEAD.search(line)
        if m:
            cur = [m.group(1), m.group(2).strip(), bytearray()]
            want = int(m.group(3))
            out.append(cur)
            continue
        if cur is not None:
            body = line.split("] ", 1)[-1] if "] " in line else line
            r = ROW.match(body)
            if r and len(cur[2]) < want:
                cur[2] += bytes.fromhex(r.group(2).replace(" ", ""))
                continue
            cur = None
    return [(c[0], c[1], bytes(c[2])) for c in out]
