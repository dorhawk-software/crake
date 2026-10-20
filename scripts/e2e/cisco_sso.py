"""A front for ocserv that signs users in the way Cisco's gateways do with
SAML and AnyConnect's "external browser" (ocserv speaks no SAML): the
client is sent to a sign-in page in the browser, the page hands the
client's local port (29786) a token encrypted to the key the client sent
(openconnect's hpke.c: ECDH on P-256, HKDF-SHA256 "AC_ECIES", AES-256-GCM),
and the client posts the token back. The front then signs in to ocserv
itself (alice, hunter2) and hands the client ocserv's session; the tunnel
that follows is passed to ocserv a TLS record for a TLS record, as
picky.py does. What it saw goes to stdout.

    cisco_sso.py <address> <port> <certificate> <key> <ocserv's port>
"""
import base64, os, re, select, socket, ssl, struct, sys, threading
from urllib.parse import quote

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.kdf.hkdf import HKDF

addr, port, cert, key, upstream = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4], int(sys.argv[5])
outside = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
outside.load_cert_chain(cert, key)
inside = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
inside.check_hostname = False
inside.verify_mode = ssl.CERT_NONE
RECORD = 16384
# The client waiting for its token: its key, and the token it will get.
waiting = {"key": None, "token": None}
lock = threading.Lock()


def say(*what):
    print(*what, flush=True)


def read_request(s, buffered=b""):
    """One HTTP request: (head, body, what came after), or None at the end."""
    data = buffered
    while b"\r\n\r\n" not in data:
        more = s.recv(RECORD)
        if not more:
            return None
        data += more
    head, rest = data.split(b"\r\n\r\n", 1)
    length = int(header(head, "content-length") or 0)
    while len(rest) < length:
        more = s.recv(RECORD)
        if not more:
            return None
        rest += more
    return head, rest[:length], rest[length:]


def header(head, name):
    for line in head.decode("latin-1").split("\r\n")[1:]:
        k, _, v = line.partition(":")
        if k.strip().lower() == name:
            return v.strip()
    return None


def respond(s, status, body=b"", headers=()):
    lines = [f"HTTP/1.1 {status}", f"Content-Length: {len(body)}", *headers]
    s.sendall(("\r\n".join(lines) + "\r\n\r\n").encode() + body)


def sealed(token, client_key_b64):
    """The token as the browser hands it to the client: encrypted to the
    client's key, in openconnect's TLV layout (hpke.c)."""
    client = serialization.load_der_public_key(base64.b64decode(client_key_b64))
    ours = ec.generate_private_key(ec.SECP256R1())
    secret = ours.exchange(ec.ECDH(), client)
    aes = HKDF(algorithm=hashes.SHA256(), length=32, salt=None, info=b"AC_ECIES").derive(secret)
    iv = os.urandom(12)
    enc = Cipher(algorithms.AES(aes), modes.GCM(iv)).encryptor()
    ciphertext = enc.update(token.encode()) + enc.finalize()
    tag = enc.tag[:12]
    pub = ours.public_key().public_bytes(serialization.Encoding.DER, serialization.PublicFormat.SubjectPublicKeyInfo)
    blob = struct.pack(">H", 1)
    for t, v in ((1, pub), (2, tag), (3, ciphertext), (4, iv)):
        blob += struct.pack(">HH", t, len(v)) + v
    return base64.b64encode(blob).decode()


def sso_form(host):
    return f"""<?xml version="1.0" encoding="UTF-8"?>
<config-auth client="vpn" type="auth-request" aggregate-auth-version="2">
<opaque is-for="sg"><tunnel-group>ACME</tunnel-group><auth-method>single-sign-on-external-browser</auth-method></opaque>
<auth id="main">
<title>Login</title>
<message>Please complete the authentication process in the AnyConnect Login window.</message>
<sso-v2-login>https://{host}/sso/login</sso-v2-login>
<sso-v2-login-final>https://{host}/sso/done</sso-v2-login-final>
<sso-v2-token-cookie-name>acSamlv2Token</sso-v2-token-cookie-name>
<sso-v2-error-cookie-name>acSamlv2Error</sso-v2-error-cookie-name>
<sso-v2-browser-mode>external</sso-v2-browser-mode>
<form><input type="sso" name="sso-token"></input></form>
</auth>
</config-auth>
""".encode()


AUTH_HEADERS = "X-Aggregate-Auth: 1\r\nX-Transcend-Version: 1\r\nContent-Type: application/x-www-form-urlencoded\r\n"


