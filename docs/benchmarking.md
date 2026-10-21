# Benchmarking : mémoire et temps

Ce guide décrit comment mesurer la consommation mémoire (détection de
fuites) et le temps d'exécution d'un programme Ocara, avec les pièges qui
faussent une mesure et les résultats de référence actuels.

Prérequis : un compilateur à jour (`make build`), et `/usr/bin/time`
(paquet `time`) pour le pic mémoire.

## Détecter une fuite : le bilan `--leak-check` (le plus simple)

`ocara programme.oc --leak-check -o programme` compte chaque allocation et
chaque libération de valeur (chaînes, tableaux, maps, objets par classe,
closures…), avec la fonction qui a alloué chaque valeur. À la sortie du
programme, ce qui n'a pas été libéré est listé par classe ou par genre, puis
par fonction d'allocation (fichier et ligne de sa déclaration) — aucun
marqueur, aucune boucle à écrire :

```
── fuites : 2 valeur(s) non libérée(s) à la sortie ──
       1 × Job                          (allouées 1, libérées 0)
                1 alloué(s) dans startWorker — worker.oc:8
       1 × variable capturée            (allouées 1, libérées 0)
                1 alloué(s) dans startWorker — worker.oc:8
```

- `OCARA_LEAK_OUT=fuites.tsv ./programme` écrit le bilan dans un fichier :
  `K<TAB>genre<TAB>restants<TAB>alloués<TAB>libérés`, suivi pour ce genre de
  lignes `S<TAB>genre<TAB>nombre<TAB>fonction<TAB>fichier<TAB>ligne`.
- Une valeur allouée dans une closure est attribuée à la fonction qui
  l'appelle.
- Juste avant le bilan, une passe du collecteur de cycles libère les valeurs
  qui y attendaient : ce qui reste est réellement perdu.
- Le programme doit se terminer normalement (un serveur arrêté par un signal
  ne produit pas de bilan) : pour un serveur, voir « le plateau » plus bas.
- Sans `--leak-check`, le coût est nul (un test de booléen par allocation).

Dans VS Code : encart Benchmark → « Détecter les fuites » ; chaque genre se
déplie en fonctions d'allocation, un clic ouvre la fonction.

## Détecter une fuite : le pic mémoire à deux tailles

Le principe : exécuter le même programme pour N et 10 × N itérations, et
comparer le pic mémoire (`%M` de `/usr/bin/time`, en Ko). Un programme sans
fuite a le même pic aux deux tailles ; une fuite fait grandir le pic
proportionnellement au nombre d'itérations.

Le programme lit son nombre d'itérations dans un marqueur `__N__` :

<!-- doc-check: skip -->
```ocara
import ocara.IO
import ocara.Convert

function work(i:int): int {
    var s:string = "item " + Convert::intToStr(i)
    return s.len()
}

function main(): int {
    var total:int = 0
    var i:int = 0
    while i smaller __N__ {
        total = total + work(i)
        i = i + 1
    }
    IO::writeln(Convert::intToStr(total))
    return 0
}
```

Script de mesure (`measure.sh programme.oc`) :

```bash
#!/bin/bash
# Compile et mesure le pic mémoire pour N = 20 000 puis 200 000.
OC=./target/release/ocara
for n in 20000 200000; do
  sed "s/__N__/$n/" "$1" > "/tmp/bench_$n.oc"
  "$OC" "/tmp/bench_$n.oc" -o "/tmp/bench_$n" > /dev/null || exit 1
  /usr/bin/time -f "$n: %M Ko" "/tmp/bench_$n" > /dev/null
done
```

Lecture du résultat :

```
20000: 2084 Ko
200000: 1960 Ko     → stable : pas de fuite
```

```
20000: 4048 Ko
200000: 17780 Ko    → fuite : (17780 - 4048) × 1024 / 180 000 ≈ 78 octets par itération
```

L'ordre de grandeur des octets perdus par itération oriente la recherche :
8 octets pour un entier dans un tableau, environ 40 pour une petite chaîne,
48 ou plus pour un objet.

## Mesurer un serveur HTTP : le plateau

Un serveur ne termine pas : on suit sa mémoire résidente (`VmRSS`) pendant
qu'on lui envoie des séries de requêtes. Sans fuite, elle se stabilise après
les premières séries (plateau) ; avec une fuite, elle grandit à chaque série.

