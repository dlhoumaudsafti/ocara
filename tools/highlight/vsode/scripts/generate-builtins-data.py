#!/usr/bin/env python3
"""Extrait le catalogue des classes builtins Ocara (méthodes + constantes)
depuis src/builtins/*.rs — source de vérité utilisée par le compilateur lui
même — pour générer les données d'autocomplétion de l'extension VS Code."""
import re, json, pathlib

# Racine du dépôt Ocara, déduite de l'emplacement du script
# (tools/highlight/vsode/scripts/) plutôt que d'un chemin absolu figé.
EXT_DIR = pathlib.Path(__file__).resolve().parents[1]
ROOT = EXT_DIR.parents[2]
BUILTINS_DIR = ROOT / "src/builtins"

# (ClassName, fichier, fonction) — depuis src/builtins/mod.rs::all_builtins()
CLASSES = [
    ("IO", "io", "class"),
    ("System", "system", "class"),
    ("Thread", "thread", "class"),
    ("Mutex", "mutex", "class"),
    ("Math", "math", "class"),
    ("String", "string", "class"),
    ("Array", "array", "class"),
    ("Map", "map", "class"),
    ("Regex", "regex", "class"),
    ("Convert", "convert", "class"),
    ("DateTime", "datetime", "class"),
    ("Date", "date", "class"),
    ("Time", "time", "class"),
    ("File", "file", "class"),
    ("Directory", "directory", "class"),
    ("JSON", "json", "class"),
    ("HTTPRequest", "httprequest", "class"),
    ("HTTPServer", "httpserver", "class"),
    ("HTTPServerRequest", "httpserver", "request_class"),
    ("Tauri", "tauri", "tauri_class"),
    ("SDL", "sdl", "sdl_class"),
    ("HTML", "html", "class"),
    ("HTMLComponent", "htmlcomponent", "class"),
    ("UnitTest", "unittest", "class"),
    ("SQLite", "sqlite", "class"),
    ("MySQL", "mysql", "class"),
    ("DotEnv", "dotenv", "class"),
    ("YAML", "yaml", "class"),
]

def find_matching_paren(s, open_idx):
    """s[open_idx] == '('. Retourne l'index de la ')' correspondante."""
    depth = 0
    i = open_idx
    while i < len(s):
        if s[i] == '(':
            depth += 1
        elif s[i] == ')':
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1

def split_top_level_args(s):
    """Découpe une liste d'arguments Rust `a, b, c` en respectant les
    parenthèses/crochets imbriqués."""
    args, depth, cur = [], 0, ""
    for ch in s:
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            depth -= 1
        if ch == ',' and depth == 0:
            args.append(cur.strip())
            cur = ""
        else:
            cur += ch
    if cur.strip():
        args.append(cur.strip())
    return args

def extract_param_tuples(vec_arg):
    """Extrait les tuples ("name", TypeExpr) d'un texte `vec_arg` (contenu
    entre les crochets de vec![...]), en respectant les parenthèses imbriquées
    de TypeExpr (ex: Type::Array(Box::new(Type::Int))) et les virgules
    finales avant `]`."""
    out = []
    for m in re.finditer(r'\(\s*"([a-zA-Z_]\w*)"\s*,', vec_arg):
        pname = m.group(1)
        # m.end() pointe juste après la virgule suivant le nom ; on repart de
        # l'ouvrante du tuple pour trouver sa fermante correspondante.
        open_idx = vec_arg.rfind('(', 0, m.end())
        close_idx = find_matching_paren(vec_arg, open_idx)
        if close_idx == -1:
            continue
        type_expr = vec_arg[m.end():close_idx].strip()
        out.append((pname, type_expr))
    return out

