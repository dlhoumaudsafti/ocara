// Composants déclarés (`components:` du `.oc.ui` et des fichiers de
// l'interface) : développement d'une instance pour l'aperçu (même règle que
// le compilateur, src/core/ui_schema.d/components.rs), gabarit d'une
// instance neuve, panneau des paramètres, conversion d'un nœud en composant.

import { S, getIn, edit, refresh, vscode } from './model.js';
export { editedComponent } from './model.js';
import { h, editor } from './editors.js';
import { ask, confirmBox } from './dialog.js';

/** Types de paramètres et genre d'éditeur (editors.js). */
export const PARAM_KINDS = { text: 'text', number: 'number', bool: 'bool', color: 'color', icon: 'icon', action: 'action', style: 'style', length: 'length' };

/** Composants du fichier (modifiables ici). */
export function ownComponents() {
    const all = S.model?.components;
    return all && typeof all === 'object' ? Object.keys(all).filter(n => n !== 'includes') : [];
}

/** Composants visibles : ceux des autres fichiers de l'interface, puis ceux du fichier. */
export function allComponents() {
    const own = Object.fromEntries(ownComponents().map(n => [n, S.model.components[n]]));
    return { ...(S.sharedComponents || {}), ...own };
}

/** Paramètres d'un composant : `[{ name, type, required, default }]`. */
export function paramsOf(component) {
    return Object.entries(component?.params || {}).map(([name, spec]) => {
        const s = typeof spec === 'string' ? { type: spec } : (spec || {});
        return { name, type: s.type || 'text', required: s.required !== false && s.default === undefined, default: s.default };
    });
}

const MISSING = Symbol('absent');
const WHOLE = /^\$([A-Za-z_][A-Za-z0-9_]*)$/;

/** Arbre affiché pour une instance (`null` : pas un composant). Un paramètre
 *  lu dans l'élément de liste (`{ bind }`) s'affiche `{champ}`. */
export function expand(node, depth = 0) {
    const component = allComponents()[node?.type];
    if (!component || depth > 8) { return null; }
    const params = paramsOf(component);
    const names = params.map(p => p.name);
    const values = {};
    for (const p of params) {
        const v = node[p.name] !== undefined ? node[p.name] : p.default;
        if (v !== undefined) { values[p.name] = v && typeof v === 'object' && v.bind ? `{${v.bind}}` : v; }
    }
    const sub = v => {
        if (typeof v === 'string') {
            const m = v.match(WHOLE);
            if (m && names.includes(m[1])) { return m[1] in values ? values[m[1]] : MISSING; }
            return v.replace(/(^|[^$])\$([A-Za-z_][A-Za-z0-9_]*)/g, (all, pre, n) => (names.includes(n) ? pre + (values[n] ?? '') : all));
        }
        if (Array.isArray(v)) { return v.map(sub).filter(x => x !== MISSING); }
        if (v && typeof v === 'object') {
            const out = {};
            for (const [k, x] of Object.entries(v)) { const y = sub(x); if (y !== MISSING) { out[k] = y; } }
            return out;
        }
        return v;
    };
    const out = sub(component.layout || { type: 'column', children: [] });
    for (const [k, v] of Object.entries(node)) {
        if (k !== 'type' && k !== 'id' && !names.includes(k)) { out[k] = v; }
    }
    return out;
}

/** Instance neuve : paramètres obligatoires remplis d'une valeur d'exemple. */
export function instanceTemplate(type) {
    const out = { type };
    for (const p of paramsOf(allComponents()[type])) {
        if (!p.required) { continue; }
        out[p.name] = { text: p.name, number: 0, bool: false, color: '#000000', icon: 'star', action: `${p.name}_action`, length: 'wrap' }[p.type] ?? p.name;
    }
    return out;
}

export async function newComponent() {
    const name = await ask('Nom du nouveau composant (ex. carCard)', '');
    if (!name || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) || allComponents()[name] || S.catalog.types.some(t => t.type === name)) { return; }
    S.screen = '◆' + name;
    edit([{ op: 'set', path: ['components', name], value: { params: {}, layout: { type: 'column', spacing: 8, children: [] } } }], ['components', name]);
}

/** Remplace le nœud `path` par une instance d'un nouveau composant qui le contient. */
export async function convertToComponent(path) {
    const node = getIn(S.model, path);
    const name = await ask('Nom du composant créé à partir de ce nœud', '');
    if (!node || !name || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) || allComponents()[name] || S.catalog.types.some(t => t.type === name)) { return; }
    edit([
        { op: 'set', path: ['components', name], value: { params: {}, layout: node } },
        { op: 'set', path, value: { type: name } },
    ], path);
}