```bash
#!/bin/bash
# Lance le serveur, envoie 3 séries de 4 000 requêtes, affiche VmRSS après chacune.
./serveur > /tmp/serveur.log 2>&1 &
PID=$!
sleep 2
for serie in 1 2 3; do
  for i in $(seq 1 2000); do
    curl -s -o /dev/null http://127.0.0.1:8081/voitures/1
    curl -s -o /dev/null "http://127.0.0.1:8081/recherche?q=a"
  done
  echo "$((serie * 4000)) requêtes : $(grep VmRSS /proc/$PID/status | awk '{print $2}') Ko"
done
kill $PID
```

La première série monte toujours (caches, pages SQLite, allocateur) : seule
l'évolution entre les séries suivantes compte. Pour un serveur qui n'a pas
d'interface sans fenêtre, compiler une variante de son point d'entrée qui
démarre seulement le serveur (voir
`examples/advanced/mini_project_hexa/probe_main.oc`).

## Mesurer le temps

```bash
/usr/bin/time -f "%e s, %M Ko" ./programme
```

Pour comparer deux versions du compilateur sur le même programme :

```bash
git stash          # version de référence
make build
./target/release/ocara bench.oc -o bench_avant
git stash pop      # version modifiée
make build
./target/release/ocara bench.oc -o bench_apres
/usr/bin/time -f "avant : %e s" ./bench_avant
/usr/bin/time -f "après : %e s" ./bench_apres
```

Exécuter chaque mesure plusieurs fois et garder le minimum et la médiane
(voir « Préparer une mesure fiable ») : le temps d'un programme court varie
d'une exécution à l'autre. Un micro-benchmark doit
faire au moins quelques millions d'opérations pour dépasser le bruit.

## Profiler par fonction : `--profile`

`ocara programme.oc --profile -o programme` encadre chaque fonction et
méthode d'une mesure de temps (`src/ir/profile.rs`, `runtime/src/profile.rs`).
À la sortie du programme, une ligne par fonction, temps propre décroissant :

```
── profil (appels, total ns, propre ns, fonction) ──
57313	6641619	6641619	Calc.fib
1	6675235	23231	main
1	10385	8854	work
```

- **total** : temps passé dans la fonction et ses appelées (appel externe
  seulement pour une fonction récursive) ; **propre** : sans les appelées.
- `OCARA_PROFILE_OUT=fichier.tsv ./programme` écrit le rapport dans un
  fichier au lieu de stderr.
- La mesure coûte quelques dizaines de nanosecondes par appel : comparer
  les fonctions entre elles, pas le temps absolu à un build sans `--profile`.
- Un programme interrompu (serveur arrêté par un signal) n'écrit pas de
  rapport.

## Préparer une mesure fiable

Noter l'environnement avant chaque série : sur une machine chargée, seules
les comparaisons relatives restent valables.

```bash
nproc; uptime                      # charge moyenne > nproc/2 : comparer en relatif
cat /proc/sys/kernel/perf_event_paranoid /proc/sys/kernel/yama/ptrace_scope
```

Répéter et garder le **minimum** (meilleur estimateur sur une machine
chargée) et la **médiane** (un grand écart avec le min signale du bruit) :

```bash
#!/bin/bash
# run.sh RUNS label binaire… → min, médiane, pic mémoire
R=$1; shift; L=$1; shift; : > /tmp/times_$L.txt
for i in $(seq 1 $R); do
  /usr/bin/time -f "%e %M" -o /tmp/t1.txt timeout 120 "$@" > /dev/null 2> /tmp/err_$L.txt
  cat /tmp/t1.txt >> /tmp/times_$L.txt
done
sort -n /tmp/times_$L.txt | awk -v l="$L" '{t[NR]=$1; if($2>m)m=$2}
  END{printf "%-28s min %6.3f  med %6.3f  rss %d KB\n", l, t[1], t[int((NR+1)/2)], m}'
```

## Détecter une complexité super-linéaire : le test par doublement

Même principe que le pic mémoire à deux tailles, appliqué au temps : on
double N plusieurs fois et on regarde le rapport des temps.

