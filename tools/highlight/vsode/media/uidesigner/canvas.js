// Aperçu de l'écran : même DOM et mêmes classes `oc-*` que le moteur desktop
// (runtime_tauri/src/ui/engine.js, apparence : engine.css copié), chaque
// élément marqué du chemin de son nœud (`data-path`) pour la sélection et le
// glisser-déposer. Les champs ne réagissent pas : un clic sélectionne.

import { S, typeSpec, editedComponent, getIn, key, startsWith, styles, themeStyles, effectiveTheme, windowProps, layoutPath, screenPath, startScreen, rootLayout, isContainer, hasSlot, axisOf, animations } from './model.js';
import { expand } from './components.js';

const FLEX_ALIGN = { start: 'flex-start', center: 'center', end: 'flex-end', stretch: 'stretch' };
const px = v => (typeof v === 'number' ? v + 'px' : v);
const spacing = v => (Array.isArray(v) ? v.map(px).join(' ') : px(v));

function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) { e.className = cls; }
    if (text !== undefined) { e.textContent = text; }
    return e;
}

/** Propriétés effectives : style nommé, nœud, surcharge de la plateforme
 *  affichée ; tables d'état fusionnées (comme le moteur), celle de l'état
 *  prévisualisé (barre d'outils) appliquée. */
function props(node) {
    const named = node.style && styles()[node.style];
    const themed = node.style && themeStyles(effectiveTheme())[node.style];
    const out = {};
    for (const layer of [named || {}, themed || {}, node, node[S.platform] || {}]) {
        for (const k in layer) {
            if (k === 'tauri' || k === 'android') { continue; }
            out[k] = k === 'states' && window.OcaraEffects ? OcaraEffects.mergeStates(out.states, layer[k]) : layer[k];
        }
    }
    const forced = S.previewState && out.states?.[S.previewState];
    return normalized(forced ? { ...out, ...forced } : out);
}

/** Ombre complète (comme `shadow_with_defaults`, src/core/ui_schema.d/effects.rs). */
const fullShadow = s => ({ x: 0, y: 0, blur: 0, spread: 0, color: '#00000040', inset: false, ...s });

/** Valeurs d'effets sous la forme de l'arbre normalisé qu'attend effects.js :
 *  `shadow` en liste complète, `animation` en table `{ name, on }`. */
function normalized(p) {
    const out = { ...p };
    const shadows = v => (Array.isArray(v) ? v : [v]).filter(s => s && typeof s === 'object').map(fullShadow);
    if (out.shadow) { out.shadow = shadows(out.shadow); }
    if (out.states && typeof out.states === 'object') {
        out.states = Object.fromEntries(Object.entries(out.states).map(([k, t]) => [k, t && t.shadow ? { ...t, shadow: shadows(t.shadow) } : t]));
    }
    if (typeof out.animation === 'string') { out.animation = { name: out.animation, on: 'appear' }; }
    else if (out.animation && typeof out.animation === 'object') { out.animation = { on: 'appear', ...out.animation }; }
    return out;
}

export function icon(name) {
    const span = el('span', 'oc-icon');
    const d = S.icons[name];
    if (d) {
        span.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor"></path></svg>';
        span.querySelector('path').setAttribute('d', d);
    }
    return span;
}

/** Texte affiché : `{champ}` pour un `bind`. */
function shown(p) {
    if (p.bind) { return `{${p.bind}}`; }
    return p.value == null ? '' : String(p.value);
}

