// ─────────────────────────────────────────────────────────────────────────────
// Documentation au survol des mots-clés d'Ocara — une phrase par mot-clé et
// la section de docs/EBNF.md qui le décrit (numéro de section : le titre
// exact et son ancre sont relus dans l'EBNF embarqué, voir docs.ts, pour ne
// jamais diverger de la documentation réelle).
// ─────────────────────────────────────────────────────────────────────────────

export interface KeywordDoc {
    /** Numéro de section de docs/EBNF.md (`"9.2"`). */
    section: string;
    summary: string;
}

export const KEYWORDS: Record<string, KeywordDoc> = {
    // Structure, imports
    namespace:  { section: '3.1', summary: "Déclare le namespace du fichier (`namespace .` = racine), utilisé pour résoudre les imports." },
    import:     { section: '4.1', summary: "Importe une classe/fonction : `import ocara.IO` (builtin), `import a.b.Classe` (namespace) ou `import Classe from \"fichier\"`." },
    from:       { section: '4.1', summary: "Dans `import X from \"fichier\"` : fichier source du symbole importé." },
    as:         { section: '4.1', summary: "Alias d'import : `import ocara.Convert as C`." },
    runtime:    { section: '5.6', summary: "Importe un fichier de bloc runtime : `runtime core.main is main`." },
    main:       { section: '5.3', summary: "Bloc runtime principal du programme." },
    error:      { section: '5.3', summary: "Bloc runtime exécuté quand le programme se termine en erreur." },
    success:    { section: '5.3', summary: "Bloc runtime exécuté quand le programme se termine avec succès." },
    exit:       { section: '5.3', summary: "Bloc runtime exécuté à la fin du programme, dans tous les cas." },
    result:     { section: '5.5', summary: "Dans un bloc runtime : fixe le code de résultat du programme (`result SUCCESS`, `result 5`) — `return` y est interdit." },
    // Variables
    var:        { section: '9.1', summary: "Variable mutable. Sa valeur est comptée : libérée quand plus rien ne la référence." },
    scoped:     { section: '9.2', summary: "Variable de bloc : rend sa référence (ferme sa ressource) à la fin du bloc ; une ressource ne peut pas s'échapper du bloc." },
    consumed:   { section: '9.3', summary: "Variable à usage unique : rend sa référence (ferme sa ressource) juste après sa première utilisation." },
    const:      { section: '9.4', summary: "Constante (globale, locale ou de classe) ; valeur de constante de classe connue à la compilation." },
    // Types
    int:        { section: '6.1', summary: "Entier signé 64 bits." },
    float:      { section: '6.1', summary: "Flottant 64 bits." },
    string:     { section: '6.1', summary: "Chaîne de caractères (méthodes `String` en instance : `s.trim()`, conversions `s.toInt()`...)." },
    bool:       { section: '6.1', summary: "Booléen `true`/`false`." },
    mixed:      { section: '6.1', summary: "Type dynamique : désactive la vérification de type (préférer une union `int|string|null`)." },
    void:       { section: '6.1', summary: "Absence de valeur de retour." },
    array:      { section: '6.2', summary: "Tableau typé `array<T>` (méthodes `Array` en instance : `arr.len()`, `arr.push(x)`...)." },
    map:        { section: '6.2', summary: "Tableau associatif `map<K, V>` (méthodes `Map` en instance : `m.keys()`, `m.size()`...)." },
    variadic:   { section: '14.2', summary: "Paramètre variadic `nom:variadic<T>` (dernier paramètre) — reçoit un `array<T>`." },
    true:       { section: '7.1', summary: "Littéral booléen vrai." },
    false:      { section: '7.1', summary: "Littéral booléen faux." },
    null:       { section: '7.1', summary: "Absence de valeur (types nullables : `T|null`)." },
    // Fonctions
    function:   { section: '14', summary: "Déclare une fonction libre : `function nom(param:Type): Retour { ... }`." },
    nameless:   { section: '14.4', summary: "Fonction anonyme (closure) : `nameless(x:int): int { ... }`." },
    async:      { section: '14.5', summary: "Fonction/méthode asynchrone : son appel retourne un `Resolvable<T>`." },
    resolve:    { section: '14.5', summary: "Attend la fin d'une tâche `async` et retourne son résultat `T`." },
    return:     { section: '12', summary: "Retourne une valeur depuis une fonction/méthode (interdit dans un bloc runtime : `result`)." },
    emit:       { section: '28', summary: "Émet une valeur depuis un générateur (fonction retournant `message<T>`)." },
    message:    { section: '28.1', summary: "Type `message<T>` : retour d'un générateur (contient `emit`), consommé par `for x in gen()`." },
    // Classes
    class:      { section: '16', summary: "Déclare une classe (instances sur le tas via `use`)." },
    struct:     { section: '16.7', summary: "Agrégat de données : champs et constantes, constructeur généré depuis les champs ; n'étend qu'un struct." },
    generic:    { section: '20', summary: "Classe générique paramétrée par des types : `generic Liste<T> { ... }` (monomorphisée à la compilation)." },
    interface:  { section: '17', summary: "Contrat de méthodes, implémenté via `implements` ; peut désigner son implémentation avec `wiring`." },
    wiring:     { section: '17.1', summary: "Dans une interface : classe concrète liée à l'interface à la compilation (`use Interface()` → cette classe)." },
    module:     { section: '19.1', summary: "Mixin : membres réutilisables incorporés dans les classes qui l'utilisent (`modules`)." },
    modules:    { section: '19.2', summary: "Dans l'en-tête d'une classe : modules (mixins) incorporés." },
    extends:    { section: '18', summary: "Héritage simple d'une classe (ou d'un struct depuis un struct)." },
    implements: { section: '18', summary: "Interfaces implémentées par la classe." },
    init:       { section: '16.2', summary: "Constructeur de la classe, appelé par `use Classe(...)`." },
    method:     { section: '16.3', summary: "Déclare une méthode : `public [static] [async] method nom(...): Retour { ... }`." },
    property:   { section: '16.3', summary: "Déclare un champ d'instance : `public property nom:Type`." },
    static:     { section: '16.5', summary: "Méthode de classe, appelée via `Classe::methode()`." },
    public:     { section: '16.3', summary: "Visibilité : accessible depuis n'importe où." },
    private:    { section: '16.3', summary: "Visibilité : accessible uniquement depuis la classe qui déclare le membre (E54)." },
    protected:  { section: '16.3', summary: "Visibilité : accessible depuis la classe déclarante et ses descendantes (E54)." },
    self:       { section: '16.6', summary: "Instance courante (`self.champ`), ou la classe courante (`self::methode()`)." },
    parent:     { section: '18.1', summary: "Classe parente : `parent::methode()`, `parent::init(...)`." },
    use:        { section: '22', summary: "Instancie une classe/struct : `use Classe(args)` (arguments nommés acceptés)." },
    enum:       { section: '21', summary: "Énumération de constantes entières : `enum Couleur { Rouge, Vert }`." },
    // Contrôle
    if:         { section: '24', summary: "Condition : `if cond { ... } elseif cond { ... } else { ... }`." },
    elseif:     { section: '24', summary: "Branche alternative conditionnelle d'un `if`." },
    else:       { section: '24', summary: "Branche par défaut d'un `if`." },
    switch:     { section: '25', summary: "Branchement multiple sur une valeur." },
    default:    { section: '25', summary: "Cas par défaut d'un `switch`/`match`." },
    match:      { section: '26', summary: "Expression de filtrage, un bras par ligne : `1 => ...`, `is string => ...`, `default => ...`." },
    while:      { section: '27.1', summary: "Boucle tant que la condition est vraie." },
    for:        { section: '27.2', summary: "Boucle d'itération : `for x in tableau`, `for k has v in map`, `for i in 0..n`." },
    in:         { section: '27.2', summary: "Dans un `for` : la collection parcourue." },
    has:        { section: '27.3', summary: "Dans `for k has v in m` : sépare la clé de la valeur d'une map." },
    break:      { section: '27.5', summary: "Sort de la boucle courante." },
    continue:   { section: '27.6', summary: "Passe à l'itération suivante de la boucle courante." },
    // Exceptions
    try:        { section: '29.1', summary: "Bloc protégé : les exceptions levées sont traitées par les `on` qui suivent." },
    on:         { section: '29.1', summary: "Gestionnaire d'exception : `on e is FileException { ... }` (sans `is` : attrape tout, en dernier)." },
    raise:      { section: '29.3', summary: "Lève une exception." },
    is:         { section: '29.2', summary: "Test de type à l'exécution (`x is int`, `e is FileException`), filtre d'un `on`." },
    // Opérateurs
    and:        { section: '11', summary: "ET logique." },
    or:         { section: '11', summary: "OU logique." },
    not:        { section: '11', summary: "NON logique (aussi `not equal`)." },
    equal:      { section: '11.1', summary: "Comparaison d'égalité (`a equal b`, `a not equal b`) — strictement typée." },
    smaller:    { section: '11.1', summary: "Comparaison `a smaller b` (`smaller or equal`) — strictement typée." },
    greater:    { section: '11.1', summary: "Comparaison `a greater b` (`greater or equal`) — strictement typée." },
};

/**
 * Mots-clés aussi utilisables comme identifiants ordinaires (variable,
 * paramètre, méthode : `var result:int`, `e.message`, `method(...)` de
 * HTTPServer...) — documentés au survol seulement hors de ces positions.
 */
export const CONTEXTUAL_KEYWORDS = new Set(['result', 'from', 'message', 'main', 'error', 'success', 'exit', 'init', 'method', 'emit', 'default', 'map', 'array', 'has']);
