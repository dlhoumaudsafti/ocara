// Palette des composants, groupés ; un type ajouté au schéma du compilateur
// sans groupe connu tombe dans « Autres ». Glisser un composant vers
// l'aperçu ou la structure l'ajoute ; un clic l'ajoute après la sélection
// (ou dans le conteneur sélectionné).

import { S, getIn, isContainer, layoutPath, template, insertNode, placeNode } from './model.js';
import { startDrag, endDrag } from './dnd.js';
import { allComponents, newComponent, openComponent } from './components.js';

const GROUPS = [
    ['Conteneurs', ['column', 'row', 'stack', 'grid', 'card', 'list']],
    ['Affichage', ['text', 'heading', 'image', 'icon', 'progress', 'divider', 'spacer', 'webview']],
    ['Saisie', ['input', 'checkbox', 'switch', 'radio', 'select', 'slider', 'date', 'time']],
    ['Actions', ['button', 'iconButton', 'link']],
    ['Écran', ['fab', 'bottomSheet', 'drawer', 'floating', 'spinner']],
];

/** Glyphe court de chaque type (palette, structure). */
export const GLYPH = {
    column: '▤', row: '▥', stack: '◫', grid: '▦', card: '▢', list: '☰',
    text: 'T', heading: 'H', image: '🖼', icon: '★', progress: '▬', divider: '―', spacer: '↔', webview: '🌐',
    input: '⌨', checkbox: '☑', switch: '⏻', radio: '◉', select: '▾', slider: '⎯', date: '📅', time: '🕒',
    button: '⏺', iconButton: '◎', link: '🔗', 'fab': '⊕', 'bottomSheet': '⬓', 'drawer': '◧',
    floating: '⧉', spinner: '◌',
};

export const LABEL = {
    column: 'Colonne', row: 'Ligne', stack: 'Pile', grid: 'Grille', card: 'Carte', list: 'Liste',
    text: 'Texte', heading: 'Titre', image: 'Image', icon: 'Icône', progress: 'Progression', divider: 'Séparateur',
    spacer: 'Espace', webview: 'Vue web', input: 'Champ', checkbox: 'Case', switch: 'Interrupteur', radio: 'Radio',
    select: 'Liste déroulante', slider: 'Curseur', date: 'Date', time: 'Heure', button: 'Bouton', iconButton: 'Bouton icône', link: 'Lien',
    'fab': 'Bouton flottant', 'bottomSheet': 'Panneau inférieur', 'drawer': 'Tiroir',
    floating: 'Flottant', spinner: 'Attente',
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
    const mine = Object.keys(allComponents()).sort();
    mine.forEach(t => known.add(t));
    const groups = [...GROUPS, ['Autres', [...known].filter(t => !grouped.has(t) && !mine.includes(t))], ['Mes composants', mine]];
    for (const [title, types] of groups) {
        const shown = types.filter(t => known.has(t) && (!filter || t.toLowerCase().includes(filter) || (LABEL[t] || '').toLowerCase().includes(filter)));
        const own = title === 'Mes composants';
        if (!shown.length && !own) { continue; }
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
            item.querySelector('.d-glyph').textContent = own ? '◆' : GLYPH[type] || '□';
            if (own) {
                item.title = `${type} — composant : glisser vers l'écran, cliquer pour ajouter, double-clic pour le modifier`;
                item.addEventListener('dblclick', () => openComponent(type));
            }
            item.querySelector('.d-name').textContent = LABEL[type] || type;
            item.addEventListener('dragstart', ev => startDrag(ev, { type }));
            item.addEventListener('dragend', endDrag);
            item.addEventListener('click', () => addNear(type));
            grid.appendChild(item);
        }
        if (own) {
            const add = document.createElement('button');
            add.className = 'd-palette-item d-palette-new';
            add.innerHTML = '<span class="d-glyph">＋</span><span class="d-name">Nouveau composant</span>';
            add.title = 'Déclarer un composant dans ce fichier (components:)';
            add.addEventListener('click', newComponent);
            grid.appendChild(add);
        }
        root.append(h, grid);
    }
}