```bash
for N in 10000 20000 40000 80000; do
  sed "s/__N__/$N/g" bench.oc > b_$N.oc && ./target/release/ocara b_$N.oc -o b_$N > /dev/null
  ./run.sh 3 b_$N ./b_$N
done
```

Rapport d'environ 2 quand N double : linéaire ; environ 4 : quadratique ; un
saut plus fort : effet de cache. Avec trois tailles éloignées, la pente
log-log `log(t2/t1) / log(N2/N1)` donne l'exposant directement (1 : linéaire,
2 : quadratique).

Le même test s'applique au **compilateur** : générer des programmes de
taille croissante (nombre d'instructions dans une fonction, de fonctions, de
classes, longueur d'une chaîne `elseif`, profondeur d'une expression) et
mesurer `--check` (lexer, parseur, sema), `--no-link -o x.o` (avec
abaissement et Cranelift) et la compilation complète. `strace -f -tt -e
trace=execve,exit_group ./target/release/ocara …` sépare le temps de
compilation de celui de l'éditeur de liens.

## Comparer à une référence Python et C

Écrire le même algorithme en Python (`time.perf_counter()`) et en C
(`cc -O2`, avec `__attribute__((noinline))` sur la fonction appelée, sinon la
boucle est éliminée). Ordre de grandeur attendu pour du code compilé : entre
C et 10 fois C. Plus lent que Python signale un problème (voir
[roadmap.d/perf-runtime-complexite.md](roadmap.d/perf-runtime-complexite.md)).

## Profiler le runtime avec `perf`

`--profile` mesure les fonctions Ocara ; le temps passé dans le runtime
(retain/release, collecte de cycles, allocations) y est compté dans
l'appelant. `perf` le montre par symbole :

```bash
sudo sysctl kernel.perf_event_paranoid=1      # temporaire ; sinon « No supported events found »
perf stat -r 5 -e task-clock,cycles,instructions,cache-misses,context-switches ./prog > /dev/null
perf record -g --call-graph dwarf -o perf.data ./prog > /dev/null
perf report -i perf.data --stdio --no-children --percent-limit 2 | head -60
```

Compiler **sans** `--release` : ce mode retire les symboles (perf et gdb
n'affichent plus que des adresses) et ne change pas la vitesse.

## Profiler sans `perf` : échantillonnage avec gdb

Sans les droits pour `perf` ni `gdb -p` (`ptrace_scope=1`), on lance le
programme **sous** gdb et on l'interrompt de l'extérieur par `SIGUSR2` ; à
chaque arrêt, le script relève la pile de chaque thread.

```python
# pmp.py — gdb -batch -nx -x pmp.py --args ./prog
import gdb, collections
for c in ["set debuginfod enabled off", "set pagination off", "set confirm off",
          "handle SIGUSR2 stop print nopass", "handle SIGPIPE nostop noprint pass"]:
    gdb.execute(c)
leaf, incl, stacks = collections.Counter(), collections.Counter(), collections.Counter()
def sample():
    for t in gdb.selected_inferior().threads():
        t.switch(); f = gdb.newest_frame(); names = []
        while f is not None and len(names) < 16:
            names.append(f.name() or hex(f.pc())); f = f.older()
        if names:
            leaf[names[0]] += 1; stacks[" <- ".join(names[:5])] += 1
            for n in set(names): incl[n] += 1
gdb.execute("run", to_string=True)
while gdb.selected_inferior().pid != 0:
    sample()
    try: gdb.execute("continue", to_string=True)
    except gdb.error: break
tot = sum(leaf.values()) or 1
for title, c in (("propre", leaf), ("inclusif", incl), ("piles", stacks)):
    print("==", title, "==")
    for k, v in c.most_common(15): print("%5.1f%% %s" % (100 * v / tot, k))
```

```bash
#!/bin/bash
# pmp.sh <échantillons> <intervalle_s> <binaire>
B=$3; NAME=$(basename "$B" | cut -c1-15)
gdb -batch -nx -x pmp.py --args "$B" 2> /dev/null | sed -n "/^==/,\$p" &
G=$!
for i in $(seq 1 $1); do sleep $2; kill -0 $G 2> /dev/null || break; pkill -USR2 -x "$NAME"; done
pkill -KILL -x "$NAME"; wait $G
```

- `handle … noprint` implique `nostop` : il faut écrire `stop print`.
- Chaque échantillon coûte environ 30 ms : viser 50 à 200 échantillons sur un
  programme de 2 à 5 s (`./pmp.sh 150 0.01 ./prog`).
- Les signaux arrivent au retour des appels système : la part des fonctions
  qui en font (`sigprocmask`) est gonflée. Confirmer avec `strace -c`.
- Pour le compilateur lui-même, même méthode sur `target/debug/ocara` (avec
  symboles) : `( sleep N; pkill -INT -x ocara ) & gdb -batch -ex run -ex "bt 40"
  --args target/debug/ocara prog.oc --no-link -o y.o`.

## Compter les appels au runtime : points d'arrêt gdb

Compte exact des retain/release et des collectes de cycles (environ 1 000
fois plus lent : prendre un N petit, par exemple 10⁵) :

```
# count.gdb — gdb -batch -nx -x count.gdb ./prog
set debuginfod enabled off
set breakpoint pending off
break ocara_runtime::rc::collect_cycles
ignore 1 100000000
break __rc_retain
ignore 2 1000000000
break __rc_release
ignore 3 1000000000
run > /dev/null
info breakpoints
```

Lire « breakpoint already hit N times ». Une fonction inlinée (par exemple
`possible_root`) n'a pas de point d'arrêt.

## Appels système dans une boucle : `strace -c`

```bash
strace -c -o sys.txt ./prog > /dev/null; head -15 sys.txt     # mono-thread
strace -f -c -o sys.txt ./prog > /dev/null                     # threads, async
```

Un nombre d'appels proportionnel à N signale un appel système par
itération : `futex` (attente de verrou, réveil d'un thread du pool de
tâches `async`). Une exception n'en fait plus aucun (`_setjmp`, sans
sauvegarde du masque de signaux).

