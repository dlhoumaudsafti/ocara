import { Document, isMap, isSeq, isNode, LineCounter, parseDocument, Scalar, YAMLMap, YAMLSeq } from 'yaml';

// ─────────────────────────────────────────────────────────────────────────────
// Modifications du designer appliquées au texte d'un `.oc.ui` : le document
// YAML est relu, modifié nœud par nœud (commentaires et style `{ … }` des
// nœuds gardés) puis réécrit ; seule la plage qui change est remplacée dans
// l'éditeur (minimalEdit), le reste du fichier reste octet pour octet.
// Un chemin désigne une valeur depuis la racine : ['screens', 'home',
// 'layout', 'children', 0].
// ─────────────────────────────────────────────────────────────────────────────

export type Path = (string | number)[];

export type Op =
    /** `value` absente : clé retirée. */
    | { op: 'set'; path: Path; value?: unknown }
    | { op: 'insert'; parent: Path; index: number; value: unknown }
    | { op: 'remove'; path: Path }
    | { op: 'move'; from: Path; parent: Path; index: number }
    /** Renomme la clé `from` de la table `path` en gardant sa place. */
    | { op: 'rename'; path: Path; from: string; to: string };

const OPTIONS = { lineWidth: 0, flowCollectionPadding: true, indentSeq: true } as const;

/** Clés de texte libre : valeurs neuves écrites entre guillemets, comme dans
 *  les fichiers d'exemple (`text: "Enregistrer"`), les noms (types, id,
 *  actions) restant nus. */
const TEXT_KEYS = new Set(['value', 'text', 'title', 'label', 'placeholder', 'tooltip', 'src', 'url', 'file', 'html', 'shortcut', 'userAgent']);

function quoteTexts(node: unknown, key?: unknown): void {
    if (isMap(node)) { node.items.forEach(p => quoteTexts(p.value, keyOf(p.key))); return; }
    if (isSeq(node)) { node.items.forEach(v => quoteTexts(v, key)); return; }
    if (node instanceof Scalar && typeof node.value === 'string' && TEXT_KEYS.has(String(key))) { node.type = 'QUOTE_DOUBLE'; }
}

export function parse(text: string): { doc: Document; lines: LineCounter } {
    const lines = new LineCounter();
    return { doc: parseDocument(text, { lineCounter: lines, uniqueKeys: false }), lines };
}

/** Nœud YAML d'une valeur neuve : un composant sans enfants s'écrit en
 *  table `{ … }` sur une ligne (style des fichiers d'exemple), le reste en
 *  blocs ; tout est sur une ligne dans une table déjà sur une ligne. */
function createNode(doc: Document, value: unknown, parentFlow: boolean, key?: unknown): unknown {
    const node = doc.createNode(value);
    quoteTexts(node, key);
    const layout = (v: unknown): void => {
        if (isMap(v)) {
            if (v.has('type') && !v.has('children') && !v.has('item')) { v.flow = true; return; }
            v.items.forEach(p => layout(p.value));
        } else if (isSeq(v)) {
            if (v.items.length === 0) { v.flow = true; }
            v.items.forEach(layout);
        }
    };
    if (parentFlow && (isMap(node) || isSeq(node))) { node.flow = true; } else { layout(node); }
    return node;
}

function sequence(doc: Document, parent: Path): YAMLSeq {
    let seq = doc.getIn(parent, true);
    if (!isSeq(seq)) {
        seq = new YAMLSeq();
        doc.setIn(parent, seq);
    }
    const holder = doc.getIn(parent.slice(0, -1), true);
    // Liste `[]` vide d'une table en blocs : repasse en blocs à la première insertion.
    if ((seq as YAMLSeq).flow && (seq as YAMLSeq).items.length === 0 && !(isMap(holder) && holder.flow)) {
        (seq as YAMLSeq).flow = false;
    }
    return seq as YAMLSeq;
}

function apply(doc: Document, op: Op): void {
    switch (op.op) {
        case 'set': {
            if (op.value === undefined) { doc.deleteIn(op.path); return; }
            const holder = doc.getIn(op.path.slice(0, -1), true);
            const current = doc.getIn(op.path, true);
            if (isNode(current) && current instanceof Scalar && (op.value === null || typeof op.value !== 'object')) {
                current.value = op.value; // garde le style de guillemets et le commentaire
            } else {
                // Types des paramètres d'un composant (`label: text`) : des noms, pas des textes.
                doc.setIn(op.path, createNode(doc, op.value, isMap(holder) && !!holder.flow, op.path.includes('params') ? undefined : op.path[op.path.length - 1]));
            }
            return;
        }
        case 'insert': {
            const seq = sequence(doc, op.parent);
            seq.items.splice(Math.min(op.index, seq.items.length), 0, createNode(doc, op.value, !!seq.flow) as any);
            return;
        }
        case 'remove':
            doc.deleteIn(op.path);
            return;
        case 'move': {
            const node = doc.getIn(op.from, true);
            if (node === undefined) { return; }
            const fromParent = op.from.slice(0, -1);
            const fromIndex = op.from[op.from.length - 1] as number;
            let index = op.index;
            if (same(fromParent, op.parent) && fromIndex < index) { index--; }
            doc.deleteIn(op.from);
            const seq = sequence(doc, adjust(op.parent, op.from));
            seq.items.splice(Math.min(index, seq.items.length), 0, node as any);
            return;
        }
        case 'rename': {
            const map = doc.getIn(op.path, true);
            if (!isMap(map)) { return; }
            const pair = (map as YAMLMap).items.find(p => keyOf(p.key) === op.from);
            if (pair) { pair.key = doc.createNode(op.to); }
            return;
        }
    }
}

