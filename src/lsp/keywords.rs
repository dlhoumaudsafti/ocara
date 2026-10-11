//! Documentation des mots-clés au survol : une phrase par mot-clé et la
//! section de docs/EBNF.md qui le décrit (titre relu dans l'EBNF embarqué).

use std::sync::OnceLock;

const EBNF: &str = include_str!("../../docs/EBNF.md");

/// (mot-clé, numéro de section de docs/EBNF.md, résumé).
const KEYWORDS: &[(&str, &str, &str)] = &[
    ("namespace", "3.1", "Déclare le namespace du fichier (`namespace .` = racine), utilisé pour résoudre les imports."),
    ("import", "4.1", "Importe une classe/fonction : `import ocara.IO` (builtin), `import a.b.Classe` (namespace) ou `import Classe from \"fichier\"`."),
    ("from", "4.1", "Dans `import X from \"fichier\"` : fichier source du symbole importé."),
    ("as", "4.1", "Alias d'import : `import ocara.Convert as C`."),
    ("runtime", "5.6", "Importe un fichier de bloc runtime : `runtime core.main is main`."),
    ("main", "5.3", "Bloc runtime principal du programme."),
    ("error", "5.3", "Bloc runtime exécuté quand le programme se termine en erreur."),
    ("success", "5.3", "Bloc runtime exécuté quand le programme se termine avec succès."),
    ("exit", "5.3", "Bloc runtime exécuté à la fin du programme, dans tous les cas."),
    ("result", "5.5", "Dans un bloc runtime : fixe le code de résultat du programme (`result SUCCESS`, `result 5`) — `return` y est interdit."),
    ("var", "9.1", "Variable mutable. Sa valeur est comptée : libérée quand plus rien ne la référence."),
    ("scoped", "9.2", "Variable de bloc : rend sa référence (ferme sa ressource) à la fin du bloc ; une ressource ne peut pas s'échapper du bloc."),
    ("consumed", "9.3", "Variable à usage unique : rend sa référence (ferme sa ressource) juste après sa première utilisation."),
    ("const", "9.4", "Constante (globale, locale ou de classe) ; valeur de constante de classe connue à la compilation."),
    ("int", "6.1", "Entier signé 64 bits."),
    ("float", "6.1", "Flottant 64 bits."),
    ("string", "6.1", "Chaîne de caractères (méthodes `String` en instance : `s.trim()`, conversions `s.toInt()`...)."),
    ("bool", "6.1", "Booléen `true`/`false`."),
    ("mixed", "6.1", "Type dynamique : désactive la vérification de type (préférer une union `int|string|null`)."),
    ("void", "6.1", "Absence de valeur de retour."),
    ("array", "6.2", "Tableau typé `array<T>` (méthodes `Array` en instance : `arr.len()`, `arr.push(x)`...)."),
    ("map", "6.2", "Tableau associatif `map<K, V>` (méthodes `Map` en instance : `m.keys()`, `m.size()`...)."),
    ("variadic", "14.2", "Paramètre variadic `nom:variadic<T>` (dernier paramètre) — reçoit un `array<T>`."),
    ("true", "7.1", "Littéral booléen vrai."),
    ("false", "7.1", "Littéral booléen faux."),
    ("null", "7.1", "Absence de valeur (types nullables : `T|null`)."),
    ("function", "14", "Déclare une fonction libre : `function nom(param:Type): Retour { ... }`."),
    ("nameless", "14.4", "Fonction anonyme (closure) : `nameless(x:int): int { ... }`."),
    ("async", "14.5", "Fonction/méthode asynchrone : son appel retourne un `Resolvable<T>`."),
    ("resolve", "14.5", "Attend la fin d'une tâche `async` et retourne son résultat `T`."),
    ("return", "12", "Retourne une valeur depuis une fonction/méthode (interdit dans un bloc runtime : `result`)."),
    ("emit", "28", "Émet une valeur depuis un générateur (fonction retournant `message<T>`)."),
    ("message", "28.1", "Type `message<T>` : retour d'un générateur (contient `emit`), consommé par `for x in gen()`."),
    ("class", "16", "Déclare une classe (instances sur le tas via `use`)."),
    ("struct", "16.7", "Agrégat de données : champs et constantes, constructeur généré depuis les champs ; n'étend qu'un struct."),
    ("generic", "20", "Classe générique paramétrée par des types : `generic Liste<T> { ... }` (monomorphisée à la compilation)."),
    ("interface", "17", "Contrat de méthodes, implémenté via `implements` ; peut désigner son implémentation avec `wiring`."),
    ("wiring", "17.1", "Dans une interface : classe concrète liée à l'interface à la compilation (`use Interface()` → cette classe)."),
    ("module", "19.1", "Mixin : membres réutilisables incorporés dans les classes qui l'utilisent (`modules`)."),
    ("modules", "19.2", "Dans l'en-tête d'une classe : modules (mixins) incorporés."),
    ("extends", "18", "Héritage simple d'une classe (ou d'un struct depuis un struct)."),
    ("implements", "18", "Interfaces implémentées par la classe."),
    ("init", "16.2", "Constructeur de la classe, appelé par `use Classe(...)`."),
    ("method", "16.3", "Déclare une méthode : `public [static] [async] method nom(...): Retour { ... }`."),
    ("property", "16.3", "Déclare un champ d'instance : `public property nom:Type`."),
    ("static", "16.5", "Méthode de classe, appelée via `Classe::methode()`."),
    ("public", "16.3", "Visibilité : accessible depuis n'importe où."),
    ("private", "16.3", "Visibilité : accessible uniquement depuis la classe qui déclare le membre (E54)."),
    ("protected", "16.3", "Visibilité : accessible depuis la classe déclarante et ses descendantes (E54)."),
    ("self", "16.6", "Instance courante (`self.champ`), ou la classe courante (`self::methode()`)."),
    ("parent", "18.1", "Classe parente : `parent::methode()`, `parent::init(...)`."),
    ("use", "22", "Instancie une classe/struct : `use Classe(args)` (arguments nommés acceptés)."),
    ("enum", "21", "Énumération de constantes entières : `enum Couleur { Rouge, Vert }`."),
    ("if", "24", "Condition : `if cond { ... } elseif cond { ... } else { ... }`."),
    ("elseif", "24", "Branche alternative conditionnelle d'un `if`."),
    ("else", "24", "Branche par défaut d'un `if`."),
    ("switch", "25", "Branchement multiple sur une valeur."),
    ("default", "25", "Cas par défaut d'un `switch`/`match`."),
    ("match", "26", "Expression de filtrage, un bras par ligne : `1 => ...`, `is string => ...`, `default => ...`."),
    ("while", "27.1", "Boucle tant que la condition est vraie."),
    ("for", "27.2", "Boucle d'itération : `for x in tableau`, `for k has v in map`, `for i in 0..n`."),
    ("in", "27.2", "Dans un `for` : la collection parcourue."),
    ("has", "27.3", "Dans `for k has v in m` : sépare la clé de la valeur d'une map."),
    ("break", "27.5", "Sort de la boucle courante."),
    ("continue", "27.6", "Passe à l'itération suivante de la boucle courante."),
    ("try", "29.1", "Bloc protégé : les exceptions levées sont traitées par les `on` qui suivent."),
    ("on", "29.1", "Gestionnaire d'exception : `on e is FileException { ... }` (sans `is` : attrape tout, en dernier)."),
    ("raise", "29.3", "Lève une exception."),
    ("is", "29.2", "Test de type à l'exécution (`x is int`, `e is FileException`), filtre d'un `on`."),
    ("and", "11", "ET logique."),
    ("or", "11", "OU logique."),
    ("not", "11", "NON logique (aussi `not equal`)."),
    ("equal", "11.1", "Comparaison d'égalité (`a equal b`, `a not equal b`) — strictement typée."),
    ("smaller", "11.1", "Comparaison `a smaller b` (`smaller or equal`) — strictement typée."),
    ("greater", "11.1", "Comparaison `a greater b` (`greater or equal`) — strictement typée."),
];

