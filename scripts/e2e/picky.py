"""A front for ocserv that lets in only clients calling themselves
AnyConnect, as some companies' gateways do (CLAUDE.md, 7d): any other
user agent gets a 404 for its first request. TLS with ocserv's own
certificate; from the first request on, everything is passed to ocserv
as it comes, a TLS record for a TLS record (ocserv takes each tunnel
packet from one record: re-chunked, it ends the session, "unexpected
CSTP length").

    picky.py <address> <port> <certificate> <key> <ocserv's port>
"""
import select, socket, ssl, sys, threading

addr, port, cert, key, upstream = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4], int(sys.argv[5])
outside = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
outside.load_cert_chain(cert, key)
inside = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
inside.check_hostname = False
inside.verify_mode = ssl.CERT_NONE
RECORD = 16384


def anyconnect(head):
    """Cisco's client, not openconnect: newer openconnect calls itself
    "AnyConnect-compatible OpenConnect VPN Agent" (Homebrew's, CI's Mac,
    2026-10-20), older "Open AnyConnect VPN Agent"; such gateways turn both
    away."""
    for line in head.decode("latin-1").split("\r\n")[1:]:
        name, _, value = line.partition(":")
        if name.strip().lower() == "user-agent":
            agent = value.strip().lower()
            return agent.startswith("anyconnect") and "openconnect" not in agent
    return False


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
        head = b""
        while b"\r\n\r\n" not in head:
            data = c.recv(RECORD)
            if not data:
                return
            head += data
        if not anyconnect(head.split(b"\r\n\r\n", 1)[0]):
            c.sendall(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            return
        u = inside.wrap_socket(socket.create_connection(("127.0.0.1", upstream)))
        u.sendall(head)
        splice(c, u)
    except (OSError, ssl.SSLError):
        pass
    finally:
        conn.close()


server = socket.create_server((addr, port))
while True:
    conn, _ = server.accept()
    threading.Thread(target=client, args=(conn,), daemon=True).start()
