// Modèle du designer : état partagé, chemins dans l'arbre YAML, opérations
// (envoyées à l'extension, qui les applique au texte : src/uidesigner/yamlops.ts,
// et appliquées ici aussitôt pour un affichage immédiat).

export const vscode = acquireVsCodeApi();

export const S = {
    catalog: null, icons: {}, base: '',
    model: null, file: '', includes: [], variants: [], problems: [], error: undefined,
    screen: '', selection: null, platform: 'tauri', theme: 'light', width: 360,
    tab: 'props', clipboard: null, collapsed: {},
    /** Fichier inclus : fichier racine qui l'inclut (`root`), ses `styles`, `window`, `menu`. */
    context: null,
};

const listeners = [];
export function onChange(fn) { listeners.push(fn); }
export function refresh() { listeners.forEach(fn => fn()); }

// ── chemins ────────────────────────────────────────────────────────────────

export const key = path => JSON.stringify(path);
export const same = (a, b) => !!a && !!b && key(a) === key(b);
export const startsWith = (path, prefix) => prefix.length <= path.length && prefix.every((k, i) => path[i] === k);
export function getIn(obj, path) { return path.reduce((o, k) => (o == null ? undefined : o[k]), obj); }

/** Chemin `target` une fois `removed` retiré de sa liste (même règle que yamlops.adjust). */
export function adjust(target, removed) {
    const depth = removed.length - 1;
    if (target.length <= depth || !startsWith(target, removed.slice(0, depth))) { return target; }
    const at = target[depth];
    const gone = removed[depth];
    if (typeof at === 'number' && typeof gone === 'number' && at > gone) {
        return [...target.slice(0, depth), at - 1, ...target.slice(depth + 1)];
    }
    return target;
}

// ── catalogue ──────────────────────────────────────────────────────────────

export const typeSpec = type => S.catalog.types.find(t => t.type === type);
export const isContainer = node => !!node && !!typeSpec(node.type)?.own.some(p => p.kind === 'children');
export const hasSlot = node => !!node && !!typeSpec(node.type)?.own.some(p => p.kind === 'node');
export const axisOf = node => (node.type === 'row' ? 'row' : node.type === 'grid' ? 'grid' : node.type === 'stack' ? 'stack' : 'column');

/** Toutes les propriétés d'un type : propres, communes, présentation. */
export function allProps(type) {
    const spec = typeSpec(type);
    return spec ? [...spec.own, ...S.catalog.common, ...S.catalog.style] : [...S.catalog.common, ...S.catalog.style];
}

// ── fichier racine (écrans inclus) ─────────────────────────────────────────

/** Styles visibles : ceux du fichier racine, puis ceux du fichier. */
export function styles() { return { ...(S.context?.styles || {}), ...(S.model?.styles || {}) }; }
export function windowProps() { return S.model?.window || S.context?.window || {}; }

// ── écrans ─────────────────────────────────────────────────────────────────

/** Écrans du fichier : `screens.<nom>`, ou `layout` racine (écran `main`). */
export function screens() {
    const m = S.model || {};
    if (m.screens && typeof m.screens === 'object') { return Object.keys(m.screens).filter(s => s !== 'includes'); }
    return m.layout ? ['main'] : [];
}
export function rootLayout() { return !!S.model && !S.model.screens && !!S.model.layout; }
export function screenPath(name = S.screen) { return rootLayout() ? [] : ['screens', name]; }
export function layoutPath(name = S.screen) { return [...screenPath(name), 'layout']; }
export function startScreen() { return S.model?.start || screens()[0]; }

// ── recherche ──────────────────────────────────────────────────────────────

