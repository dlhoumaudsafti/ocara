// Panneau de droite : propriétés de la sélection (nœud ou écran), fenêtre,
// styles nommés et menu de l'application. Toutes les propriétés du schéma
// sont éditables, rangées par rubrique ; une rubrique repliée le reste.

import {
    S, getIn, edit, setProp, typeSpec, allProps, inItem, screens, rootLayout, screenPath, startScreen,
    removeNode, refresh, vscode, styles,
} from './model.js';
import { editor, menuEditor, h } from './editors.js';
import { LABEL } from './palette.js';
import { duplicate, moveBy } from './actions.js';
import { ask, confirmBox } from './dialog.js';

/** Options d'une liste en grille, montrées avec `display: grid` seulement. */
const GRID_PROPS = ['itemWidth', 'minColumns', 'maxColumns', 'direction'];
const SNAKE_WIDTH = 50;

/** Propriété d'une liste : en serpentin, `itemWidth` est obligatoire (50
 *  posé au choix de la direction, champ vidé encadré en rouge). Quitter la
 *  grille retire ses options. */
function gridRow(p, path, node, row) {
    const snake = /snake$/.test(node.direction || '');
    if (p.name === 'direction') {
        return field(p, path, node, { set: v => {
            const ops = [{ op: 'set', path: [...path, 'direction'], value: v }];
            if (/snake$/.test(v || '') && node.itemWidth === undefined) { ops.push({ op: 'set', path: [...path, 'itemWidth'], value: SNAKE_WIDTH }); }
            edit(ops);
        } });
    }
    if (p.name === 'display') {
        return field(p, path, node, { set: v => {
            const ops = [{ op: 'set', path: [...path, 'display'], value: v }];
            if (v !== 'grid') { GRID_PROPS.forEach(k => node[k] !== undefined && ops.push({ op: 'set', path: [...path, k] })); }
            edit(ops);
        } });
    }
    const r = row(p);
    if (p.name === 'itemWidth' && snake && node.itemWidth === undefined) {
        r.classList.add('d-invalid-field');
        r.title = 'Obligatoire en serpentin (50 si absent à l\'exécution)';
    }
    return r;
}

const LAYOUT = ['id', 'style', 'visible', 'enabled', 'width', 'height', 'weight', 'align'];
const A11Y = ['label', 'tooltip'];
/** Ni surchargées par plateforme, ni listées dans les rubriques ordinaires. */
const STRUCTURAL = ['type', 'id', 'children', 'item', 'tauri', 'android', 'binds', 'contextMenu'];

let open = {}; // rubrique → repliée ?

function section(title, rows, { id = title, collapsed = false } = {}) {
    const box = h('section', 'd-section');
    const head = h('h3', 'd-section-title', title);
    const closed = open[id] ?? collapsed;
    if (closed) { box.classList.add('closed'); }
    head.addEventListener('click', () => { open[id] = !box.classList.contains('closed'); box.classList.toggle('closed'); });
    box.appendChild(head);
    const body = h('div', 'd-section-body');
    rows.forEach(r => r && body.appendChild(r));
    box.appendChild(body);
    return rows.filter(Boolean).length ? box : null;
}

/** Ligne libellé + éditeur ; `path` : table qui porte la propriété. */
function field(spec, path, holder, { required = false, inherited, set } = {}) {
    const r = h('div', 'd-field');
    const label = h('label', 'd-label', spec.name);
    if (required) { label.appendChild(h('span', 'd-required', ' *')); }
    if (holder?.[spec.name] !== undefined) { label.classList.add('d-set'); }
    label.title = `${spec.name} (${spec.kind})`;
    r.append(label, editor(spec, holder?.[spec.name], set || (v => setProp(path, spec.name, v)), { inherited }));
    r.dataset.key = JSON.stringify([...path, spec.name]);
    if (required && holder?.[spec.name] === undefined) { r.classList.add('d-missing'); }
    return r;
}

function header(title, sub, buttons = []) {
    const head = h('div', 'd-head');
    const t = h('div', 'd-head-title');
    t.append(h('strong', '', title), h('span', 'd-muted', sub));
    const tools = h('div', 'd-head-tools');
    buttons.forEach(([text, tip, fn]) => {
        const b = h('button', 'd-mini', text);
        b.title = tip;
        b.addEventListener('click', fn);
        tools.appendChild(b);
    });
    head.append(t, tools);
    return head;
}