def simplify_type_text(t, local_vars):
    # Aplatit sur une seule ligne + retire les virgules finales avant )/] :
    # nécessaire pour les types multi-lignes imbriqués (ex: MySQL::query).
    t = re.sub(r'\s+', '', t)
    t = re.sub(r'\.clone\(\)', '', t)
    for _ in range(4):
        t2 = re.sub(r',\s*([\)\]])', r'\1', t)
        if t2 == t:
            break
        t = t2
    if t in local_vars:
        return simplify_type_text(local_vars[t], local_vars)
    # Type::Array(Box::new(X)) -> X[]
    m = re.fullmatch(r'Type::Array\(Box::new\((.+)\)\)', t)
    if m:
        return simplify_type_text(m.group(1), local_vars) + "[]"
    # Type::Map(Box::new(K), Box::new(V)) -> map<K, V>
    m = re.fullmatch(r'Type::Map\(Box::new\((.+)\),\s*Box::new\((.+)\)\)', t)
    if m:
        return f"map<{simplify_type_text(m.group(1), local_vars)}, {simplify_type_text(m.group(2), local_vars)}>"
    # Type::Union(vec![A, B, ...]) -> A|B|...
    m = re.fullmatch(r'Type::Union\(vec!\[(.+)\]\)', t)
    if m:
        parts = split_top_level_args(m.group(1))
        return "|".join(simplify_type_text(p, local_vars) for p in parts)
    # Type::Function { ret_ty: Box::new(R), param_tys: vec![P, ...] } -> Function<R(P, ...)>
    m = re.fullmatch(r'Type::Function\s*\{\s*ret_ty:\s*Box::new\((.+?)\),\s*param_tys:\s*vec!\[(.*)\]\s*\}', t)
    if m:
        ret = simplify_type_text(m.group(1), local_vars)
        param_parts = split_top_level_args(m.group(2)) if m.group(2).strip() else []
        params_s = ", ".join(simplify_type_text(p, local_vars) for p in param_parts)
        return f"Function<{ret}({params_s})>"
    # Type::Named("X".into()) / Type::Named("X".to_string())
    m = re.fullmatch(r'Type::Named\("(\w+)"\.\w+\(\)\)', t)
    if m:
        return m.group(1)
    m = re.fullmatch(r'Type::(\w+)', t)
    if m:
        return {"String": "string", "Int": "int", "Float": "float", "Bool": "bool",
                "Void": "void", "Mixed": "mixed", "Null": "null"}.get(m.group(1), m.group(1).lower())
    return t  # fallback : expression brute (rare, cas complexes)

def find_matching_brace(s, open_idx):
    """Comme find_matching_paren, mais pour une accolade `{` — utilisé pour
    isoler le corps d'UNE fonction précise dans un fichier qui en contient
    plusieurs (ex: httpserver.rs, qui déclare `class()` ET `request_class()`)."""
    depth = 0
    i = open_idx
    while i < len(s):
        if s[i] == '{':
            depth += 1
        elif s[i] == '}':
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1

def function_body(text, fn_name):
    """Isole le corps `{ ... }` de `pub fn {fn_name}(...) -> ClassInfo { ... }`
    dans `text` — nécessaire dès qu'un fichier `src/builtins/<mod>.rs` déclare
    PLUSIEURS classes (ex: `httpserver.rs` : `class()` pour HTTPServer ET
    `request_class()` pour HTTPServerRequest, comme `httprequest.rs` le fait
    déjà pour `class()`/`response_class()`) : sans cette délimitation, un scan
    sur `text` entier confondrait les méthodes des deux classes. Retourne
    `text` inchangé si `fn_name` n'est pas trouvé (comportement historique —
    un seul `pub fn class()` par fichier, toujours vrai pour la majorité des
    modules builtins)."""
    m = re.search(r'\bfn\s+' + re.escape(fn_name) + r'\s*\([^)]*\)[^{]*\{', text)
    if not m:
        return text
    open_idx = m.end() - 1
    close_idx = find_matching_brace(text, open_idx)
    if close_idx == -1:
        return text
    return text[open_idx:close_idx + 1]

