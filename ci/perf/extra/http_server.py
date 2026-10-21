import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

# Réponse écrite en un envoi, sans Nagle : sinon chaque requête attend
# l'ACK retardé (≈ 40 ms) et la comparaison ne mesure plus le serveur.
class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    wbufsize = 65536
    disable_nagle_algorithm = True
    def do_GET(self):
        body = b"ok"
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def log_message(self, *args):
        pass

ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Handler).serve_forever()