function nodePanel(path) {
    const node = getIn(S.model, path);
    const spec = typeSpec(node.type);
    const out = [];
    out.push(header(LABEL[node.type] || node.type, node.type, [
        ['↑', 'Monter (Alt+↑)', () => moveBy(path, -1)],
        ['↓', 'Descendre (Alt+↓)', () => moveBy(path, 1)],
        ['⧉', 'Dupliquer (Ctrl+D)', () => duplicate(path)],
        ['{ }', 'Voir dans le source', () => vscode.postMessage({ type: 'reveal', path })],
        ['🗑', 'Supprimer (Suppr)', () => removeNode(path)],
    ]));
    const problems = S.problems.filter(p => p.path && JSON.stringify(p.path) === JSON.stringify(path));
    problems.forEach(p => out.push(h('div', 'd-problem-inline ' + p.severity, p.message)));

    // Changement de type : garde les propriétés que le nouveau type accepte.
    const types = h('select', 'd-input');
    S.catalog.types.forEach(t => types.appendChild(new Option(`${LABEL[t.type] || t.type} (${t.type})`, t.type)));
    types.value = node.type;
    types.addEventListener('change', () => {
        const accepted = new Set(allProps(types.value).map(p => p.name));
        const next = { type: types.value };
        for (const [k, v] of Object.entries(node)) { if (k !== 'type' && accepted.has(k)) { next[k] = v; } }
        edit([{ op: 'set', path, value: next }]);
    });
    const typeRow = h('div', 'd-field');
    typeRow.append(h('label', 'd-label', 'type'), types);
    out.push(typeRow);

    const required = spec?.required || [];
    const named = node.style && styles()[node.style];
    const row = p => field(p, path, node, { required: required.includes(p.name), inherited: named?.[p.name] });
    const own = (spec?.own || []).filter(p => !['children', 'node'].includes(p.kind) && (p.kind !== 'field' || inItem(path)))
        .filter(p => node.type !== 'list' || node.display === 'grid' || !GRID_PROPS.includes(p.name));
    const common = Object.fromEntries(S.catalog.common.map(p => [p.name, p]));
    out.push(section('Propriétés', own.filter(p => p.kind !== 'action').map(p => (node.type === 'list' ? gridRow(p, path, node, row) : row(p))), { id: 'own' }));
    out.push(section('Événements', own.filter(p => p.kind === 'action').map(row), { id: 'events' }));
    out.push(section('Disposition', LAYOUT.map(n => common[n] && row(common[n])), { id: 'layout' }));
    out.push(section('Présentation', S.catalog.style.map(row), { id: 'style', collapsed: true }));
    out.push(section('Accessibilité', A11Y.map(n => common[n] && row(common[n])), { id: 'a11y' }));
    if (inItem(path) && common.binds) { out.push(section('Liaisons (élément de liste)', [row(common.binds)], { id: 'binds' })); }
    if (common.contextMenu) {
        const r = h('div', 'd-field d-wide');
        r.appendChild(menuEditor(node.contextMenu, v => setProp(path, 'contextMenu', v)));
        out.push(section('Menu contextuel', [r], { id: 'context', collapsed: !node.contextMenu }));
    }
    for (const [platform, title] of [['tauri', 'Surcharge desktop (tauri)'], ['android', 'Surcharge Android']]) {
        const holder = node[platform] && typeof node[platform] === 'object' ? node[platform] : {};
        const props = allProps(node.type).filter(p => !STRUCTURAL.includes(p.name) && !['children', 'node'].includes(p.kind))
            .filter(p => node.type !== 'list' || node.display === 'grid' || !GRID_PROPS.includes(p.name));
        const rows = props.map(p => {
            const r = h('div', 'd-field');
            const label = h('label', 'd-label', p.name);
            if (holder[p.name] !== undefined) { label.classList.add('d-set'); }
            r.append(label, editor(p, holder[p.name], v => {
                const next = { ...holder };
                if (v === undefined) { delete next[p.name]; } else { next[p.name] = v; }
                setProp(path, platform, Object.keys(next).length ? next : undefined);
            }));
            return r;
        });
        out.push(section(title, rows, { id: platform, collapsed: !node[platform] }));
    }
    return out;
}

function screenPanel(name) {
    const path = screenPath(name);
    const screen = getIn(S.model, path) || {};
    const out = [header(`Écran ${name}`, name === startScreen() ? 'écran de démarrage' : '', [
        ['✎', 'Renommer', () => renameScreen(name)],
        ['★', 'Écran de démarrage', () => edit([{ op: 'set', path: ['start'], value: name }])],
        ['🗑', 'Supprimer l\'écran', () => deleteScreen(name)],
    ])];
    out.push(section('Écran', S.catalog.screen.map(p => field(p, path, screen)), { id: 'screen' }));
    return out;
}