def parse_file(class_name, mod_name, fn_name):
    path = BUILTINS_DIR / f"{mod_name}.rs"
    full_text = path.read_text()
    # `methods.insert`/`class_consts.insert` sont cherchés UNIQUEMENT dans le
    # corps de `fn_name` (voir sa doc) ; les helpers de type (`local_vars`
    # ci-dessous) restent cherchés dans le FICHIER ENTIER — ce sont des
    # fonctions/`let` top-level partagées par toutes les classes du fichier
    # (ex: `req_ty()` utilisée à la fois par `class()` et `request_class()`).
    text = function_body(full_text, fn_name)

    # ── Variables locales de type (ex: let str_arr = Type::Array(...);) ──────
    local_vars = {}
    for lm in re.finditer(r'^\s*let\s+(\w+)\s*=\s*(Type::[^;]+);', full_text, re.M):
        local_vars[lm.group(1)] = lm.group(2).strip()
    # ── Helpers de type sans paramètre (ex: fn req_ty() -> Type { Type::Named(...) })
    # — même rôle qu'un `let`, mais écrit comme une fonction réutilisable
    # entre PLUSIEURS `ClassInfo` du même fichier (voir httprequest.rs :
    # `req_ty()`/`res_ty()` utilisées par `class()` ET implicitement par les
    # signatures qui prennent un HTTPResponse). Corps à une seule expression
    # `{ EXPR }` uniquement (largement suffisant en pratique) — un corps plus
    # complexe retombe sur le fallback "texte brut" de `simplify_type_text`,
    # comme avant l'ajout de cette résolution.
    for fm in re.finditer(r'fn\s+(\w+)\s*\(\s*\)\s*->\s*Type\s*\{\s*(Type::[^;]+?);?\s*\}', full_text):
        local_vars[fm.group(1) + "()"] = fm.group(2).strip()

    # ── Fonctions helper -> is_static fixe (définies au niveau du FICHIER,
    # partagées par class()/request_class() — voir la doc de function_body) ──
    helper_static = {}
    for hm in re.finditer(r'fn\s+(\w+)\s*\(.*?\)\s*->\s*FuncSig\s*\{(.*?)\n\}', full_text, re.S):
        body = hm.group(2)
        sm = re.search(r'is_static:\s*(true|false)', body)
        if sm:
            helper_static[hm.group(1)] = (sm.group(1) == "true")

    # ── methods.insert("name".into(), HELPER( ... )); ────────────────────────
    methods = []
    for im in re.finditer(r'methods\.insert\(\s*"(\w+)"\.(?:into|to_string)\(\)\s*,\s*(\w+)\(', text):
        method_name = im.group(1)
        helper = im.group(2)
        open_paren_idx = im.end() - 1
        close_idx = find_matching_paren(text, open_paren_idx)
        if close_idx == -1:
            continue
        inner = text[open_paren_idx + 1:close_idx]
        top_args = split_top_level_args(inner)
        params = []
        ret_ty = "mixed"
        if top_args:
            vec_arg = top_args[0]
            for pname, ptype_expr in extract_param_tuples(vec_arg):
                params.append({"name": pname, "type": simplify_type_text(ptype_expr, local_vars)})
            if len(top_args) >= 2:
                ret_ty = simplify_type_text(top_args[1], local_vars)
        is_static = helper_static.get(helper, True)
        methods.append({
            "name": method_name,
            "params": params,
            "returns": ret_ty,
            "static": is_static,
        })

    # ── class_consts.insert("NAME".into(), (Type::X, Visibility::Y)); ────────
    consts = []
    for cm in re.finditer(r'class_consts\.insert\(\s*"(\w+)"\.into\(\)\s*,\s*\(\s*(Type::\w+)\s*,\s*Visibility::(\w+)\s*\)', text):
        if cm.group(3) != "Public":
            continue
        consts.append({"name": cm.group(1), "type": simplify_type_text(cm.group(2), local_vars)})

    # ── Constructeur : `use ClassName(...)` — repéré via is_static:false sur "init" ou via convention new()/open()/connect() déjà couverte par methods
    return {
        "name": class_name,
        "methods": sorted(methods, key=lambda m: m["name"]),
        "consts": sorted(consts, key=lambda c: c["name"]),
    }

DOCS_DIR = ROOT / "docs/builtins"
DOC_MAX_CHARS = 1500
HEADING_RE = re.compile(r'^(#{1,4})\s+(.*)$')
# `Classe::methode(`, `objet.methode(` ou `methode(` entre backticks
HEADING_METHOD_RE = re.compile(r'`(?:([A-Za-z_]\w*)(::|\.))?([A-Za-z_]\w*)\s*\(')
TABLE_ROW_RE = re.compile(r'^\|([^|]*`[^|]*)\|(.*)\|\s*$')
TABLE_NAME_RE = re.compile(r'^(?:([A-Za-z_]\w*)(::|\.))?([A-Za-z_]\w*)\s*(?:\(|$)')

CODE_CALL_RE = r'(?:\.|::){}\s*\('

def method_class(owner, sep, current):
    """`Classe::m` désigne Classe ; `objet.m`/`m` la classe documentée en cours."""
    return owner if owner and sep == '::' and owner[0].isupper() else current