/** Panneau d'un composant édité : nom, paramètres (type, obligatoire, défaut). */
export function componentPanel(name, header, section) {
    const path = ['components', name];
    const component = S.model.components?.[name] || {};
    const out = [header(`◆ ${name}`, 'composant', [
        ['✎', 'Renommer le composant', () => renameComponent(name)],
        ['🗑', 'Supprimer le composant', () => deleteComponent(name)],
    ])];
    const rows = [];
    for (const p of paramsOf(component)) {
        const line = h('div', 'd-param');
        const type = h('select', 'd-input');
        Object.keys(PARAM_KINDS).forEach(t => type.appendChild(new Option(t, t)));
        type.value = p.type;
        const required = h('label', 'd-check');
        const box = h('input');
        box.type = 'checkbox';
        box.checked = p.required;
        required.append(box, document.createTextNode('obligatoire'));
        const write = changes => {
            const next = { type: p.type, required: p.required, default: p.default, ...changes };
            const value = next.required && next.default === undefined ? next.type : { type: next.type, ...(next.required ? {} : { required: false }), ...(next.default !== undefined ? { default: next.default } : {}) };
            edit([{ op: 'set', path: [...path, 'params', p.name], value }]);
        };
        type.addEventListener('change', () => write({ type: type.value }));
        box.addEventListener('change', () => write({ required: box.checked, default: box.checked ? undefined : p.default }));
        const del = h('button', 'd-mini', '✕');
        del.title = 'Retirer le paramètre';
        del.addEventListener('click', () => edit([{ op: 'remove', path: [...path, 'params', p.name] }]));
        const head = h('div', 'd-inline');
        head.append(h('strong', '', `$${p.name}`), type, required, del);
        line.appendChild(head);
        if (!p.required) {
            const def = h('div', 'd-field');
            def.append(h('label', 'd-label', 'défaut'), editor({ name: 'default', kind: PARAM_KINDS[p.type] || 'text' }, p.default, v => write({ default: v })));
            line.appendChild(def);
        }
        rows.push(line);
    }
    const add = h('button', 'd-add', '＋ paramètre');
    add.addEventListener('click', async () => {
        const param = await ask('Nom du paramètre (utilisé dans le layout : $nom)', '');
        if (!param || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(param) || ['id', 'type'].includes(param) || component.params?.[param]) { return; }
        edit([{ op: 'set', path: [...path, 'params', param], value: 'text' }]);
    });
    rows.push(add);
    rows.push(h('p', 'd-muted', 'Dans le layout, $nom est remplacé par la valeur de l\'instance (seul : toute la valeur ; dans un texte : son texte). Un paramètre facultatif absent retire la propriété qui le cite.'));
    out.push(section('Paramètres', rows, { id: `params:${name}` }));
    return out;
}

async function renameComponent(name) {
    const to = await ask('Nouveau nom du composant', name);
    if (!to || to === name || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(to) || allComponents()[to]) { return; }
    const ops = [{ op: 'rename', path: ['components'], from: name, to }];
    instancesOf(name).forEach(p => ops.push({ op: 'set', path: [...p, 'type'], value: to }));
    S.screen = '◆' + to;
    edit(ops, ['components', to, 'layout']);
}

async function deleteComponent(name) {
    const used = instancesOf(name).length;
    if (!(await confirmBox(`Supprimer le composant « ${name} »${used ? ` (encore utilisé ${used} fois)` : ''} ?`))) { return; }
    S.screen = '';
    edit([{ op: 'remove', path: ['components', name] }], null);
}

/** Chemins des instances du composant `name` dans le fichier. */
export function instancesOf(name) {
    const out = [];
    const visit = (v, path) => {
        if (Array.isArray(v)) { v.forEach((x, i) => visit(x, [...path, i])); return; }
        if (!v || typeof v !== 'object') { return; }
        if (v.type === name) { out.push(path); }
        for (const [k, x] of Object.entries(v)) { visit(x, [...path, k]); }
    };
    visit(S.model, []);
    return out;
}

/** Ouvre la déclaration du composant `name` (ici, ou dans son fichier). */
export function openComponent(name) {
    if (ownComponents().includes(name)) {
        S.screen = '◆' + name;
        S.selection = ['components', name, 'layout'];
        refresh();
        return;
    }
    const file = S.sharedComponents?.[name]?.__file;
    if (file) { vscode.postMessage({ type: 'open', file, designer: true }); }
}