## `/usr/bin/time -v` : champs utiles

`/usr/bin/time -v ./prog > /dev/null 2> tv.txt` :

- « Maximum resident set size » : le pic mémoire (`%M`) ;
- « Voluntary context switches » : attentes, `join` ;
- « Involuntary context switches » : machine chargée ;
- « Minor page faults » : environ une par page allouée (allocations
  massives) ;
- « User » / « System time » : un temps système élevé signale des appels
  système dans la boucle.

## Lire le comptage de références généré : `--dump`

```bash
./target/release/ocara prog.oc --dump -o prog > dump.txt
grep -n "^func " dump.txt
sed -n '/^func Classe_methode/,/^func /p' dump.txt | grep -c "__rc_retain\|__rc_release"
```

Utile pour vérifier ce qu'un appel de méthode, un gabarit ou une boucle
coûte en retain/release, et si une valeur est boxée (`__box_int_for_mixed`).

## Fuites invisibles au bilan `--leak-check`

Le bilan compte les valeurs Ocara. Une instance Rust créée par un builtin
(`Box::into_raw`) sans destruction correspondante n'y apparaît pas : seul le
pic mémoire à deux tailles la révèle (10⁵ contre 10⁶ instances). Liste des
candidats :

```bash
grep -n "Box::into_raw\|Box::from_raw" runtime/src/*.rs
```

Pour un bilan sur tous les exemples : compiler chaque `examples/**/*.oc` qui
a un `main` avec `--leak-check`, l'exécuter sous `timeout 30` avec une entrée
standard préparée, puis classer avec `grep -q "fuites : aucune"`. Copier
seulement les sources (les dossiers `.ocaraunit_cache` pèsent des dizaines de
Go).

## Serveur HTTP sous charge concurrente

Le plateau ci-dessus utilise `curl` en série. Pour une charge concurrente,
un script Python avec un `ThreadPoolExecutor` et une
`http.client.HTTPConnection` par thread (keep-alive) envoie N requêtes avec un
parallélisme P, compte les codes HTTP et les exceptions, et relève `VmRSS` et
`Threads` dans `/proc/$(pgrep -x serveur)/status` avant et après. Répéter 3 à
6 séries pour vérifier le plateau. Référence : environ 5 000 à 6 600
requêtes/s avec 8 workers.