function decorate(e, p, parentAxis, container) {
    const s = e.style;
    if (p.align && !container) {
        s.alignSelf = s.justifySelf = FLEX_ALIGN[p.align];
        if (p.align === 'center' || p.align === 'end') { s.textAlign = p.align; }
    }
    if (p.padding != null) { s.padding = spacing(p.padding); }
    if (p.margin != null) { s.margin = spacing(p.margin); }
    if (p.fontSize != null) { s.fontSize = px(p.fontSize); }
    if (p.bold) { s.fontWeight = 'bold'; }
    if (p.italic) { s.fontStyle = 'italic'; }
    if (p.color) { s.color = p.color; }
    if (p.background) { s.background = p.background; }
    if (p.radius != null) { s.borderRadius = px(p.radius); }
    if (p.border) { s.border = '1px solid ' + p.border; }
    if (p.elevation) { s.boxShadow = `0 ${p.elevation / 2}px ${p.elevation * 1.5}px rgba(0,0,0,.25)`; }
    for (const dim of ['width', 'height']) {
        const v = p[dim];
        const main = (dim === 'width') === (parentAxis === 'row');
        if (v === 'fill') {
            if (main) { s.flex = '1 1 0'; s[dim === 'width' ? 'minWidth' : 'minHeight'] = '0'; } else { s.alignSelf = 'stretch'; }
        } else if (typeof v === 'number') { s[dim] = px(v); }
    }
    if (p.weight) { s.flex = p.weight + ' 1 0'; s.minWidth = '0'; }
    if (p.tooltip) { e.title = String(p.tooltip); }
    if (p.visible === false) { e.classList.add('d-invisible'); }
    if (p.enabled === false) { e.classList.add('oc-disabled'); }
    // Ombres, opacité, flous, transitions, tables d'état et animations : effects.js du moteur.
    if (window.OcaraEffects) { OcaraEffects.apply(e, p, animationTable); }
}

/** Animations de l'aperçu, normalisées à chaque rendu. */
let animationTable = {};

function container(node, p, path) {
    const e = el('div', 'oc-' + node.type);
    if (p.spacing != null) { e.style.gap = px(p.spacing); }
    if (node.type === 'grid') { e.style.gridTemplateColumns = `repeat(${p.columns || 2}, minmax(0, 1fr))`; }
    if (p.align) { e.style.alignItems = FLEX_ALIGN[p.align]; }
    const children = Array.isArray(node.children) ? node.children : [];
    children.forEach((child, i) => e.appendChild(build(child, path && [...path, 'children', i], axisOf(node))));
    if (!children.length) { e.appendChild(el('div', 'd-empty', 'Déposez un composant ici')); }
    return e;
}

function list(node, p, path) {
    const e = el('div', 'oc-list');
    if (p.spacing != null) { e.style.gap = px(p.spacing); }
    if (node.item && typeof node.item === 'object') {
        const item = build(node.item, path && [...path, 'item'], 'column');
        item.classList.add('oc-list-item', 'd-template');
        e.appendChild(item);
        const cells = [item];
        // Copies fantômes : une ligne de grille et un retour à la ligne.
        for (let i = 0; i < (p.display === 'grid' ? 5 : 2); i++) {
            const ghost = build(node.item, null, 'column');
            ghost.classList.add('oc-list-item', 'd-ghost');
            e.appendChild(ghost);
            cells.push(ghost);
        }
        if (p.display === 'grid') { grid(e, cells, p); }
    } else {
        const slot = el('div', 'd-empty d-slot', 'Gabarit d\'élément : déposez un composant');
        if (path) { slot.dataset.slot = key([...path, 'item']); }
        e.appendChild(slot);
    }
    return e;
}

/** Liste en grille : même règle que le moteur desktop (engine.js, `grid`). */
function grid(e, cells, p) {
    const gap = p.spacing || 0;
    const dir = p.direction || 'ltr';
    const snake = /snake$/.test(dir);
    const rtl = dir.startsWith('rtl');
    const width = p.itemWidth || (snake ? 50 : null);
    if (width == null) {
        e.style.flexFlow = (rtl ? 'row-reverse' : 'row') + ' wrap';
        for (const c of cells) {
            if (p.maxColumns) { c.style.minWidth = `calc((100% - ${(p.maxColumns - 1) * gap}px) / ${p.maxColumns})`; }
            if (p.minColumns) { c.style.maxWidth = `calc((100% - ${(p.minColumns - 1) * gap}px) / ${p.minColumns})`; }
        }
        return;
    }
    e.style.display = 'grid';
    e.ocPlace = () => {
        let n = Math.floor((e.clientWidth + gap) / (width + gap));
        n = Math.max(p.minColumns || 1, Math.min(p.maxColumns || n, n), 1);
        e.style.gridTemplateColumns = `repeat(${n}, minmax(0, 1fr))`;
        cells.forEach((c, i) => {
            const line = Math.floor(i / n);
            const col = i % n;
            const reversed = rtl !== (snake && line % 2 === 1);
            c.style.gridRow = String(line + 1);
            c.style.gridColumn = String((reversed ? n - 1 - col : col) + 1);
        });
    };
    grids.push(e);
}

