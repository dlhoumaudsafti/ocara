// Aperçu de l'écran : même DOM et mêmes classes `oc-*` que le moteur desktop
// (runtime_tauri/src/ui/engine.js, apparence : engine.css copié), chaque
// élément marqué du chemin de son nœud (`data-path`) pour la sélection et le
// glisser-déposer. Les champs ne réagissent pas : un clic sélectionne.

import { S, getIn, key, startsWith, styles, windowProps, layoutPath, screenPath, startScreen, rootLayout, isContainer, hasSlot, axisOf } from './model.js';

const FLEX_ALIGN = { start: 'flex-start', center: 'center', end: 'flex-end', stretch: 'stretch' };
const px = v => (typeof v === 'number' ? v + 'px' : v);
const spacing = v => (Array.isArray(v) ? v.map(px).join(' ') : px(v));

function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) { e.className = cls; }
    if (text !== undefined) { e.textContent = text; }
    return e;
}

/** Propriétés effectives : style nommé, nœud, surcharge de la plateforme affichée. */
function props(node) {
    const named = node.style && styles()[node.style];
    const out = {};
    for (const layer of [named || {}, node, node[S.platform] || {}]) {
        for (const k in layer) { if (k !== 'tauri' && k !== 'android') { out[k] = layer[k]; } }
    }
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
}

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
        for (let i = 0; i < 2; i++) {
            const ghost = build(node.item, null, 'column');
            ghost.classList.add('oc-list-item', 'd-ghost');
            e.appendChild(ghost);
        }
    } else {
        const slot = el('div', 'd-empty d-slot', 'Gabarit d\'élément : déposez un composant');
        if (path) { slot.dataset.slot = key([...path, 'item']); }
        e.appendChild(slot);
    }
    return e;
}

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

/** Composants Material d'Android : masqués en aperçu desktop (comme le
 *  moteur, sélectionnables dans la structure) ; en aperçu Android, bouton
 *  flottant en bas à droite, tiroir et panneau affichés ouverts quand ils
 *  sont sélectionnés (ou `open: true`). */
function android(node, p, path) {
    if (S.platform !== 'android') {
        const e = el('span');
        e.hidden = true;
        return e;
    }
    if (node.type === 'android.fab') {
        const e = el('button', 'oc-button oc-variant-primary d-fab');
        if (p.icon) { e.appendChild(icon(p.icon)); }
        if (p.text != null) { e.appendChild(document.createTextNode(String(p.text))); }
        return e;
    }
    const e = container(node, p, path);
    const drawer = node.type === 'android.drawer';
    e.classList.add(drawer ? 'd-drawer' : 'd-sheet');
    e.prepend(el('div', 'd-android-title', `${drawer ? 'Tiroir' : 'Panneau inférieur'} · ${node.id ?? ''}`));
    const selected = path && S.selection && startsWith(S.selection, path);
    if (!selected && p.open !== true) { e.hidden = true; }
    return e;
}

/** Élément d'un nœud ; `path` nul : copie fantôme (éléments de liste). */
function build(node, path, axis) {
    if (!node || typeof node !== 'object') { return el('div', 'd-invalid', 'nœud invalide'); }
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
        case 'webview':
            e = el('div', 'oc-webview');
            e.dataset.src = p.url || p.file || (p.html != null ? 'html' : '');
            break;
        case 'android.fab': case 'android.bottomSheet': case 'android.drawer': e = android(node, p, path); break;
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
    const win = windowProps();
    // Taille d'une vraie fenêtre : `height: fill` et `scroll` s'y comportent
    // comme dans l'application (l'écran défile dans le cadre).
    frame.style.width = S.width + 'px';
    frame.style.height = (S.platform === 'android' ? 780 : Number(win.height) || 720) + 'px';
    canvas.dataset.theme = S.theme;
    canvas.dataset.platform = S.platform;
    canvas.style.setProperty('--oc-primary', win.primary || '#3f51b5');
    if (!S.model) { return; }
    const screen = getIn(S.model, screenPath()) || {};
    const bar = el('div', 'oc-bar');
    if (!rootLayout() && S.screen !== startScreen()) { bar.appendChild(icon('back')); }
    bar.appendChild(el('span', 'oc-bar-title', String((!rootLayout() && screen.title) || win.title || '')));
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
    for (const [path, top, left] of scrolled) {
        const e = path === 'screen' ? body : [...canvas.querySelectorAll('[data-path]')].find(x => x.dataset.path === path);
        if (e) { e.scrollTop = top; e.scrollLeft = left; }
    }
}
