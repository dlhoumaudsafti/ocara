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

Exécuter chaque mesure plusieurs fois et garder la meilleure : le temps d'un
programme court varie d'une exécution à l'autre. Un micro-benchmark doit
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

## Pièges

- **`for i in 0 until N` alloue un tableau de N entiers** (8 octets par
  itération, voir docs/roadmap.d/langage-for-range-allocation.md). La
  mesure le prend pour une fuite : utiliser une boucle `while` pour piloter
  le nombre d'itérations.
- **Le détecteur de cycles agit par lots** : un objet qui a été partagé puis
  libéré peut rester en attente jusqu'à ce que 10 000 candidats se soient
  accumulés. Mesurer sur au moins 20 000 itérations ; en dessous, la mémoire
  semble monter alors qu'elle sera rendue au lot suivant.
- **Threads** : la collecte des cycles a lieu quand les autres threads sont
  bloqués dans un appel du runtime (attente d'une requête HTTP, `join`,
  `sleep`, verrou, boucle d'événements Tauri, `present`/`delay` SDL). Un
  thread qui calcule sans jamais se bloquer la retarde.
- **SQLite** garde un cache de pages par connexion : ouvrir une base dans
  la boucle mesure aussi ce cache. Comparer à taille égale de données.
- **Compiler en dehors de la mesure** : `/usr/bin/time` doit porter sur le
  binaire, jamais sur `ocara`.
- **Sortie** : rediriger la sortie standard (`> /dev/null`) pour ne pas
  mesurer l'affichage du terminal.

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
