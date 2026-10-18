"""The OpenVPN gateway's sign-in, for scripts/e2e-test.sh: openvpn's
management interface (management-client-auth) asks it about each client.
A client that says it can open a page (IV_SSO webauth) is sent one
(client-pending-auth WEB_AUTH), and signed in once that page is fetched; a
client back with its token is let in by openvpn itself (auth-gen-token),
without asking here; any other is refused. What each client said goes to /link/openvpn-seen.

    sso.py <management socket> <page address> <page port>
"""
import socket, sys, threading, time
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

path, host, port = sys.argv[1], sys.argv[2], int(sys.argv[3])
pending = {}
lock = threading.Lock()

while True:
    try:
        mgmt = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        mgmt.connect(path)
        break
    except OSError:
        time.sleep(0.2)


def send(line):
    with lock:
        mgmt.sendall((line + "\n").encode())


class Page(BaseHTTPRequestHandler):
    def do_GET(self):
        cid = parse_qs(urlparse(self.path).query).get("cid", [""])[0]
        kid = pending.pop(cid, None)
        if kid is None:
            self.send_response(404)
            self.end_headers()
            return
        send(f"client-auth-nt {cid} {kid}")
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"Signed in\n")

    def log_message(self, *args):
        pass


threading.Thread(target=HTTPServer((host, port), Page).serve_forever, daemon=True).start()


def seen(text):
    with open("/link/openvpn-seen", "a") as f:
        f.write(text + "\n")


buf, client, env = b"", None, {}
while True:
    data = mgmt.recv(4096)
    if not data:
        break
    buf += data
    while b"\n" in buf:
        line, buf = buf.split(b"\n", 1)
        line = line.decode(errors="replace").rstrip("\r")
        if line.startswith(">CLIENT:CONNECT,") or line.startswith(">CLIENT:REAUTH,"):
            kind, rest = line[len(">CLIENT:"):].split(",", 1)
            cid, kid = rest.split(",")[:2]
            client, env = (kind, cid, kid), {}
        elif line.startswith(">CLIENT:ENV,") and client:
            item = line[len(">CLIENT:ENV,"):]
            if item != "END":
                k, _, v = item.partition("=")
                env[k] = v
                continue
            kind, cid, kid = client
            client = None
            state = env.get("session_state", "")
            sso = env.get("IV_SSO", "")
            seen(f"{kind} session_state={state} IV_SSO={sso}")
            if kind == "REAUTH" or state.startswith("Authenticated"):
                send(f"client-auth-nt {cid} {kid}")
            elif "webauth" in sso.split(","):
                pending[cid] = kid
                url = f"http://{host}:{port}/sign-in?cid={cid}"
                send(f'client-pending-auth {cid} {kid} "WEB_AUTH::{url}" 120')
            else:
                send(f'client-deny {cid} {kid} "no sign-in page" "This gateway signs in in a browser."')