/** Grilles à colonnes calculées : placées une fois l'aperçu dans la page. */
const grids = [];

function field(node, p) {
    if (node.type === 'select') {
        const e = el('select', 'oc-input');
        const o = (p.options || []).map(normalize).find(x => x.value === p.value) || normalize((p.options || [])[0]);
        e.appendChild(el('option', '', o ? o.text : ''));
        return e;
    }
    if (node.type === 'radio') {
        const g = el('div', 'oc-radio');
        for (const o of (p.options || []).map(normalize)) {
            const label = el('label');
            const r = el('input');
            r.type = 'radio';
            r.checked = o.value === p.value;
            label.append(r, document.createTextNode(o.text));
            g.appendChild(label);
        }
        return g;
    }
    const multiline = p.kind === 'multiline';
    const e = el(multiline ? 'textarea' : 'input', 'oc-input');
    if (!multiline) { e.type = { password: 'password', email: 'email', number: 'number' }[p.kind] || 'text'; }
    if (node.type === 'date' || node.type === 'time') { e.type = 'text'; e.placeholder = p.placeholder || (node.type === 'date' ? 'AAAA-MM-JJ' : 'HH:MM'); }
    e.value = p.bind ? `{${p.bind}}` : (p.value == null ? '' : String(p.value));
    if (p.placeholder) { e.placeholder = String(p.placeholder); }
    return e;
}

const normalize = o => (o == null ? o : typeof o === 'object' ? { value: String(o.value), text: String(o.text ?? o.value) } : { value: String(o), text: String(o) });

/** Ancre (`anchor` d'un `floating`, `position` d'un `fab`) → (vertical, horizontal). */
const ANCHORS = {
    topStart: ['top', 'start'], topCenter: ['top', 'center'], topEnd: ['top', 'end'],
    startCenter: ['center', 'start'], center: ['center', 'center'], endCenter: ['center', 'end'],
    bottomStart: ['bottom', 'start'], bottomCenter: ['bottom', 'center'], bottomEnd: ['bottom', 'end'],
};

/** Place `e` dans la boîte qui l'entoure (l'écran, ou le parent d'un
 *  `floating` en `position: absolute`), comme layers.js du moteur. */
function place(e, anchor, x, y) {
    const s = e.style, a = ANCHORS[anchor] || ANCHORS.topStart;
    const dx = `${x || 0}px`, dy = `${y || 0}px`;
    s.position = 'absolute';
    if (a[1] === 'start') { s.left = dx; } else if (a[1] === 'end') { s.right = dx; } else { s.left = `calc(50% + ${dx})`; }
    if (a[0] === 'top') { s.top = dy; } else if (a[0] === 'bottom') { s.bottom = dy; } else { s.top = `calc(50% + ${dy})`; }
    s.translate = `${a[1] === 'center' ? '-50%' : '0'} ${a[0] === 'center' ? '-50%' : '0'}`;
}

/** Composants d'écran : bouton flottant à sa position, `floating` à son
 *  ancre ; tiroir et panneau affichés ouverts quand ils sont sélectionnés
 *  (ou `open: true`). */
function screenNode(node, p, path) {
    if (node.type === 'fab') {
        const e = el('button', 'oc-button oc-variant-primary d-fab d-layer');
        if (p.icon) { e.appendChild(icon(p.icon)); }
        if (p.text != null) { e.appendChild(document.createTextNode(String(p.text))); }
        place(e, p.position || 'bottomEnd', 16, 16);
        return e;
    }
    const e = container(node, p, path);
    if (node.type === 'floating') {
        e.classList.add('d-floating');
        if (p.position !== 'absolute') { e.classList.add('d-layer'); }
        place(e, p.anchor, p.x, p.y);
        return e;
    }
    const drawer = node.type === 'drawer';
    e.classList.add(drawer ? 'd-drawer' : 'd-sheet', 'd-layer');
    if (drawer && p.side === 'end') { e.classList.add('d-drawer-end'); }
    e.prepend(el('div', 'd-panel-title', `${drawer ? 'Tiroir' : 'Panneau inférieur'} · ${node.id ?? ''}`));
    const selected = path && S.selection && startsWith(S.selection, path);
    if (!selected && p.open !== true) { e.hidden = true; }
    return e;
}