export async function renameScreen(name) {
    const input = await ask('Nouveau nom de l\'écran', name);
    if (!input || input === name || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(input) || screens().includes(input)) { return; }
    const ops = [{ op: 'rename', path: ['screens'], from: name, to: input }];
    if (S.model.start === name) { ops.push({ op: 'set', path: ['start'], value: input }); }
    S.screen = input;
    edit(ops, ['screens', input]);
}

async function deleteScreen(name) {
    if (screens().length < 2 || !(await confirmBox(`Supprimer l'écran « ${name} » ?`))) { return; }
    const ops = [{ op: 'remove', path: ['screens', name] }];
    const rest = screens().filter(s => s !== name);
    if (S.model.start === name) { ops.push({ op: 'set', path: ['start'], value: rest[0] }); }
    S.screen = rest[0];
    edit(ops, null);
}

function windowPanel() {
    const win = S.model.window || {};
    const out = [header('Fenêtre', 'window')];
    out.push(section('Fenêtre', S.catalog.window.map(p => field(p, ['window'], win)), { id: 'window' }));
    if (!rootLayout()) {
        const start = h('select', 'd-input');
        screens().forEach(s => start.appendChild(new Option(s, s)));
        start.value = startScreen() || '';
        start.addEventListener('change', () => edit([{ op: 'set', path: ['start'], value: start.value }]));
        const r = h('div', 'd-field');
        r.append(h('label', 'd-label', 'start'), start);
        out.push(section('Démarrage', [r], { id: 'start' }));
    }
    out.push(section('Desktop (window.tauri)', S.catalog.windowTauri.map(p => field(p, ['window', 'tauri'], win.tauri)), { id: 'wtauri' }));
    out.push(section('Android (window.android)', S.catalog.windowAndroid.map(p => field(p, ['window', 'android'], win.android)), { id: 'wandroid' }));
    return out;
}

function stylesPanel() {
    const all = S.model.styles || {};
    const styles = Object.fromEntries(Object.entries(all).filter(([n]) => !THEMES.includes(n)));
    const out = [header('Styles', 'styles', [['＋', 'Nouveau style', async () => {
        const name = await ask('Nom du style', '');
        if (name && /^[A-Za-z_][A-Za-z0-9_]*$/.test(name) && !styles[name]) { edit([{ op: 'set', path: ['styles', name], value: {} }]); open[`style:${name}`] = false; }
    }]])];
    const used = {};
    JSON.stringify(S.model, (k, v) => { if (k === 'style' && typeof v === 'string') { used[v] = (used[v] || 0) + 1; } return v; });
    for (const name of Object.keys(styles)) {
        const path = ['styles', name];
        const rows = S.catalog.style.map(p => field(p, path, styles[name]));
        const tools = h('div', 'd-inline');
        const rename = h('button', 'd-mini', '✎ renommer');
        rename.addEventListener('click', async () => {
            const to = await ask('Nouveau nom du style', name);
            if (!to || to === name || styles[to] || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(to)) { return; }
            const ops = [{ op: 'rename', path: ['styles'], from: name, to }];
            walkStyles(name, p => ops.push({ op: 'set', path: [...p, 'style'], value: to }));
            edit(ops);
        });
        const del = h('button', 'd-mini', '🗑 supprimer');
        del.addEventListener('click', () => {
            const ops = [{ op: 'remove', path }];
            walkStyles(name, p => ops.push({ op: 'remove', path: [...p, 'style'] }));
            edit(ops);
        });
        tools.append(rename, del);
        out.push(section(`${name}  ·  ${used[name] || 0} nœud(s)`, [tools, ...rows], { id: `style:${name}`, collapsed: true }));
    }
    if (!Object.keys(styles).length) { out.push(h('p', 'd-muted d-pad', 'Aucun style. Un style nommé regroupe des propriétés de présentation, appliquées par la propriété style d\'un nœud.')); }
    for (const theme of THEMES) { out.push(themePanel(theme, all[theme] || {})); }
    return out;
}

const THEMES = ['dark', 'light'];

/** Surcharges d'un thème (`styles.dark`, `styles.light`) : propriétés
 *  appliquées par-dessus le style de même nom quand ce thème est affiché. */