def extract_docs():
    """Documentation markdown par (classe, méthode), depuis docs/builtins/*.md.

    Deux formes reconnues : une section dont le titre contient un appel entre
    backticks (corps = jusqu'au titre suivant de niveau <= ou un `---`), et
    une ligne de tableau dont la première cellule est un nom de méthode (corps
    = cellules suivantes). Un titre `ocara.Nom` change la classe en cours
    (ex. HTTPServer.md documente aussi HTTPServerRequest). Le premier texte
    trouvé pour une méthode l'emporte (titres avant tableaux)."""
    docs, table_docs = {}, {}
    for md in sorted(DOCS_DIR.glob("*.md")):
        current = md.stem
        lines = md.read_text(encoding="utf-8").split("\n")
        for i, line in enumerate(lines):
            hm = HEADING_RE.match(line)
            if hm:
                level, title = len(hm.group(1)), hm.group(2)
                switch = re.search(r'ocara\.([A-Z]\w*)', title)
                if switch and level <= 2:
                    current = switch.group(1)
                mm = HEADING_METHOD_RE.search(title)
                if not mm:
                    continue
                key = (method_class(mm.group(1), mm.group(2), current), mm.group(3))
                body = []
                for nxt in lines[i + 1:]:
                    nh = HEADING_RE.match(nxt)
                    if (nh and len(nh.group(1)) <= level) or nxt.strip() == '---':
                        break
                    body.append(nxt)
                text = (title.strip() + "\n\n" + "\n".join(body).strip()).strip()
                docs.setdefault(key, text[:DOC_MAX_CHARS])
                continue
            tm = TABLE_ROW_RE.match(line)
            if tm:
                # Première cellule : un ou plusieurs noms entre backticks
                # (`getTitle()` / `setTitle(title:string)`).
                first = tm.group(1).strip()
                cells = [c.strip() for c in tm.group(2).split("|") if c.strip()]
                if not cells:
                    continue
                for code in re.findall(r'`([^`]+)`', first):
                    nm = TABLE_NAME_RE.match(code.strip())
                    if nm:
                        key = (method_class(nm.group(1), nm.group(2), current), nm.group(3))
                        table_docs.setdefault(key, first + " — " + " — ".join(cells))
    for key, text in table_docs.items():
        docs.setdefault(key, text)
    return docs

def example_doc(class_name, method_name):
    """Repli pour une méthode sans section ni ligne de tableau : première ligne
    d'exemple de code qui l'appelle (avec son commentaire), dans la doc de sa
    classe d'abord — `server.workers(32)  // threads workers (défaut : 4)`."""
    own = DOCS_DIR / f"{class_name}.md"
    candidates = ([own] if own.exists() else []) + [f for f in sorted(DOCS_DIR.glob("*.md")) if f != own]
    call = re.compile(CODE_CALL_RE.format(re.escape(method_name)))
    for md in candidates:
        in_code = False
        for line in md.read_text(encoding="utf-8").split("\n"):
            if line.strip().startswith("```"):
                in_code = not in_code
                continue
            if in_code and call.search(line):
                return "Exemple (docs/builtins/" + md.name + ") :\n\n```ocara\n" + line.strip() + "\n```"
    return None

def main():
    catalog = []
    for class_name, mod_name, fn_name in CLASSES:
        catalog.append(parse_file(class_name, mod_name, fn_name))

    docs = extract_docs()
    documented = 0
    for c in catalog:
        for m in c["methods"]:
            doc = docs.get((c["name"], m["name"])) or example_doc(c["name"], m["name"])
            if doc:
                m["doc"] = doc
                documented += 1

    total_methods = sum(len(c["methods"]) for c in catalog)
    total_consts = sum(len(c["consts"]) for c in catalog)
    print(f"{len(catalog)} classes, {total_methods} méthodes ({documented} documentées), {total_consts} constantes")
    for c in catalog:
        static_n = sum(1 for m in c["methods"] if m["static"])
        inst_n = len(c["methods"]) - static_n
        print(f"  {c['name']:15s} {len(c['methods']):3d} méthodes ({static_n} statiques, {inst_n} instance), {len(c['consts'])} const")

    out = EXT_DIR / "data/builtins-data.json"
    out.write_text(json.dumps(catalog, indent=2, ensure_ascii=False) + "\n")
    print(f"\nÉcrit -> {out}")

if __name__ == "__main__":
    main()