/** Parcourt chaque nœud de l'interface : fn(node, path). */
export function walk(fn) {
    const visit = (node, path) => {
        if (!node || typeof node !== 'object' || Array.isArray(node)) { return; }
        fn(node, path);
        (Array.isArray(node.children) ? node.children : []).forEach((c, i) => visit(c, [...path, 'children', i]));
        if (node.item) { visit(node.item, [...path, 'item']); }
    };
    if (rootLayout()) { visit(S.model.layout, ['layout']); }
    for (const s of screens()) { if (!rootLayout()) { visit(S.model.screens[s]?.layout, ['screens', s, 'layout']); } }
}

export function ids() { const out = new Set(); walk(n => { if (n.id) { out.add(String(n.id)); } }); return out; }

/** Actions nommées dans le fichier, plus `back` et `close` (prises en charge par le moteur). */
export function actions() {
    const out = new Set(['back', 'close']);
    const scan = v => {
        if (Array.isArray(v)) { v.forEach(scan); return; }
        if (!v || typeof v !== 'object') { return; }
        for (const [k, x] of Object.entries(v)) {
            if ((/^on[A-Z]/.test(k) || k === 'action') && typeof x === 'string') { out.add(x); }
            if (k === 'webHandlers' && x && typeof x === 'object') { Object.values(x).forEach(a => typeof a === 'string' && out.add(a)); }
            scan(x);
        }
    };
    scan(S.model);
    return [...out].sort();
}

export function uniqueId(base) {
    const taken = ids();
    let n = 1;
    while (taken.has(`${base}${n}`)) { n++; }
    return `${base}${n}`;
}

/** Le nœud est-il dans un gabarit d'élément de liste (seul lieu de `bind`) ? */
export const inItem = path => path.includes('item');

// ── composants neufs ───────────────────────────────────────────────────────

export function template(type) {
    const id = base => uniqueId(base);
    switch (type) {
        case 'column': case 'row': case 'stack': case 'card': return { type, spacing: 8, children: [] };
        case 'grid': return { type, columns: 2, spacing: 8, children: [] };
        case 'text': return { type, value: 'Texte' };
        case 'heading': return { type, value: 'Titre', level: 2 };
        case 'image': return { type, src: '', label: 'Image' };
        case 'icon': return { type, name: 'star' };
        case 'progress': return { type, value: 0.5 };
        case 'list': return { type, id: id('list'), item: { type: 'text', bind: 'name' } };
        case 'webview': return { type, id: id('web'), url: 'https://example.com' };
        case 'input': return { type, id: id('input'), placeholder: 'Saisir…' };
        case 'checkbox': case 'switch': return { type, id: id(type), label: 'Option' };
        case 'radio': case 'select': return { type, id: id(type), options: [{ value: 'a', text: 'Choix A' }, { value: 'b', text: 'Choix B' }] };
        case 'slider': return { type, id: id('slider'), min: 0, max: 100, value: 50 };
        case 'date': case 'time': return { type, id: id(type) };
        case 'button': return { type, text: 'Bouton', variant: 'primary' };
        case 'iconButton': return { type, icon: 'add', label: 'Ajouter' };
        case 'android.fab': return { type, icon: 'add', label: 'Ajouter' };
        case 'android.bottomSheet': return { type, id: id('sheet'), children: [] };
        case 'android.drawer': return { type, id: id('drawer'), children: [] };
        default: return { type };
    }
}

/** Copie d'un nœud dont les `id` sont rendus uniques. */
export function cloneFresh(node) {
    const taken = ids();
    const copy = JSON.parse(JSON.stringify(node));
    const visit = n => {
        if (!n || typeof n !== 'object') { return; }
        if (n.id) {
            const base = String(n.id).replace(/_?\d+$/, '');
            let i = 2;
            while (taken.has(`${base}_${i}`)) { i++; }
            n.id = `${base}_${i}`;
            taken.add(n.id);
        }
        (n.children || []).forEach(visit);
        if (n.item) { visit(n.item); }
    };
    visit(copy);
    return copy;
}

// ── opérations ─────────────────────────────────────────────────────────────

