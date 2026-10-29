// Structure : écrans et arbre des nœuds de l'écran affiché. Un clic
// sélectionne, un glisser déplace (avant, après ou dans un conteneur selon
// la hauteur du pointeur sur la ligne), le triangle replie.

import { S, getIn, key, same, screens, rootLayout, layoutPath, screenPath, isContainer, hasSlot, startScreen, refresh, vscode } from './model.js';
import { startDrag, endDrag, drop, dragging } from './dnd.js';
import { GLYPH } from './palette.js';

/** Résumé d'un nœud : son id, sinon son texte. */
export function summary(node) {
    if (!node || typeof node !== 'object') { return ''; }
    if (node.id) { return '#' + node.id; }
    const text = node.value ?? node.text ?? node.label ?? node.name ?? (node.bind ? `{${node.bind}}` : '');
    return text === '' ? '' : `« ${String(text).slice(0, 28)} »`;
}

function row(depth, label, extra, opts) {
    const r = document.createElement('div');
    r.className = 'd-tree-row' + (opts.selected ? ' on' : '') + (opts.error ? ' d-tree-error' : '');
    r.style.paddingLeft = 6 + depth * 14 + 'px';
    const twisty = document.createElement('span');
    twisty.className = 'd-twisty';
    twisty.textContent = opts.collapsible ? (opts.collapsed ? '▸' : '▾') : '';
    const glyph = document.createElement('span');
    glyph.className = 'd-glyph';
    glyph.textContent = opts.glyph || '';
    const name = document.createElement('span');
    name.className = 'd-tree-name';
    name.textContent = label;
    const more = document.createElement('span');
    more.className = 'd-tree-extra';
    more.textContent = extra;
    r.append(twisty, glyph, name, more);
    if (opts.collapsible) {
        twisty.addEventListener('click', ev => { ev.stopPropagation(); opts.toggle(); });
    }
    return r;
}

/** Cible de dépôt sur une ligne de nœud : avant / dans / après. */
function rowTarget(r, path, node, y) {
    const box = r.getBoundingClientRect();
    const t = (y - box.top) / box.height;
    if (isContainer(node) && t > 0.25 && t < 0.75) {
        return { parent: [...path, 'children'], index: (node.children || []).length };
    }
    if (hasSlot(node) && !node.item && t > 0.25 && t < 0.75) { return { slot: [...path, 'item'] }; }
    const last = path[path.length - 1];
    if (typeof last !== 'number') { return null; }
    return { parent: path.slice(0, -1), index: t < 0.5 ? last : last + 1 };
}

function nodeRows(out, node, path, depth) {
    if (!node || typeof node !== 'object') { return; }
    const k = key(path);
    const children = Array.isArray(node.children) ? node.children : [];
    const collapsible = children.length > 0 || !!node.item;
    const r = row(depth, node.type, summary(node), {
        glyph: GLYPH[node.type] || '□', selected: same(S.selection, path), collapsible, collapsed: !!S.collapsed[k],
        error: S.problems.some(p => p.path && key(p.path) === k),
        toggle: () => { S.collapsed[k] = !S.collapsed[k]; refresh(); },
    });
    r.draggable = true;
    r.addEventListener('click', () => { S.selection = path; S.tab = 'props'; refresh(); });
    r.addEventListener('dragstart', ev => startDrag(ev, { from: path }));
    r.addEventListener('dragend', endDrag);
    r.addEventListener('dragover', ev => {
        if (!dragging()) { return; }
        const target = rowTarget(r, path, node, ev.clientY);
        if (!target) { return; }
        ev.preventDefault();
        ev.stopPropagation();
        r.dataset.drop = target.slot || target.parent.length > path.length ? 'in' : target.index === path[path.length - 1] ? 'before' : 'after';
    });
    r.addEventListener('dragleave', () => { delete r.dataset.drop; });
    r.addEventListener('drop', ev => {
        ev.preventDefault();
        ev.stopPropagation();
        delete r.dataset.drop;
        drop(rowTarget(r, path, node, ev.clientY));
    });
    out.appendChild(r);
    if (S.collapsed[k]) { return; }
    children.forEach((c, i) => nodeRows(out, c, [...path, 'children', i], depth + 1));
    if (node.item) {
        const label = row(depth + 1, 'item', 'gabarit d\'élément', { glyph: '⋯' });
        label.classList.add('d-tree-label');
        out.appendChild(label);
        nodeRows(out, node.item, [...path, 'item'], depth + 2);
    }
}

export function renderTree() {
    const root = document.getElementById('tree');
    root.replaceChildren();
    if (!S.model) { return; }
    for (const name of screens()) {
        const current = name === S.screen;
        const r = row(0, rootLayout() ? 'layout' : name, name === startScreen() ? 'démarrage' : '', {
            glyph: '▭', selected: current && same(S.selection, screenPath()) && !rootLayout(),
        });
        r.classList.add('d-tree-screen');
        r.addEventListener('click', () => {
            S.screen = name;
            S.selection = rootLayout() ? null : screenPath(name);
            S.tab = 'props';
            refresh();
        });
        root.appendChild(r);
        if (!current) { continue; }
        const layout = getIn(S.model, layoutPath());
        if (layout) {
            nodeRows(root, layout, layoutPath(), 1);
        } else {
            const empty = row(1, 'aucun layout', 'déposez un composant', {});
            empty.addEventListener('dragover', ev => { if (dragging()) { ev.preventDefault(); } });
            empty.addEventListener('drop', ev => { ev.preventDefault(); drop({ slot: layoutPath() }); });
            root.appendChild(empty);
        }
    }
    for (const inc of S.includes) {
        const r = row(0, inc.file, inc.screens.join(', '), { glyph: '↗' });
        r.classList.add('d-tree-include');
        r.title = 'Fichier inclus (screens.includes) : ouvrir dans le designer';
        r.addEventListener('click', () => vscode.postMessage({ type: 'open', file: inc.file, designer: true }));
        root.appendChild(r);
    }
}
