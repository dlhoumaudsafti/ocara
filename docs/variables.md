# Variables et constantes : cycle de vie

Ce document décrit, pour chaque forme de variable et de constante d'Ocara,
comment on la déclare, comment on l'utilise, ce que le compilateur interdit,
et quand sa valeur est libérée. La grammaire formelle est dans
[EBNF.md §9](EBNF.md#9-variables-et-constantes), les messages d'erreur dans
[diagnostics.md](diagnostics.md).

## Vue d'ensemble

| Forme | Où | Réaffectable | Visible | Valeur rendue | Ressource fermée |
|---|---|---|---|---|---|
| `var` | fonction, méthode | oui | jusqu'à la fin du bloc | fin du bloc, écrasement | jamais (E28) |
| `const` locale | fonction, méthode | non | jusqu'à la fin du bloc | fin du bloc | jamais (E28) |
| `scoped` | fonction, méthode | oui | jusqu'à la fin du bloc | fin du bloc, écrasement | fin du bloc |
| `consumed` | fonction, méthode | oui | jusqu'à son unique usage | après l'instruction de son usage | après son usage |
| `const` globale | hors fonction | non | tout le module | jamais (littéral) | — |
| `const` de classe | dans une classe | non | selon sa visibilité | jamais (littéral) | — |
| `property` | dans une classe | oui | selon sa visibilité | avec l'objet | avec l'objet |
| paramètre | signature | non | tout le corps | fin de l'appel | — |

« Ressource » désigne un handle natif : `Mutex`, `SQLite`, `MySQL`,
`MariaDB`, `HTTPRequest`, `HTTPResponse` (et `Thread`, qui se finalise par
`.join()`/`.detach()`).

## Le modèle mémoire en bref

- Une valeur scalaire (`int`, `float`, `bool`) est copiée, jamais allouée.
- Une valeur tas (`string`, `array`, `map`, instance de classe, `struct`,
  closure) est **comptée** : chaque variable, champ, élément de conteneur ou
  capture qui la référence en tient une référence. Elle est libérée au
  moment précis où la dernière référence disparaît.
- Affecter une valeur à une autre variable la **partage** : `var y = x` ne
  copie pas. Pour une copie indépendante, l'écrire explicitement
  (`x.slice(0, x.len())` pour un tableau).
- Un littéral (`"texte"`) est figé dans le binaire : jamais alloué, jamais
  libéré.
- Les références circulaires (un parent qui pointe vers son enfant et
  réciproquement) sont rattrapées par un détecteur de cycles.
- Les comptes sont atomiques : une valeur peut être partagée entre threads.
- Ce n'est pas un ramasse-miettes : rien ne parcourt le tas, la libération
  est déterministe.

Une sortie anticipée (`return`, `break`, `continue`) et une exception
(`raise`) rendent les références et ferment les ressources de tous les blocs
et de toutes les fonctions qu'elles traversent.

## `var` — variable

```ocara
var count:int = 0
var names:array<string> = ["a", "b"]
count = count + 1
names = []               // l'ancien tableau est rendu
```

- **Déclaration** : type et valeur initiale obligatoires
  (`var x:int` seul est une erreur de syntaxe).
- **Portée** : du point de déclaration à la fin du bloc `{ }` qui la
  contient. Une `var` déclarée dans un `if` ou une boucle n'existe plus
  après. Un nom redéclaré dans un bloc imbriqué masque l'extérieur jusqu'à
  la fin de ce bloc.
- **Libération** : la variable rend sa valeur en fin de bloc ; une
  réaffectation rend l'ancienne. La valeur survit si elle a été partagée
  (`return`, champ, conteneur, autre variable, argument conservé par
  l'appelé).
- **Contrainte** : une ressource déclarée en `var` n'est **jamais fermée
  automatiquement**. Si elle ne s'échappe pas et n'est jamais fermée
  (`.close()`/`.destroy()`), c'est l'erreur **E28** : la fermer à la main ou
  la déclarer `scoped`/`consumed`.

## `const` locale — variable non réaffectable

```ocara
function main(): int {
    const total:int = compute()   // n'importe quelle expression
    return total
}
```

- **Déclaration** : comme `var` ; la valeur peut être n'importe quelle
  expression, évaluée à la déclaration.
- **Contrainte** : toute réaffectation est refusée (**E10**,
  `cannot assign to 'total'`). Les éléments d'un tableau ou d'une map `const`
  restent modifiables : seule la variable est figée.
- **Libération** : identique à `var`. Même règle **E28** pour une ressource.

## `scoped` — variable de bloc

```ocara
if ready {
    scoped db:SQLite = SQLite::open("./app.db")
    scoped rows:array<map<string, mixed>> = db.query("SELECT * FROM t")
    IO::writeln(rows.len())
}   // rows rendu, puis db fermée
```

- **Déclaration et portée** : comme `var`.
- **Libération** : en fin de bloc, la variable rend sa valeur (même règle
  que `var`) et **ferme sa ressource** (`.close()`/`.destroy()` implicite).
  Une sortie anticipée ou une exception ferme aussi.
- **Contraintes** :
  - une ressource `scoped` ne peut pas sortir de son bloc (affectation,
    `return`, argument) : **E18** ;
  - un `Thread` `scoped` doit être `.join()` ou `.detach()` avant la fin du
    bloc : **E19** ;
  - fermer deux fois une ressource à la main : **E25** ;
  - interdit sur un champ de classe (utiliser `property`).
- Une **valeur** `scoped` (`string`, `array`, objet…) peut sortir de son bloc
  sans restriction : elle est partagée, et reste vivante tant qu'on la
  référence.

## `consumed` — variable à usage unique

```ocara
consumed rows:array<map<string, mixed>> = db.query("SELECT * FROM t")
for row in rows {          // l'unique usage
    IO::writeln(row["name"])
}                          // rows rendu juste après cette instruction
```

- **Déclaration** : comme `var`.
- **Libération** : juste après l'**instruction** de sa première lecture.
  Si cette instruction a conservé la valeur ailleurs (champ, conteneur,
  autre variable, `return`), elle reste vivante par cette autre référence.
  Une ressource `consumed` est fermée au même moment.
- **Usage dans une boucle** plus profonde que la déclaration : l'usage se
  répète, la valeur n'est donc rendue qu'en fin de bloc.
- **Contraintes** :
  - toute seconde lecture est refusée : **E17** (y compris deux `${x}` dans
    un même gabarit `HTML::renderFile`) ;
  - jamais lue : avertissement « variable non utilisée », valeur rendue en
    fin de bloc ;
  - mêmes règles que `scoped` pour les ressources (E18, E19, E25) ;
  - interdit sur un champ de classe.

## `const` globale

```ocara
const TAX:float = 0.2
const TIMEOUT_MS:int = 60 * 1000
const APP_NAME:string = "Ocara"
```

- **Déclaration** : au niveau du module, hors de toute fonction.
- **Contrainte** : la valeur doit être connue à la compilation — un
  littéral, éventuellement négé ou combiné par `+ - * / %` (concaténation
  `+` entre chaînes). Un appel de fonction est refusé : **E61**. Pour une
  valeur calculée, écrire une fonction.
- **Usage** : par son nom, depuis toute fonction ou méthode du module.
- **Libération** : jamais — c'est un littéral figé dans le binaire.

## `const` de classe

```ocara
class Config {
    public const VERSION:string = "1.0.0"
    protected const RETRY:int   = 3
    private const SECRET:string = "abc"

    public static method retries(): int {
        return self::RETRY        // depuis la classe
    }
}

IO::writeln(Config::VERSION)      // depuis l'extérieur, sans instance
```

- **Déclaration** : dans le corps de la classe, avec une visibilité.
- **Contrainte** : valeur connue à la compilation (mêmes règles que la
  `const` globale) — sinon **E55** ; la visibilité s'applique (`public`
  partout, `protected` dans la classe et ses sous-classes, `private` dans
  la classe).
- **Usage** : `Classe::NOM`, ou `self::NOM` dans la classe. La valeur est
  insérée telle quelle à chaque usage.
- **Libération** : jamais.

## `property` — champ d'objet

```ocara
class Repo {
    private property db:SQLite
    public property name:string = "repo"

    init() {
        self.db = SQLite::open("./app.db")
    }
}
```

- **Instanciation** : avec l'objet (`use Repo()`), initialisée par sa
  valeur par défaut ou dans `init()`.
- **Libération** : quand l'objet est libéré (plus aucune référence), chaque
  champ rend sa valeur et **chaque champ ressource est fermé**.
- **Contrainte** : ne pas fermer à la main un champ ressource
  (`self.db.close()`), il le serait deux fois : **E35**. Un objet qui
  possède une ressource se déclare `scoped`/`consumed` (règle E28).

## Paramètres

- **Non réaffectables** : `n = 2` dans le corps est refusé (E10).
- La valeur reçue est empruntée : elle reste valide pendant tout l'appel,
  même si l'appelant la rend ailleurs entre-temps. Pour la garder au-delà
  (champ, conteneur), il suffit de l'y ranger.

## Cas particuliers

- **Variable de boucle** (`for x in xs`) : non réaffectable, vivante le
  temps d'une itération.
- **Capture par une closure** : la variable est partagée par le bloc et la
  closure ; elle reste vivante tant que l'un des deux existe. Une
  affectation depuis l'un est visible de l'autre.
- **Exceptions** : la valeur levée par `raise` est rendue après le
  gestionnaire `on` qui la rattrape.
- **Générateurs** (`emit`) : les variables vivent dans l'état du
  générateur ; un générateur abandonné (`break`, `return`) est détruit
  proprement.

## Récapitulatif des diagnostics

| Code | Règle |
|---|---|
| E10 | Affectation d'une `const`, d'un paramètre ou d'une variable de boucle |
| E17 | Seconde lecture d'une `consumed` |
| E18 | Ressource `scoped`/`consumed` qui sort de son bloc |
| E19 | `Thread` `scoped`/`consumed` ni `.join()` ni `.detach()` |
| E25 | Ressource fermée deux fois |
| E28 | Ressource `var`/`const` jamais fermée |
| E35 | Fermeture manuelle d'un champ ressource |
| E55 | `const` de classe non évaluable à la compilation |
| E61 | `const` globale non évaluable à la compilation |