function setLocal(path, value) {
    let o = S.model;
    for (let i = 0; i < path.length - 1; i++) {
        if (o[path[i]] == null || typeof o[path[i]] !== 'object') { o[path[i]] = typeof path[i + 1] === 'number' ? [] : {}; }
        o = o[path[i]];
    }
    const last = path[path.length - 1];
    if (value === undefined) {
        if (Array.isArray(o)) { o.splice(last, 1); } else { delete o[last]; }
    } else {
        o[last] = value;
    }
}

function applyLocal(op) {
    const clone = v => (v === undefined ? v : JSON.parse(JSON.stringify(v)));
    switch (op.op) {
        case 'set': setLocal(op.path, clone(op.value)); break;
        case 'insert': {
            if (!Array.isArray(getIn(S.model, op.parent))) { setLocal(op.parent, []); }
            getIn(S.model, op.parent).splice(op.index, 0, clone(op.value));
            break;
        }
        case 'remove': setLocal(op.path, undefined); break;
        case 'move': {
            const node = getIn(S.model, op.from);
            const fromParent = op.from.slice(0, -1);
            let index = op.index;
            if (same(fromParent, op.parent) && op.from[op.from.length - 1] < index) { index--; }
            setLocal(op.from, undefined);
            const parent = adjust(op.parent, op.from);
            if (!Array.isArray(getIn(S.model, parent))) { setLocal(parent, []); }
            getIn(S.model, parent).splice(index, 0, node);
            break;
        }
        case 'rename': {
            const map = getIn(S.model, op.path);
            if (!map) { break; }
            const entries = Object.entries(map).map(([k, v]) => [k === op.from ? op.to : k, v]);
            Object.keys(map).forEach(k => delete map[k]);
            entries.forEach(([k, v]) => { map[k] = v; });
            break;
        }
    }
}

/** Applique `ops` (ici et dans le document) ; `select` : nouvelle sélection. */
export function edit(ops, select) {
    // YAML invalide : le document ne peut pas être modifié avant correction dans le source.
    if (!S.model || S.error) { return; }
    // Valeurs sans clés `undefined` (entrées de menu, choix…) : ni ici ni dans le YAML.
    ops = ops.map(o => (o.value !== undefined ? { ...o, value: JSON.parse(JSON.stringify(o.value)) } : o));
    ops.forEach(applyLocal);
    if (select !== undefined) { S.selection = select; }
    vscode.postMessage({ type: 'edit', ops });
    refresh();
}

/** Insère `node` dans la liste `parent` au rang `index` et le sélectionne. */
export function insertNode(parent, index, node) {
    edit([{ op: 'insert', parent, index, value: node }], [...parent, index]);
}

/** Déplace le nœud `from` dans la liste `parent` au rang `index` (rangs d'avant le déplacement). */
export function moveNode(from, parent, index) {
    let at = index;
    if (same(from.slice(0, -1), parent) && from[from.length - 1] < index) { at--; }
    if (same(from.slice(0, -1), parent) && from[from.length - 1] === at) { return; }
    edit([{ op: 'move', from, parent, index }], [...adjust(parent, from), at]);
}

/** Place un nœud dans un emplacement unique (`layout` d'un écran, `item` d'une liste). */
export function placeNode(slot, node, from) {
    if (from) {
        const target = adjust(slot, from);
        edit([{ op: 'set', path: slot, value: node }, { op: 'remove', path: from }], target);
    } else {
        edit([{ op: 'set', path: slot, value: node }], slot);
    }
}

export function removeNode(path) {
    const last = path[path.length - 1];
    if (typeof last === 'number') {
        edit([{ op: 'remove', path }], path.slice(0, -2).length ? path.slice(0, -2) : null);
    } else {
        edit([{ op: 'remove', path }], path.length > 1 ? path.slice(0, -1) : null);
    }
}

/** Propriété `prop` du nœud `path` (`undefined` : retirée). */
export function setProp(path, prop, value) {
    edit([{ op: 'set', path: [...path, prop], value }]);
}
