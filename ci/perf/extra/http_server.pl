# Serveur HTTP::Daemon, un processus par connexion (connexions persistantes).
# Sans Nagle : en-tête et corps partent en deux envois, chaque requête
# attendrait l'ACK retardé (≈ 40 ms).
use strict; use warnings; use HTTP::Daemon; use HTTP::Response; use Socket qw(IPPROTO_TCP TCP_NODELAY);
$SIG{CHLD} = 'IGNORE';
my $d = HTTP::Daemon->new(LocalAddr => '127.0.0.1', LocalPort => $ARGV[0], ReuseAddr => 1, Listen => 64) or exit 1;
while (my $c = $d->accept) {
    if (fork) { close $c; next; }
    setsockopt($c, IPPROTO_TCP, TCP_NODELAY, 1);
    while (my $r = $c->get_request) {
        $c->send_response(HTTP::Response->new(200, 'OK', ['Content-Length' => 2], 'ok'));
    }
    exit 0;
}