function themePanel(theme, overrides) {
    const rows = [];
    const add = h('select', 'd-input');
    add.append(new Option(`＋ surcharger un style en thème ${theme === 'dark' ? 'sombre' : 'clair'}…`, ''),
        ...Object.keys(styles()).filter(n => !overrides[n]).map(n => new Option(n, n)));
    add.addEventListener('change', () => { if (add.value) { edit([{ op: 'set', path: ['styles', theme, add.value], value: {} }]); open[`${theme}:${add.value}`] = false; } });
    const tools = h('div', 'd-inline');
    tools.appendChild(add);
    rows.push(tools);
    for (const name of Object.keys(overrides)) {
        const path = ['styles', theme, name];
        const del = h('button', 'd-mini', '🗑 retirer la surcharge');
        del.addEventListener('click', () => {
            const rest = Object.keys(overrides).filter(n => n !== name);
            edit([{ op: 'remove', path: rest.length ? path : ['styles', theme] }]);
        });
        const line = h('div', 'd-inline');
        line.appendChild(del);
        const base = styles()[name] || {};
        const fields = S.catalog.style.map(p => field(p, path, overrides[name], { inherited: base[p.name] }));
        rows.push(section(`${name}`, [line, ...fields], { id: `${theme}:${name}`, collapsed: true }));
    }
    return section(theme === 'dark' ? 'Thème sombre (styles.dark)' : 'Thème clair (styles.light)', rows, { id: `theme:${theme}`, collapsed: !Object.keys(overrides).length });
}

/** Nœuds qui utilisent le style `name` : fn(chemin). */
function walkStyles(name, fn) {
    const visit = (v, path) => {
        if (Array.isArray(v)) { v.forEach((x, i) => visit(x, [...path, i])); return; }
        if (!v || typeof v !== 'object') { return; }
        if (v.type && v.style === name) { fn(path); }
        for (const [k, x] of Object.entries(v)) { if (k !== 'styles') { visit(x, [...path, k]); } }
    };
    visit(S.model, []);
}

function menuPanel() {
    const r = h('div', 'd-field d-wide');
    r.appendChild(menuEditor(S.model.menu, v => setProp([], 'menu', v)));
    return [header('Menu de l\'application', 'menu'), r];
}

/** Fichier inclus : fenêtre, styles et menu sont ceux du fichier racine. */
function rootNotice(owner) {
    const open = h('button', 'd-mini', `Ouvrir ${S.context.root}`);
    open.addEventListener('click', () => vscode.postMessage({ type: 'open', file: S.context.root, designer: true }));
    const names = owner === 'styles' ? Object.keys(S.context.styles || {}).join(', ') : '';
    const tools = h('div', 'd-pad d-inline');
    tools.appendChild(open);
    if (owner === 'styles') {
        // Un fichier inclus peut déclarer les styles de ses écrans.
        const add = h('button', 'd-mini', '＋ Style dans ce fichier');
        add.addEventListener('click', async () => {
            const name = await ask('Nom du style', '');
            if (name && /^[A-Za-z_][A-Za-z0-9_]*$/.test(name) && !styles()[name]) { edit([{ op: 'set', path: ['styles', name], value: {} }]); }
        });
        tools.appendChild(add);
    }
    return [
        header(owner === 'window' ? 'Fenêtre' : owner === 'styles' ? 'Styles' : 'Menu', owner),
        h('p', 'd-muted d-pad', `Ce fichier est inclus par ${S.context.root} : ${owner === 'styles' ? 'les styles de l\'interface y sont déclarés, ou dans ses autres fichiers' : `${owner} y est déclaré`}.${names ? ` Styles disponibles : ${names}.` : ''}`),
        tools,
    ];
}

export function renderInspector() {
    const root = document.getElementById('inspector');
    const focus = document.activeElement?.closest?.('[data-key]')?.dataset.key;
    document.querySelectorAll('#tabs button').forEach(b => b.classList.toggle('on', b.dataset.tab === S.tab));
    root.replaceChildren();
    if (!S.model) { return; }
    let parts;
    const sel = S.selection;
    const node = sel && getIn(S.model, sel);
    const owner = { window: 'window', styles: 'styles', menu: 'menu' }[S.tab];
    if (owner && S.context && S.model[owner] === undefined) { parts = rootNotice(owner); } else if (S.tab === 'window') { parts = windowPanel(); } else if (S.tab === 'styles') { parts = stylesPanel(); } else if (S.tab === 'menu') { parts = menuPanel(); } else if (node && typeof node === 'object' && node.type) {
        parts = nodePanel(sel);
    } else if (!rootLayout() && S.screen) {
        parts = screenPanel(S.screen);
    } else {
        parts = [h('p', 'd-muted d-pad', 'Sélectionnez un composant dans l\'aperçu ou la structure.')];
    }
    parts.filter(Boolean).forEach(p => root.appendChild(p));
    // Le champ qui avait le focus le retrouve après le réaffichage.
    if (focus) { [...root.querySelectorAll('[data-key]')].find(e => e.dataset.key === focus)?.querySelector('input, select, textarea')?.focus(); }
}

export function installInspector() {
    document.querySelectorAll('#tabs button').forEach(b => b.addEventListener('click', () => { S.tab = b.dataset.tab; refresh(); }));
}