/** Indicateur d'attente : modèle d'engine.css ou image animée. */
function spinner(p) {
    const size = `${p.size || 32}px`;
    const e = p.src ? el('img', 'oc-spinner') : el('span', `oc-spinner oc-spin-${p.kind || 'circle'}`);
    if (p.src) { e.src = String(p.src); }
    e.style.width = e.style.height = size;
    e.style.setProperty('--oc-spin-size', size);
    const parts = { dots: 3, bars: 4, orbit: 1 }[p.kind] || 0;
    for (let i = 0; i < parts; i++) { e.appendChild(el('i')); }
    return e;
}

/** Élément d'un nœud ; `path` nul : copie fantôme (éléments de liste). */
/** Profondeur des instances en cours de rendu (composant utilisé dans lui-même). */
let nesting = 0;

function build(node, path, axis) {
    if (!node || typeof node !== 'object') { return el('div', 'd-invalid', 'nœud invalide'); }
    // Instance d'un composant : son arbre développé, sélectionnable d'un bloc.
    const expanded = !typeSpec(node.type) && expand(node, nesting);
    if (expanded) {
        nesting++;
        const e = build(expanded, null, axis);
        nesting--;
        e.classList.add('d-instance');
        e.dataset.component = node.type;
        if (path) {
            e.dataset.path = key(path);
            if (S.problems.some(pb => pb.path && key(pb.path) === key(path))) { e.classList.add('d-error'); }
        }
        return e;
    }
    const p = props(node);
    let e;
    switch (node.type) {
        case 'column': case 'row': case 'stack': case 'grid': case 'card': e = container(node, p, path); break;
        case 'divider': e = el('hr', 'oc-divider'); break;
        case 'spacer': e = el('div', 'oc-spacer d-spacer'); break;
        case 'text': e = el('span', 'oc-text', shown(p)); break;
        case 'heading': e = el('h' + (p.level || 2), 'oc-heading', shown(p)); break;
        case 'image': {
            const src = p.bind ? '' : String(p.src || '');
            if (!src) { e = el('div', 'oc-image d-placeholder', p.bind ? `image {${p.bind}}` : 'image'); break; }
            e = el('img', 'oc-image');
            e.src = /^(https?:|data:)/.test(src) ? src : `${S.base}/${src}`;
            e.alt = p.label || '';
            if (p.fit) { e.style.objectFit = p.fit; }
            break;
        }
        case 'icon': e = icon(p.name); break;
        case 'progress': e = el('progress', 'oc-progress'); if (p.value != null) { e.value = p.value; } break;
        case 'list': e = list(node, p, path); break;
        case 'input': case 'date': case 'time': case 'radio': case 'select': e = field(node, p); break;
        case 'checkbox': case 'switch': {
            e = el('label', 'oc-' + node.type);
            const box = el('input');
            box.type = 'checkbox';
            box.checked = !!p.checked;
            e.appendChild(box);
            if (node.type === 'switch') { e.appendChild(el('span', 'oc-switch-track')); }
            if (p.label) { e.appendChild(document.createTextNode(String(p.label))); }
            break;
        }
        case 'slider': {
            e = el('input', 'oc-slider');
            e.type = 'range';
            e.min = p.min ?? 0; e.max = p.max ?? 100; e.step = p.step || 'any';
            e.value = p.value ?? e.min;
            break;
        }
        case 'button': case 'iconButton':
            e = el('button', 'oc-button ' + (node.type === 'iconButton' ? 'oc-icon-button' : 'oc-variant-' + (p.variant || 'primary')));
            if (p.icon) { e.appendChild(icon(p.icon)); }
            if (node.type === 'button' && p.text != null) { e.appendChild(document.createTextNode(String(p.text))); }
            break;
        case 'link':
            e = el('a', 'oc-link' + (p.visited ? ' oc-visited' : ''), p.text == null ? '' : String(p.text));
            if (p.url) { e.title = p.target ? `${p.url} → vue web ${p.target}` : `${p.url} (navigateur)`; }
            break;
        case 'webview':
            e = el('div', 'oc-webview');
            e.dataset.src = p.url || p.file || (p.html != null ? 'html' : '');
            break;
        case 'fab': case 'bottomSheet': case 'drawer': case 'floating': e = screenNode(node, p, path); break;
        case 'spinner': e = spinner(p); break;
        default: e = el('div', 'd-invalid', `type inconnu : ${node.type}`);
    }
    decorate(e, p, axis, isContainer(node) || node.type === 'list');
    if (path) {
        e.dataset.path = key(path);
        if (isContainer(node) || hasSlot(node)) { e.dataset.container = '1'; }
        if (S.problems.some(pb => pb.path && key(pb.path) === key(path))) { e.classList.add('d-error'); }
    }
    return e;
}

