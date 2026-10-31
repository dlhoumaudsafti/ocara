// Éditeurs de valeur du panneau des propriétés, un par genre de propriété
// (src/uidesigner/catalog.ts, `Kind`). `editor(spec, value, set, ctx)` rend
// l'élément ; `set(v)` enregistre la valeur, `set(undefined)` retire la clé.
// Les champs texte valident à la sortie du champ ou sur Entrée (une étape
// d'annulation par valeur, pas par frappe).

import { S, actions, styles, webviewIds } from './model.js';
import { icon } from './canvas.js';

export function h(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls) { e.className = cls; }
    if (text !== undefined) { e.textContent = text; }
    return e;
}

function button(text, title, onClick, cls = 'd-mini') {
    const b = h('button', cls, text);
    b.type = 'button';
    b.title = title;
    b.addEventListener('click', onClick);
    return b;
}

/** Champ texte validé au `change` ; `parse` : texte → valeur (`undefined` : retirée). */
function textInput(value, set, { parse = v => (v === '' ? undefined : v), placeholder = '', list, pattern, cls } = {}) {
    const i = h('input', cls || 'd-input');
    i.value = value == null ? '' : Array.isArray(value) ? value.join(' ') : String(value);
    i.placeholder = placeholder;
    if (list) { i.setAttribute('list', list); }
    i.addEventListener('change', () => {
        if (pattern && i.value !== '' && !pattern.test(i.value)) { i.classList.add('d-bad'); return; }
        i.classList.remove('d-bad');
        set(parse(i.value));
    });
    i.addEventListener('keydown', ev => { if (ev.key === 'Enter') { i.blur(); } });
    return i;
}

function numberParse(spec) {
    return v => {
        if (v.trim() === '') { return undefined; }
        const n = Number(v.replace(',', '.'));
        if (Number.isNaN(n)) { return undefined; }
        return spec.kind === 'int' ? Math.round(n) : n;
    };
}

/** Booléen à trois états : défaut (clé absente), oui, non. */
function tri(value, set) {
    const seg = h('div', 'd-seg d-tri');
    for (const [label, v] of [['défaut', undefined], ['oui', true], ['non', false]]) {
        const b = button(label, '', () => set(v));
        if (value === v) { b.classList.add('on'); }
        seg.appendChild(b);
    }
    return seg;
}

function select(values, value, set, empty = '—') {
    const s = h('select', 'd-input');
    s.appendChild(new Option(empty, ''));
    for (const v of values) { s.appendChild(new Option(v, v)); }
    if (value != null && !values.includes(String(value))) { s.appendChild(new Option(String(value), String(value))); }
    s.value = value == null ? '' : String(value);
    s.addEventListener('change', () => set(s.value === '' ? undefined : s.value));
    return s;
}

