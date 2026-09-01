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
    ("Tauri", "tauri", "tauri_class"),
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

def parse_file(class_name, mod_name, fn_name):
    path = BUILTINS_DIR / f"{mod_name}.rs"
    text = path.read_text()

    # ── Variables locales de type (ex: let str_arr = Type::Array(...);) ──────
    local_vars = {}
    for lm in re.finditer(r'^\s*let\s+(\w+)\s*=\s*(Type::[^;]+);', text, re.M):
        local_vars[lm.group(1)] = lm.group(2).strip()

    # ── Fonctions helper -> is_static fixe ────────────────────────────────────
    helper_static = {}
    for hm in re.finditer(r'fn\s+(\w+)\s*\(.*?\)\s*->\s*FuncSig\s*\{(.*?)\n\}', text, re.S):
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

def main():
    catalog = []
    for class_name, mod_name, fn_name in CLASSES:
        catalog.append(parse_file(class_name, mod_name, fn_name))

    total_methods = sum(len(c["methods"]) for c in catalog)
    total_consts = sum(len(c["consts"]) for c in catalog)
    print(f"{len(catalog)} classes, {total_methods} méthodes, {total_consts} constantes")
    for c in catalog:
        static_n = sum(1 for m in c["methods"] if m["static"])
        inst_n = len(c["methods"]) - static_n
        print(f"  {c['name']:15s} {len(c['methods']):3d} méthodes ({static_n} statiques, {inst_n} instance), {len(c['consts'])} const")

    out = EXT_DIR / "data/builtins-data.json"
    out.write_text(json.dumps(catalog, indent=2, ensure_ascii=False) + "\n")
    print(f"\nÉcrit -> {out}")

if __name__ == "__main__":
    main()
