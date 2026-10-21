# Client de charge de make perf-extra : <port> <processus> <requêtes par
# processus>, connexions persistantes (HTTP/1.1). Affiche les requêtes par
# seconde, ou rien si le serveur n'a pas répondu.
import sys, time, http.client
from multiprocessing import Pool

def worker(args):
    port, count = args
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    for _ in range(count):
        conn.request("GET", "/")
        response = conn.getresponse()
        response.read()
        if response.status != 200:
            raise SystemExit(f"statut {response.status}")
    conn.close()

def wait_ready(port):
    for _ in range(100):
        try:
            conn = http.client.HTTPConnection("127.0.0.1", port, timeout=1)
            conn.request("GET", "/"); conn.getresponse().read(); conn.close()
            return True
        except OSError:
            time.sleep(0.05)
    return False

port, procs, count = (int(a) for a in sys.argv[1:4])
if wait_ready(port):
    start = time.perf_counter()
    with Pool(procs) as pool:
        pool.map(worker, [(port, count)] * procs)
    print(int(procs * count / (time.perf_counter() - start)))