Cas limites à couvrir : clients lents (corps incomplet avec un
`Content-Length` annoncé, en-têtes incomplets) en nombre égal aux workers ;
sockets brutes (pipelining, HTTP/1.0, chunked, octets non UTF-8) ;
fichiers statiques binaires (`curl -s $B/big.bin -o got.bin; cmp got.bin
www/big.bin`).

## Dans VS Code : encart « Benchmark »

La barre latérale Ocara (icône du logo) a un encart « Benchmark » pour le
script affiché :

- **Mesurer temps et mémoire** : build Release, `ocara.benchmark.runs`
  exécutions (5 par défaut), temps moyen/min/max et pic mémoire ;
- **Détecter une fuite (`__N__`)** : la méthode ci-dessus, avec les deux
  valeurs de `ocara.benchmark.leakSizes` ; verdict stable ou fuite probable
  (croissance du pic de plus de 1 Mo et de 10 %) ;
- **Profiler par fonction** : build `--profile`, tableau par fonction ;
- **Historique** : mesures conservées par script, avec l'écart à la mesure
  précédente du même genre (▲ plus lent/plus gros, ▼ mieux).

Chaque exécution est limitée à `ocara.benchmark.timeout` secondes (30 par
défaut). Le pic mémoire demande `/usr/bin/time`.

Pistes d'évolution de l'encart (courbe de complexité, compteurs du runtime,
flamegraph, `strace -c`, comparaison A/B) :
[roadmap.d/tooling-vscode-benchmark-profilage.md](roadmap.d/tooling-vscode-benchmark-profilage.md).

## Pièges

- **Le détecteur de cycles agit par lots** : un objet qui a été partagé puis
  libéré peut rester en attente jusqu'à ce que 10 000 candidats se soient
  accumulés. Mesurer sur au moins 20 000 itérations ; en dessous, la mémoire
  semble monter alors qu'elle sera rendue au lot suivant.
- **Threads** : la collecte des cycles a lieu aussi pendant que d'autres
  threads tournent (collecte concurrente) ; quand ils produisent des cycles
  plus vite qu'elle ne les rend, ils l'attendent brièvement — un programme
  multi-thread qui crée beaucoup de cycles peut donc paraître plus lent
  qu'en mono-thread, à mémoire bornée.
- **SQLite** garde un cache de pages par connexion : ouvrir une base dans
  la boucle mesure aussi ce cache. Comparer à taille égale de données.
- **Compiler en dehors de la mesure** : `/usr/bin/time` doit porter sur le
  binaire, jamais sur `ocara`.
- **Sortie** : rediriger la sortie standard (`> /dev/null`) pour ne pas
  mesurer l'affichage du terminal.