/** Élément du nœud le plus profond sous le point (x, y) : les champs de
 *  l'aperçu ignorent la souris (pointer-events), la cible d'un événement
 *  serait leur conteneur. */
export function nodeAt(x, y) {
    let best = null;
    for (const e of document.querySelectorAll('#canvas [data-path]')) {
        const r = e.getBoundingClientRect();
        if (x >= r.left && x <= r.right && y >= r.top && y <= r.bottom && (!best || best.contains(e))) { best = e; }
    }
    return best;
}

/** Affiche l'écran courant dans `#canvas` (barre de titre comprise). */
export function renderCanvas() {
    const canvas = document.getElementById('canvas');
    const frame = document.getElementById('frame');
    // Défilement de l'écran et des conteneurs `scroll` gardé d'un rendu à
    // l'autre : sélectionner un nœud en bas de l'écran ne remonte pas la vue.
    const scrolled = [...canvas.querySelectorAll('.oc-screen, [data-path]')]
        .filter(e => e.scrollTop || e.scrollLeft)
        .map(e => [e.dataset.path || 'screen', e.scrollTop, e.scrollLeft]);
    canvas.replaceChildren();
    grids.length = 0;
    const win = windowProps();
    // Taille d'une vraie fenêtre : `height: fill` et `scroll` s'y comportent
    // comme dans l'application (l'écran défile dans le cadre).
    frame.style.width = S.width + 'px';
    frame.style.height = (S.platform === 'android' ? 780 : Number(win.height) || 720) + 'px';
    canvas.dataset.theme = effectiveTheme();
    canvas.dataset.platform = S.platform;
    canvas.style.setProperty('--oc-primary', win.primary || '#3f51b5');
    if (!S.model) { return; }
    animationTable = animations();
    if (window.OcaraEffects) { OcaraEffects.setTree({ animations: animationTable }); }
    const screen = getIn(S.model, screenPath()) || {};
    const bar = el('div', 'oc-bar');
    const component = editedComponent();
    if (!component && !rootLayout() && S.screen !== startScreen()) { bar.appendChild(icon('back')); }
    bar.appendChild(el('span', 'oc-bar-title', component ? `◆ ${component} — composant` : String((!rootLayout() && screen.title) || win.title || '')));
    bar.dataset.screen = '1';
    canvas.appendChild(bar);
    const body = el('div', 'oc-screen');
    body.dataset.screen = '1';
    const layout = getIn(S.model, layoutPath());
    if (layout && typeof layout === 'object') {
        body.appendChild(build(layout, layoutPath(), 'column'));
    } else {
        const slot = el('div', 'd-empty d-slot d-root-slot', 'Écran vide : déposez un conteneur (column, row…)');
        slot.dataset.slot = key(layoutPath());
        body.appendChild(slot);
    }
    canvas.appendChild(body);
    // Couche de l'écran (comme le moteur) : bouton flottant, tiroir, panneau
    // et `floating` fixe placés dans le cadre, pas dans leur parent ; les
    // copies fantômes des éléments de liste n'en ont pas.
    for (const e of body.querySelectorAll('.d-layer')) { if (e.dataset.path) { canvas.appendChild(e); } else { e.remove(); } }
    grids.forEach(g => g.ocPlace());
    for (const [path, top, left] of scrolled) {
        const e = path === 'screen' ? body : [...canvas.querySelectorAll('[data-path]')].find(x => x.dataset.path === path);
        if (e) { e.scrollTop = top; e.scrollLeft = left; }
    }
}
