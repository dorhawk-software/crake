"""What of the client's own sign-in shows in the dump."""
import sys
sys.path.insert(0, sys.argv[0].rsplit("/", 1)[0])
from dumps import dumps
ds = dumps(sys.argv[1])
for needle in (b"alice", b"hunter2"):
    hits = sorted({l for _, l, d in ds if needle in d})
    print(f"{needle.decode()!r} in dumps labelled: {hits or 'none'}")
txt = open(sys.argv[1], errors="replace").read()
print("'alice' anywhere in the log text:", "alice" in txt)