- **`--release` ne rend pas plus rapide** : il retire seulement les symboles
  (Cranelift sans niveau d'optimisation). Pour profiler, garder un build par
  défaut.
- **Le temps propre de `--profile`** inclut le runtime (retain/release,
  collectes) et environ 95 ns de mesure par appel instrumenté.
- **Collectes de cycles mal attribuées** : en mono-thread, une collecte est
  déclenchée par le `release` qui fait déborder le tampon ; son coût apparaît
  dans une fonction utilisateur arbitraire (un constructeur, un accesseur).
- **Threads et `async`** : mesurer à petite taille (2 000 et 10 000) et sans
  `ulimit -v`, qui fait échouer la création des threads (abort, rc 134).
  Plusieurs dizaines de Ko par itération signalent un thread jamais joint.
- **Code de retour** : relever le code de sortie à chaque taille (124 :
  délai, 134 : abort, 139 : SIGSEGV) ; un plantage qui n'apparaît qu'à grande
  taille est souvent une valeur brute prise pour un pointeur (seuil 65 536,
  `PTR_THRESHOLD`).

## Mesures de référence

Programmes de mesure du chantier sur le comptage de références
([roadmap.d/memoire-refcount.md](roadmap.d/memoire-refcount.md)), pic mémoire
à 20 000 puis 200 000 itérations :

| Scénario | 20 000 | 200 000 |
|---|---|---|
| Concaténations de chaînes dans une fonction | 2,1 Mo | 2,0 Mo |
| Objets, conteneur partagé par deux porteurs, `match` | 3,0 Mo | 3,0 Mo |
| Closures avec captures, appels indirects | 2,8 Mo | 3,1 Mo |
| Exceptions levées et rattrapées | 2,7 Mo | 2,9 Mo |
| `raise` traversant trois fonctions avec locales | 2,6 Mo | 2,7 Mo |
| Générateurs (`for`, `break`, `return`, `fromMessage`) | 2,0 Mo | 2,0 Mo |
| Ressources `scoped` fermées par un `raise` | 3,3 Mo | 3,3 Mo |
| Cycles créés dans un thread pendant un `join` | 3,8 Mo | 3,9 Mo |

Serveur `examples/advanced/mini_project_hexa` : plateau à environ 9,5 Mo sur
16 000 requêtes (`/voitures/1` et `/recherche`).

Coût d'un appel de fonction manipulant des valeurs comptées (retenue,
libération, enregistrement pour le déroulement des exceptions) : environ
20 ns (5 millions d'appels en 0,11 s).

Audit du 2026-10-19, mis à jour le 2026-10-21 (machine de 12 cœurs chargée ;
minimum de 3 à 5 exécutions). Règle : **aucune mesure ne doit être plus lente
que la référence Python**. `make perf` (`ci/perf_python.sh`, programmes dans
`ci/perf/`) la vérifie : chaque mesure compile le programme Ocara et
l'exécute avec le même algorithme en Python (minimum de 5 exécutions), et
échoue si Ocara est plus lent. Hors de `make regression`, les temps
dépendant de la charge de la machine. Pistes restantes :
[roadmap.d/perf-runtime-complexite.md](roadmap.d/perf-runtime-complexite.md).

| Mesure | Ocara | Référence |
|---|---|---|
| Boucle d'entiers, 2·10⁸ itérations | 0,58 s (`while`), 0,43 s (`for … until`) | C `-O2` 0,15 s ; Python ≈ 11,6 s |
| Boucle de flottants, 10⁸ itérations | 0,25 s | C `-O2` 0,10 s |
| Boucle flottante + `fib(30)` (`loop`) | 0,07 s | Python 1,18 s |
| Appel de fonction libre / accesseur / méthode qui appelle | 1,4 ns / 0,5 ns / ≈ 15 ns | C 0,5 ns ; méthode Python 37 ns |
| 10⁷ appels de méthode qui en appelle une autre (`meth`) | 0,15 s | Python 0,81 s |
| `use P(i)` (objet int + string) | ≈ 80 ns | Python 123 ns |
| Tableau : 2·10⁶ `push` + 2·10⁷ lectures (`arr`) | 0,04 s | Python 1,20 s |
| `map<string,int>`, 10⁶ insertions + lectures (`map`) | 0,28 s | Python 0,53 s |
| `s = s + "x"`, 10⁶ (`concat`) | 0,01 s | Python 0,03 s |
| Graphe de 10⁶ objets liés, parcouru 5 fois (`cycles`) | 0,36 s | Python 0,85 s |
| Gabarit à trois parties, 2·10⁶ (`tpl`) | 0,09 s | f-string Python 0,32 s |
| `String::split` (6 morceaux), 10⁶ (`split`) | 0,14 s | Python 0,16 s |
| `String::replace` + `upper`, 10⁶ (`replace`) | 0,08 s | Python 0,13 s |
| `Array::join` + `intToStr`, 10⁶ (`join`) | 0,09 s | Python 0,19 s |
| Parcours `for k has v in m`, 2·10⁶ paires (`mapiter`) | 0,05 s | Python 0,11 s |
| Comparaisons de chaînes, 2·10⁷ (`strcmp`) | 0,20 s | Python 0,69 s |
| `Array::sort`, 10⁶ entiers (`sort`) | 0,04 s | Python 0,19 s |
| `JSON::encode` + `decode`, 2·10⁵ (`jsonb`) | 0,27 s | Python 0,60 s |
| 10⁶ objets créés + appel de méthode (`objs`) | 0,09 s | Python 0,63 s |
| Exception levée et rattrapée, 10⁶ (`exc`) | 0,05 s | Python 0,17 s |
| `async` + `resolve`, 5·10⁴ (`asy`) | 0,10 s (≈ 2 µs) | `asyncio` Python 0,23 s |
| Serveur HTTP, 8 workers | 5 000–6 600 requêtes/s | — |