def ocserv_session():
    """Signs in to ocserv as alice and gives back its last answer (status
    line, headers, body), the session's cookie in it."""
    u = inside.wrap_socket(socket.create_connection(("127.0.0.1", upstream)))
    cookies = {}
    body = (b'<?xml version="1.0" encoding="UTF-8"?><config-auth client="vpn" type="init">'
            b'<version who="vpn">v9.12</version><device-id>linux-64</device-id></config-auth>')
    for _ in range(4):
        cookie = "; ".join(f"{k}={v}" for k, v in cookies.items())
        req = (f"POST / HTTP/1.1\r\nHost: vpn\r\nUser-Agent: AnyConnect Linux_64 4.10\r\n{AUTH_HEADERS}"
               f"{'Cookie: ' + cookie + chr(13) + chr(10) if cookie else ''}Content-Length: {len(body)}\r\n\r\n")
        u.sendall(req.encode() + body)
        head, answer, _ = read_request(u)
        for line in head.decode("latin-1").split("\r\n"):
            if line.lower().startswith("set-cookie:"):
                k, _, v = line.split(":", 1)[1].strip().split(";")[0].partition("=")
                cookies[k] = v
        text = answer.decode()
        if 'type="complete"' in text:
            u.close()
            return head, answer
        fields = "".join(
            f"<{n}>{'alice' if n == 'username' else 'hunter2'}</{n}>"
            for n in re.findall(r'<input[^>]*name="([^"]+)"', text)
        )
        body = (b'<?xml version="1.0" encoding="UTF-8"?><config-auth client="vpn" type="auth-reply">'
                b'<version who="vpn">v9.12</version><device-id>linux-64</device-id>'
                + f"<auth>{fields}</auth></config-auth>".encode())
    raise RuntimeError("ocserv didn't sign alice in")


def splice(a, b):
    """Each record read from one side written whole to the other."""
    other = {a: b, b: a}
    while True:
        ready = [s for s in other if s.pending()]
        if not ready:
            ready, _, _ = select.select(list(other), [], [])
        for s in ready:
            data = s.recv(RECORD)
            if not data:
                return
            other[s].sendall(data)


def client(conn):
    try:
        c = outside.wrap_socket(conn, server_side=True)
        buffered = b""
        while True:
            got = read_request(c, buffered)
            if got is None:
                return
            head, body, buffered = got
            first = head.split(b"\r\n", 1)[0].decode("latin-1")
            host = header(head, "host") or f"{addr}:{port}"
            if first.startswith("CONNECT "):
                # The tunnel, with ocserv's session: ocserv's from here on.
                say("tunnel")
                u = inside.wrap_socket(socket.create_connection(("127.0.0.1", upstream)))
                u.sendall(head + b"\r\n\r\n" + body + buffered)
                splice(c, u)
                return
            if first.startswith("GET /sso/login"):
                # The browser: the user signs in here (nothing to ask in a
                # test), and the page hands the client its token.
                with lock:
                    token, client_key = waiting["token"], waiting["key"]
                if not token:
                    respond(c, "400 Bad Request", b"no client is signing in")
                    continue
                say("page")
                to = f"http://localhost:29786/api/sso/{quote(sealed(token, client_key), safe='')}?return={quote(f'https://{host}/sso/done', safe='')}"
                respond(c, "302 Found", headers=(f"Location: {to}",))
                continue
            if first.startswith("GET /sso/done"):
                respond(c, "200 OK", b"Signed in. You can close this tab.")
                continue
            text = body.decode("utf-8", "replace")
            if 'type="init"' in text:
                client_key = header(head, "x-anyconnect-strap-dh-pubkey")
                if not client_key or "single-sign-on-external-browser" not in text:
                    say("init without an external browser")
                    respond(c, "403 Forbidden", b"this gateway signs in through the browser")
                    return
                with lock:
                    waiting["key"] = client_key
                    waiting["token"] = base64.b32encode(os.urandom(15)).decode().rstrip("=")
                say("init")
                respond(c, "200 OK", sso_form(host), ("Content-Type: text/xml",))
                continue
            sent = re.search(r"<sso-token>([A-Za-z0-9]+)</sso-token>", text)
            with lock:
                expected = waiting["token"]
            if sent and sent.group(1) == expected:
                with lock:
                    waiting["token"] = None
                say("token")
                head2, answer = ocserv_session()
                lines = head2.decode("latin-1").split("\r\n")
                kept = [l for l in lines[1:] if not l.lower().startswith(("content-length", "connection", "transfer-encoding"))]
                respond(c, lines[0].split(" ", 1)[1], answer, kept)
                continue
            say("refused", first)
            respond(c, "401 Unauthorized", b"not signed in")
            return
    except (OSError, ssl.SSLError, RuntimeError) as e:
        say("error", e)
    finally:
        conn.close()


server = socket.create_server((addr, port))
while True:
    conn, _ = server.accept()
    threading.Thread(target=client, args=(conn,), daemon=True).start()