/// Mots-clés aussi utilisables comme identifiants ordinaires (`var result:int`,
/// `e.message`…) : documentés seulement en position de mot-clé.
const CONTEXTUAL: &[&str] = &["result", "from", "message", "main", "error", "success", "exit", "init", "method", "emit", "default", "map", "array", "has"];

const MODIFIERS: &[&str] = &["public", "private", "protected", "static", "async", "is"];

/// Documentation du mot-clé `word`, entouré de `before`/`after` sur sa ligne.
pub fn doc(word: &str, before: &str, after: &str) -> Option<String> {
    let (_, section, summary) = KEYWORDS.iter().find(|(k, _, _)| *k == word)?;
    let after_trim = after.trim_start();
    if before.ends_with('.') || before.ends_with(':') || (after_trim.starts_with(':') && !after_trim.starts_with("::")) {
        return None;
    }
    if CONTEXTUAL.contains(&word) {
        let last = before.split_whitespace().last();
        let statement_start = before.trim().is_empty() || last.is_some_and(|w| MODIFIERS.contains(&w) && before.ends_with(char::is_whitespace));
        let as_keyword = (statement_start && !after_trim.starts_with(['.', '=', ':']))
            || ((word == "map" || word == "array") && after_trim.starts_with('<'))
            || (word == "default" && after_trim.starts_with("=>"));
        if !as_keyword {
            return None;
        }
    }
    let mut parts = vec![format!("```ocara\n{}\n```", word), summary.to_string()];
    if let Some(heading) = heading(section) {
        let label = heading.replace('`', "");
        parts.push(format!("[📖 EBNF §{}](ocara-doc:EBNF.md#{})", label, super::builtin_docs::percent_encode(&heading)));
    }
    Some(parts.join("\n\n"))
}

/// Titre de la section `number` de l'EBNF (`"9.2"` → `"9.2 Variable de bloc…"`).
fn heading(number: &str) -> Option<String> {
    static HEADINGS: OnceLock<Vec<String>> = OnceLock::new();
    let headings = HEADINGS.get_or_init(|| EBNF.lines()
        .filter(|l| l.starts_with("##") && l.trim_start_matches('#').starts_with(' ') && l.chars().take_while(|c| *c == '#').count() <= 4)
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .collect());
    headings.iter().find(|h| {
        let rest = h.strip_prefix(number);
        rest.is_some_and(|r| r.starts_with(' ') || r.starts_with(". "))
    }).cloned()
}