function color(value, set) {
    const wrap = h('div', 'd-inline');
    const picker = h('input', 'd-color');
    picker.type = 'color';
    picker.value = typeof value === 'string' && /^#[0-9a-fA-F]{6}/.test(value) ? value.slice(0, 7) : '#000000';
    const text = textInput(value, set, { placeholder: '#rrggbb, rgb(…)', pattern: /^(#([0-9a-fA-F]{3,4}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})|rgba?\(\s*\d+\s*,\s*\d+\s*,\s*\d+\s*(,\s*[\d.]+\s*)?\))$/ });
    picker.addEventListener('change', () => set(picker.value + (typeof value === 'string' && value.length === 9 ? value.slice(7) : '')));
    wrap.append(picker, text, button('✕', 'Retirer', () => set(undefined)));
    return wrap;
}

function length(value, set) {
    const wrap = h('div', 'd-inline');
    const seg = h('div', 'd-seg');
    for (const v of ['fill', 'wrap']) {
        const b = button(v, v === 'fill' ? 'Prend la place libre' : 'Taille du contenu', () => set(value === v ? undefined : v));
        if (value === v) { b.classList.add('on'); }
        seg.appendChild(b);
    }
    const num = textInput(typeof value === 'number' ? value : '', set, { parse: numberParse({ kind: 'number' }), placeholder: 'px' });
    num.classList.add('d-narrow');
    wrap.append(seg, num);
    return wrap;
}

/** Marge : `8`, `8 16` (vertical horizontal) ou `4 8 4 8`. */
function spacing(value, set) {
    return textInput(value, set, {
        placeholder: '8 · 8 16 · 4 8 4 8',
        pattern: /^\s*\d+(\.\d+)?(\s+\d+(\.\d+)?){0,3}\s*$/,
        parse: v => {
            const parts = v.trim().split(/\s+/).filter(Boolean).map(Number);
            if (!parts.length) { return undefined; }
            return parts.length === 1 ? parts[0] : parts.length === 3 ? [parts[0], parts[1], parts[2], parts[1]] : parts;
        },
    });
}

function iconPicker(value, set) {
    const wrap = h('div', 'd-icons');
    for (const name of S.catalog.icons) {
        const b = button('', name, () => set(value === name ? undefined : name), 'd-icon-pick' + (value === name ? ' on' : ''));
        b.appendChild(icon(name));
        wrap.appendChild(b);
    }
    return wrap;
}

/** Lignes clé → valeur (`binds`, `webHandlers`). */
function pairs(value, set, keyHint, valueHint, valueList) {
    const map = value && typeof value === 'object' ? { ...value } : {};
    const wrap = h('div', 'd-rows');
    const commit = m => set(Object.keys(m).length ? m : undefined);
    for (const [k, v] of Object.entries(map)) {
        const r = h('div', 'd-inline');
        r.append(
            textInput(k, nk => { const m = {}; for (const [a, b] of Object.entries(map)) { m[a === k ? nk ?? a : a] = b; } commit(m); }, { placeholder: keyHint }),
            textInput(v, nv => { map[k] = nv ?? ''; commit(map); }, { placeholder: valueHint, list: valueList }),
            button('✕', 'Retirer', () => { delete map[k]; commit(map); }),
        );
        wrap.appendChild(r);
    }
    wrap.appendChild(button('＋ ajouter', '', () => { let n = 'cle', i = 1; while (map[n]) { n = `cle${++i}`; } map[n] = ''; commit(map); }, 'd-add'));
    return wrap;
}

/** Champs typés d'une table (`fields`) ; `set(table)`, `undefined` si vide. */
export function fieldsEditor(fields, value, set) {
    const current = value && typeof value === 'object' ? { ...value } : {};
    const wrap = h('div', 'd-rows d-object');
    for (const field of fields) {
        const r = h('div', 'd-field');
        const label = h('label', 'd-label', field.name);
        if (current[field.name] !== undefined) { label.classList.add('d-set'); }
        r.append(label, editor(field, current[field.name], v => {
            if (v === undefined) { delete current[field.name]; } else { current[field.name] = v; }
            set(Object.keys(current).length ? current : undefined);
        }));
        wrap.appendChild(r);
    }
    return wrap;
}

/** Table aux champs typés (table d'état, `transition`) ; liste de tables
 *  (`shadow` : plusieurs ombres, la première devant). */
function objectEditor(spec, value, set) {
    if (!spec.list) { return fieldsEditor(spec.fields, value, set); }
    const list = (Array.isArray(value) ? value : value ? [value] : []).map(o => ({ ...o }));
    const out = () => set(list.length ? list : undefined);
    const wrap = h('div', 'd-rows');
    list.forEach((item, i) => {
        const head = h('div', 'd-inline');
        head.append(h('span', 'd-muted', `${spec.name} ${i + 1}`),
            button('↑', 'Monter', () => { if (i) { list.splice(i - 1, 0, list.splice(i, 1)[0]); out(); } }),
            button('✕', 'Retirer', () => { list.splice(i, 1); out(); }));
        wrap.append(head, fieldsEditor(spec.fields, item, v => { list[i] = v || {}; out(); }));
    });
    wrap.appendChild(button(`＋ ${spec.name}`, '', () => { list.push(spec.name === 'shadow' ? { x: 0, y: 2, blur: 6, color: '#00000040' } : {}); out(); }, 'd-add'));
    return wrap;
}

/** Couleur commune, ou une par thème (`{ light, dark }`) : une seule
 *  valeur pour les deux thèmes s'écrit comme une couleur simple. */
function themeColor(value, set) {
    const pair = value && typeof value === 'object' ? { ...value } : { light: value, dark: value };
    const commit = () => set(pair.light === pair.dark ? pair.light : (pair.light || pair.dark ? { ...(pair.light ? { light: pair.light } : {}), ...(pair.dark ? { dark: pair.dark } : {}) } : undefined));
    const wrap = h('div', 'd-rows');
    for (const theme of ['light', 'dark']) {
        const r = h('div', 'd-inline');
        r.append(h('span', 'd-muted', theme === 'light' ? '☀' : '☾'), color(pair[theme], v => { pair[theme] = v; commit(); }));
        wrap.appendChild(r);
    }
    return wrap;
}

/** `animation` d'un nœud : nom (sous `animations:`) et déclencheur ; le nom
 *  seul se lit `appear`. */
function animationEditor(value, set) {
    const current = typeof value === 'string' ? { name: value } : value && typeof value === 'object' ? { ...value } : {};
    const commit = () => set(!current.name ? undefined : current.on && current.on !== 'appear' ? { name: current.name, on: current.on } : current.name);
    const wrap = h('div', 'd-inline');
    wrap.append(
        select(Object.keys(S.model?.animations || {}), current.name, v => { current.name = v; commit(); }, '— aucune —'),
        select(['appear', 'hover', 'active', 'focus'], current.on, v => { current.on = v; commit(); }, 'appear'),
    );
    return wrap;
}

/** Choix de `radio`/`select` : lignes valeur + libellé. */
function options(value, set) {
    const list = (Array.isArray(value) ? value : []).map(o => (o && typeof o === 'object' ? { value: o.value, text: o.text } : { value: o, text: undefined }));
    const out = () => set(list.length ? list.map(o => (o.text === undefined || o.text === '' ? o.value : { value: o.value, text: o.text })) : undefined);
    const wrap = h('div', 'd-rows');
    list.forEach((o, i) => {
        const r = h('div', 'd-inline');
        r.append(
            textInput(o.value, v => { o.value = v ?? ''; out(); }, { placeholder: 'valeur' }),
            textInput(o.text, v => { o.text = v; out(); }, { placeholder: 'libellé' }),
            button('↑', 'Monter', () => { if (i) { list.splice(i - 1, 0, list.splice(i, 1)[0]); out(); } }),
            button('✕', 'Retirer', () => { list.splice(i, 1); out(); }),
        );
        wrap.appendChild(r);
    });
    wrap.appendChild(button('＋ choix', '', () => { list.push({ value: `choix${list.length + 1}`, text: `Choix ${list.length + 1}` }); out(); }, 'd-add'));
    return wrap;
}

/** Entrées de menu (`menu`, `contextMenu`, sous-menus `items`). */
export function menuEditor(value, set, depth = 0) {
    const list = Array.isArray(value) ? value.map(e => ({ ...e })) : [];
    const out = () => set(list.length ? list : undefined);
    const wrap = h('div', 'd-rows d-menu');
    list.forEach((entry, i) => {
        const box = h('div', 'd-menu-entry');
        const head = h('div', 'd-inline');
        if (entry.separator) {
            head.append(h('span', 'd-muted', '──── séparateur'));
        } else {
            head.append(
                textInput(entry.text, v => { entry.text = v; out(); }, { placeholder: 'texte' }),
                textInput(entry.action, v => { entry.action = v; out(); }, { placeholder: 'action', list: 'actionList', pattern: /^[A-Za-z_][A-Za-z0-9_]*$/ }),
            );
        }
        head.append(
            button('↑', 'Monter', () => { if (i) { list.splice(i - 1, 0, list.splice(i, 1)[0]); out(); } }),
            button('✕', 'Retirer', () => { list.splice(i, 1); out(); }),
        );
        box.appendChild(head);
        if (!entry.separator) {
            const opts = h('div', 'd-inline');
            opts.append(textInput(entry.shortcut, v => { entry.shortcut = v; out(); }, { placeholder: 'raccourci (CmdOrCtrl+S)' }));
            for (const [flag, label] of [['checked', 'cochée'], ['enabled', 'active']]) {
                const c = h('label', 'd-check');
                const box2 = h('input');
                box2.type = 'checkbox';
                box2.checked = flag === 'enabled' ? entry.enabled !== false : !!entry.checked;
                box2.addEventListener('change', () => {
                    if (flag === 'enabled') { entry.enabled = box2.checked ? undefined : false; } else { entry.checked = box2.checked ? true : undefined; }
                    out();
                });
                c.append(box2, document.createTextNode(label));
                opts.appendChild(c);
            }
            box.appendChild(opts);
            if (depth < 3) {
                if (entry.items) {
                    box.appendChild(menuEditor(entry.items, v => { entry.items = v; out(); }, depth + 1));
                } else {
                    box.appendChild(button('＋ sous-menu', '', () => { entry.items = [{ text: 'Entrée' }]; out(); }, 'd-add'));
                }
            }
        }
        wrap.appendChild(box);
    });
    const add = h('div', 'd-inline');
    add.append(
        button('＋ entrée', '', () => { list.push({ text: 'Entrée' }); out(); }, 'd-add'),
        button('＋ séparateur', '', () => { list.push({ separator: true }); out(); }, 'd-add'),
    );
    wrap.appendChild(add);
    return wrap;
}

/** Éditeur de la propriété `spec` ; `ctx.inherited` : valeur venue du style. */
export function editor(spec, value, set, ctx = {}) {
    const placeholder = ctx.inherited !== undefined ? `${ctx.inherited} (style)` : '';
    switch (spec.kind) {
        case 'bool': return tri(value, set);
        case 'int': case 'number': {
            const range = spec.min !== undefined && spec.min > -1e300 ? `${spec.min} – ${spec.max}` : '';
            return textInput(value, v => set(v !== undefined && spec.min !== undefined ? Math.min(spec.max, Math.max(spec.min, v)) : v),
                { parse: numberParse(spec), placeholder: placeholder || range });
        }
        case 'enum': return select(spec.values, value, set);
        case 'color': return color(value, set);
        case 'length': return length(value, set);
        case 'spacing': return spacing(value, set);
        case 'icon': return iconPicker(value, set);
        case 'action': return textInput(value, set, { list: 'actionList', placeholder: 'nom d\'action (self.nom : action du composant)', pattern: /^(self\.)?[A-Za-z_][A-Za-z0-9_]*$/ });
        case 'id': return textInput(value, set, { placeholder: 'identifiant', pattern: /^[A-Za-z_][A-Za-z0-9_]*$/ });
        case 'style': return select(Object.keys(styles()), value, set, '— aucun —');
        case 'field': return textInput(value, set, { placeholder: 'champ de l\'élément (a.b), hors liste : racine.champ' });
        case 'strings': {
            const t = h('textarea', 'd-input');
            t.value = Array.isArray(value) ? value.join('\n') : '';
            t.placeholder = 'une valeur par ligne';
            t.addEventListener('change', () => { const l = t.value.split('\n').map(s => s.trim()).filter(Boolean); set(l.length ? l : undefined); });
            return t;
        }
        case 'options': return options(value, set);
        case 'menu': return menuEditor(value, set);
        case 'actionMap': return pairs(value, set, 'message', 'action', 'actionList');
        case 'binds': return pairs(value, set, 'propriété', 'champ');
        case 'object': return objectEditor(spec, value, set);
        case 'animation': return animationEditor(value, set);
        case 'themeColor': return themeColor(value, set);
        case 'webview': return select(webviewIds(), value, set, '— navigateur du système —');
        case 'iterations': return textInput(value, set, {
            placeholder: '1 · infinite',
            pattern: /^(infinite|[1-9]\d*)$/,
            parse: v => (v === '' ? undefined : v === 'infinite' ? v : Number(v)),
        });
        default: {
            if (spec.name === 'html' || (typeof value === 'string' && value.includes('\n'))) {
                const t = h('textarea', 'd-input');
                t.value = value == null ? '' : String(value);
                t.addEventListener('change', () => set(t.value === '' ? undefined : t.value));
                return t;
            }
            return textInput(value, set, { placeholder });
        }
    }
}

/** Liste des actions connues pour la complétion des champs d'action. */
export function fillActionList() {
    const list = document.getElementById('actionList');
    list.replaceChildren(...actions().map(a => new Option(a)));
}