function keyOf(key: unknown): unknown {
    return key instanceof Scalar ? key.value : key;
}

function same(a: Path, b: Path): boolean {
    return a.length === b.length && a.every((k, i) => k === b[i]);
}

/** Chemin `target` une fois `removed` retiré de sa liste (rangs suivants décalés). */
export function adjust(target: Path, removed: Path): Path {
    const depth = removed.length - 1;
    if (target.length <= depth || !same(target.slice(0, depth), removed.slice(0, depth))) { return target; }
    const at = target[depth];
    const gone = removed[depth];
    if (typeof at === 'number' && typeof gone === 'number' && at > gone) {
        return [...target.slice(0, depth), at - 1, ...target.slice(depth + 1)];
    }
    return target;
}

/** Retire l'espace intérieur des listes `[ … ]` sur une ligne ajoutée par
 *  `flowCollectionPadding` (gardé pour les tables `{ … }`), hors chaînes. */
function unpadSequences(line: string): string {
    let out = '';
    let quote = '';
    const stack: string[] = [];
    for (let i = 0; i < line.length; i++) {
        const c = line[i];
        if (quote) {
            if (c === '\\' && quote === '"') { out += c + (line[++i] ?? ''); continue; }
            if (c === quote) { quote = ''; }
            out += c;
            continue;
        }
        if (c === '#' && (i === 0 || line[i - 1] === ' ')) { return out + line.slice(i); }
        if ((c === '"' || c === '\'') && /[\s[{,:]/.test(line[i - 1] ?? ' ')) { quote = c; out += c; continue; }
        const before = out.trimEnd().slice(-1);
        if ((c === '[' || c === '{') && (before === '' || ':-,[{'.includes(before))) {
            stack.push(c);
            out += c;
            if (c === '[' && line[i + 1] === ' ') { i++; }
            continue;
        }
        if ((c === ']' || c === '}') && stack.length) {
            if (stack.pop() === '[' && out.endsWith(' ')) { out = out.slice(0, -1); }
        }
        out += c;
    }
    return out;
}

/** Texte après les modifications, ou l'erreur YAML qui les empêche. */
export function applyOps(text: string, ops: Op[]): { text: string } | { error: string } {
    const { doc } = parse(text);
    if (doc.errors.length) { return { error: doc.errors[0].message }; }
    for (const op of ops) { apply(doc, op); }
    const out = doc.toString(OPTIONS);
    const old = text.split('\n');
    return { text: out.split('\n').map(l => (old.includes(l) ? l : unpadSequences(l))).join('\n') };
}

/** Plus petite plage à remplacer pour passer de `a` à `b`. */
export function minimalEdit(a: string, b: string): { start: number; end: number; text: string } {
    let start = 0;
    while (start < a.length && start < b.length && a[start] === b[start]) { start++; }
    let endA = a.length;
    let endB = b.length;
    while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) { endA--; endB--; }
    return { start, end: endA, text: b.slice(start, endB) };
}

/** Chemin du nœud le plus profond qui couvre la position `offset`. */
export function pathAt(doc: Document, offset: number): Path {
    const path: Path = [];
    let node: unknown = doc.contents;
    for (;;) {
        let next: { key: string | number; value: unknown } | undefined;
        if (isMap(node)) {
            for (const pair of node.items) {
                const v = pair.value as any;
                const k = pair.key as any;
                const from = k?.range?.[0] ?? v?.range?.[0];
                const to = v?.range?.[2] ?? k?.range?.[2];
                if (from !== undefined && to !== undefined && offset >= from && offset <= to) {
                    next = { key: String(keyOf(pair.key)), value: v };
                }
            }
        } else if (isSeq(node)) {
            node.items.forEach((v: any, i) => {
                if (v?.range && offset >= v.range[0] && offset <= v.range[2]) { next = { key: i, value: v }; }
            });
        }
        if (!next) { return path; }
        path.push(next.key);
        node = next.value;
    }
}
