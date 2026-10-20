"""The OpenVPN gateway's sign-in, for scripts/e2e-test.sh: openvpn's
management interface (management-client-auth) asks it about each client.
A client signing in as sso-placeholder that says it can open a page (IV_SSO
webauth) is sent one (client-pending-auth WEB_AUTH), and signed in once that
page is fetched; alice is let in with her password (hunter2); dave gives
his (hunter2) and then a code from his app (424242), asked with CR_TEXT
when the client says it can answer one (IV_SSO crtext); a client back
with its token is let in by openvpn itself (auth-gen-token), without asking
here; any other is refused. What each client said goes to /link/openvpn-seen.

    sso.py <management socket> <page address> <page port> [seen file]
"""
import base64, socket, sys, threading, time
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.parse import parse_qs, urlparse

path, host, port = sys.argv[1], sys.argv[2], int(sys.argv[3])
seen_file = sys.argv[4] if len(sys.argv) > 4 else "/link/openvpn-seen"
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
    with open(seen_file, "a") as f:
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
        if line.startswith((">CLIENT:CONNECT,", ">CLIENT:REAUTH,", ">CLIENT:CR_RESPONSE,")):
            kind, rest = line[len(">CLIENT:"):].split(",", 1)
            cid, kid, *answer = rest.split(",")
            client, env = (kind, cid, kid, answer[0] if answer else ""), {}
        elif line.startswith(">CLIENT:ENV,") and client:
            item = line[len(">CLIENT:ENV,"):]
            if item != "END":
                k, _, v = item.partition("=")
                env[k] = v
                continue
            kind, cid, kid, answer = client
            client = None
            state = env.get("session_state", "")
            sso = env.get("IV_SSO", "")
            user = env.get("username", "")
            seen(f"{kind} user={user} session_state={state} IV_SSO={sso}")
            if kind == "CR_RESPONSE":
                code = base64.b64decode(answer).decode(errors="replace")
                if code == "424242":
                    send(f"client-auth-nt {cid} {kid}")
                else:
                    send(f'client-deny {cid} {kid} "wrong code"')
            elif kind == "REAUTH" or state.startswith("Authenticated"):
                send(f"client-auth-nt {cid} {kid}")
            elif user == "alice":
                if env.get("password") == "hunter2":
                    send(f"client-auth-nt {cid} {kid}")
                else:
                    send(f'client-deny {cid} {kid} "wrong password"')
            elif user == "dave" and "crtext" in sso.split(","):
                if env.get("password") == "hunter2":
                    send(f'client-pending-auth {cid} {kid} "CR_TEXT:R:Enter the code from your app" 120')
                else:
                    send(f'client-deny {cid} {kid} "wrong password"')
            elif user == "sso-placeholder" and "webauth" in sso.split(","):
                pending[cid] = kid
                url = f"http://{host}:{port}/sign-in?cid={cid}"
                send(f'client-pending-auth {cid} {kid} "WEB_AUTH::{url}" 120')
            else:
                send(f'client-deny {cid} {kid} "no sign-in page" "This gateway signs in in a browser."')
