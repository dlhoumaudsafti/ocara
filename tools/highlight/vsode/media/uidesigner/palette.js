// Palette des composants, groupés ; un type ajouté au schéma du compilateur
// sans groupe connu tombe dans « Autres ». Glisser un composant vers
// l'aperçu ou la structure l'ajoute ; un clic l'ajoute après la sélection
// (ou dans le conteneur sélectionné).

import { S, getIn, isContainer, layoutPath, template, insertNode, placeNode } from './model.js';
import { startDrag, endDrag } from './dnd.js';

const GROUPS = [
    ['Conteneurs', ['column', 'row', 'stack', 'grid', 'card', 'list']],
    ['Affichage', ['text', 'heading', 'image', 'icon', 'progress', 'divider', 'spacer', 'webview']],
    ['Saisie', ['input', 'checkbox', 'switch', 'radio', 'select', 'slider', 'date', 'time']],
    ['Actions', ['button', 'iconButton']],
    ['Android', ['android.fab', 'android.bottomSheet', 'android.drawer']],
];

/** Glyphe court de chaque type (palette, structure). */
export const GLYPH = {
    column: '▤', row: '▥', stack: '◫', grid: '▦', card: '▢', list: '☰',
    text: 'T', heading: 'H', image: '🖼', icon: '★', progress: '▬', divider: '―', spacer: '↔', webview: '🌐',
    input: '⌨', checkbox: '☑', switch: '⏻', radio: '◉', select: '▾', slider: '⎯', date: '📅', time: '🕒',
    button: '⏺', iconButton: '◎', 'android.fab': '⊕', 'android.bottomSheet': '⬓', 'android.drawer': '◧',
};

export const LABEL = {
    column: 'Colonne', row: 'Ligne', stack: 'Pile', grid: 'Grille', card: 'Carte', list: 'Liste',
    text: 'Texte', heading: 'Titre', image: 'Image', icon: 'Icône', progress: 'Progression', divider: 'Séparateur',
    spacer: 'Espace', webview: 'Vue web', input: 'Champ', checkbox: 'Case', switch: 'Interrupteur', radio: 'Radio',
    select: 'Liste déroulante', slider: 'Curseur', date: 'Date', time: 'Heure', button: 'Bouton', iconButton: 'Bouton icône',
    'android.fab': 'Bouton flottant', 'android.bottomSheet': 'Panneau inférieur', 'android.drawer': 'Tiroir',
};

/** Ajoute un composant près de la sélection. */
function addNear(type) {
    const node = template(type);
    const sel = S.selection;
    const target = sel && getIn(S.model, sel);
    if (sel && target && isContainer(target)) {
        insertNode([...sel, 'children'], (target.children || []).length, node);
    } else if (sel && typeof sel[sel.length - 1] === 'number') {
        insertNode(sel.slice(0, -1), sel[sel.length - 1] + 1, node);
    } else {
        const layout = getIn(S.model, layoutPath());
        if (!layout) { placeNode(layoutPath(), node); } else if (isContainer(layout)) {
            insertNode([...layoutPath(), 'children'], (layout.children || []).length, node);
        }
    }
}

export function renderPalette() {
    const root = document.getElementById('palette');
    const filter = document.getElementById('paletteFilter').value.trim().toLowerCase();
    root.replaceChildren();
    const known = new Set(S.catalog.types.map(t => t.type));
    const grouped = new Set(GROUPS.flatMap(g => g[1]));
    const groups = [...GROUPS, ['Autres', [...known].filter(t => !grouped.has(t))]];
    for (const [title, types] of groups) {
        const shown = types.filter(t => known.has(t) && (!filter || t.toLowerCase().includes(filter) || (LABEL[t] || '').toLowerCase().includes(filter)));
        if (!shown.length) { continue; }
        const h = document.createElement('h3');
        h.textContent = title;
        const grid = document.createElement('div');
        grid.className = 'd-palette-grid';
        for (const type of shown) {
            const item = document.createElement('button');
            item.className = 'd-palette-item';
            item.draggable = true;
            item.title = `${type} — glisser vers l'écran, ou cliquer pour ajouter`;
            item.innerHTML = '<span class="d-glyph"></span><span class="d-name"></span>';
            item.querySelector('.d-glyph').textContent = GLYPH[type] || '□';
            item.querySelector('.d-name').textContent = LABEL[type] || type;
            item.addEventListener('dragstart', ev => startDrag(ev, { type }));
            item.addEventListener('dragend', endDrag);
            item.addEventListener('click', () => addNear(type));
            grid.appendChild(item);
        }
        root.append(h, grid);
    }
}
