// Actions sur le nœud sélectionné : dupliquer, déplacer d'un rang, copier,
// couper, coller, envelopper dans un conteneur ; menu contextuel de
// l'aperçu et de la structure, raccourcis clavier.

import { S, getIn, edit, isContainer, cloneFresh, insertNode, moveNode, removeNode, placeNode, layoutPath, refresh, vscode } from './model.js';
import { LABEL } from './palette.js';
import { convertToComponent } from './components.js';

const inList = path => !!path && typeof path[path.length - 1] === 'number';

export function duplicate(path) {
    if (!inList(path)) { return; }
    insertNode(path.slice(0, -1), path[path.length - 1] + 1, cloneFresh(getIn(S.model, path)));
}

export function moveBy(path, delta) {
    if (!inList(path)) { return; }
    const list = getIn(S.model, path.slice(0, -1));
    const i = path[path.length - 1];
    const j = i + delta;
    if (j < 0 || j >= list.length) { return; }
    moveNode(path, path.slice(0, -1), delta > 0 ? j + 1 : j);
}

export function copy(path) { S.clipboard = JSON.parse(JSON.stringify(getIn(S.model, path))); }

export function cut(path) { copy(path); removeNode(path); }

/** Colle après la sélection, ou dans le conteneur sélectionné, ou comme layout d'un écran vide. */
export function paste() {
    if (!S.clipboard) { return; }
    const node = cloneFresh(S.clipboard);
    const sel = S.selection;
    const target = sel && getIn(S.model, sel);
    if (target && isContainer(target)) { insertNode([...sel, 'children'], (target.children || []).length, node); return; }
    if (inList(sel)) { insertNode(sel.slice(0, -1), sel[sel.length - 1] + 1, node); return; }
    if (!getIn(S.model, layoutPath())) { placeNode(layoutPath(), node); }
}

/** Remplace le nœud par un conteneur `type` qui le contient. */
export function wrapIn(path, type) {
    const node = getIn(S.model, path);
    edit([{ op: 'set', path, value: { type, spacing: 8, children: [node] } }], [...path, 'children', 0]);
}

/** Sélectionne le nœud parent. */
export function selectParent() {
    const sel = S.selection;
    if (!sel) { return; }
    let p = sel.slice(0, -1);
    while (p.length && !getIn(S.model, p)?.type) { p = p.slice(0, -1); }
    S.selection = p.length ? p : null;
    refresh();
}

// ── menu contextuel ────────────────────────────────────────────────────────

let menu;

export function closeMenu() { menu?.remove(); menu = undefined; }

export function contextMenu(ev, path) {
    ev.preventDefault();
    closeMenu();
    S.selection = path;
    refresh();
    const entries = [
        ['Dupliquer', 'Ctrl+D', () => duplicate(path), inList(path)],
        ['Copier', 'Ctrl+C', () => copy(path), true],
        ['Couper', 'Ctrl+X', () => cut(path), true],
        ['Coller', 'Ctrl+V', paste, !!S.clipboard],
        null,
        ['Monter', 'Alt+↑', () => moveBy(path, -1), inList(path)],
        ['Descendre', 'Alt+↓', () => moveBy(path, 1), inList(path)],
        ['Sélectionner le parent', 'Échap', selectParent, path.length > 1],
        null,
        ...['column', 'row', 'card'].map(t => [`Envelopper dans : ${LABEL[t]}`, '', () => wrapIn(path, t), true]),
        ['Convertir en composant…', '', () => convertToComponent(path), true],
        null,
        ['Voir dans le source', '', () => vscode.postMessage({ type: 'reveal', path }), true],
        ['Supprimer', 'Suppr', () => removeNode(path), true],
    ];
    menu = document.createElement('div');
    menu.className = 'd-context';
    for (const entry of entries) {
        if (!entry) { menu.appendChild(document.createElement('hr')); continue; }
        const [label, keys, fn, enabled] = entry;
        const b = document.createElement('button');
        b.disabled = !enabled;
        b.innerHTML = '<span></span><kbd></kbd>';
        b.firstChild.textContent = label;
        b.lastChild.textContent = keys;
        b.addEventListener('click', () => { closeMenu(); fn(); });
        menu.appendChild(b);
    }
    document.body.appendChild(menu);
    const r = menu.getBoundingClientRect();
    menu.style.left = Math.min(ev.clientX, innerWidth - r.width - 4) + 'px';
    menu.style.top = Math.min(ev.clientY, innerHeight - r.height - 4) + 'px';
}

// ── clavier ────────────────────────────────────────────────────────────────

export function installKeys() {
    document.addEventListener('mousedown', ev => { if (menu && !menu.contains(ev.target)) { closeMenu(); } });
    document.addEventListener('keydown', ev => {
        const t = ev.target;
        if (t.closest && t.closest('input, textarea, select, [contenteditable="true"], .d-modal')) { return; }
        const sel = S.selection;
        const node = sel && getIn(S.model, sel);
        const mod = ev.ctrlKey || ev.metaKey;
        if (ev.key === 'Escape') { closeMenu(); selectParent(); return; }
        if (mod && ev.key.toLowerCase() === 'v') { paste(); ev.preventDefault(); return; }
        if (!node || !node.type) { return; }
        if (ev.key === 'Delete' || ev.key === 'Backspace') { removeNode(sel); ev.preventDefault(); } else if (mod && ev.key.toLowerCase() === 'd') { duplicate(sel); ev.preventDefault(); } else if (mod && ev.key.toLowerCase() === 'c') { copy(sel); ev.preventDefault(); } else if (mod && ev.key.toLowerCase() === 'x') { cut(sel); ev.preventDefault(); } else if (ev.altKey && ev.key === 'ArrowUp') { moveBy(sel, -1); ev.preventDefault(); } else if (ev.altKey && ev.key === 'ArrowDown') { moveBy(sel, 1); ev.preventDefault(); }
    });
}
