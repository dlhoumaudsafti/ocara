// Designer `.oc.ui` (src/uidesigner/designer.ts) : point d'entrée de la
// webview. Reçoit le catalogue (`init`) puis le modèle à chaque changement
// du document (`model`) ; la barre d'outils choisit l'écran, la largeur, la
// plateforme et le thème de l'aperçu.

import { S, vscode, onChange, refresh, getIn, key, screens, startScreen, setProp, rootLayout, screenPath, edit } from './model.js';
import { renderCanvas, nodeAt } from './canvas.js';
import { renderPalette } from './palette.js';
import { renderTree, summary } from './tree.js';
import { renderInspector, installInspector, renameScreen } from './inspector.js';
import { fillActionList } from './editors.js';
import { installCanvasDrop, startDrag, endDrag, stageRect } from './dnd.js';
import { contextMenu, installKeys } from './actions.js';
import { ask } from './dialog.js';

/** Largeurs de l'aperçu : seuils des classes de taille (docs/oc-ui.md). */
const SIZES = [['xs', 360], ['sm', 600], ['md', 840], ['lg', 1200], ['xl', 1600]];
const $ = id => document.getElementById(id);

function save() {
    vscode.setState({ screen: S.screen, width: S.width, platform: S.platform, theme: S.theme, collapsed: S.collapsed });
}

// ── barre d'outils ─────────────────────────────────────────────────────────

function renderToolbar() {
    const select = $('screen');
    select.replaceChildren(...screens().map(s => new Option(s + (s === startScreen() ? '  ★' : ''), s)));
    select.value = S.screen;
    $('screenAdd').hidden = rootLayout();
    $('sizes').querySelectorAll('button').forEach(b => b.classList.toggle('on', Number(b.dataset.width) === S.width));
    $('platform').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.platform === S.platform));
    $('theme').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.theme === S.theme));
    const variants = $('variants');
    variants.replaceChildren();
    for (const v of S.variants) {
        const b = document.createElement('button');
        b.textContent = v.size || 'défaut';
        b.title = v.file;
        b.classList.toggle('on', v.current);
        b.addEventListener('click', () => { if (!v.current) { vscode.postMessage({ type: 'open', file: v.file, designer: true }); } });
        variants.appendChild(b);
    }
    const missing = SIZES.map(s => s[0]).filter(s => !S.variants.some(v => v.size === s));
    if (missing.length) {
        const add = document.createElement('select');
        add.title = 'Créer une variante pour une taille d\'écran (copie du fichier par défaut)';
        add.append(new Option('＋ variante', ''), ...missing.map(s => new Option(`${s} (≥ ${SIZES.find(x => x[0] === s)[1]} px)`, s)));
        add.addEventListener('change', () => { if (add.value) { vscode.postMessage({ type: 'variant', size: add.value }); } add.value = ''; });
        variants.appendChild(add);
    }
}

function installToolbar() {
    for (const [name, width] of SIZES) {
        const b = document.createElement('button');
        b.textContent = name;
        b.title = `${width} px`;
        b.dataset.width = String(width);
        b.addEventListener('click', () => { S.width = width; save(); refresh(); });
        $('sizes').appendChild(b);
    }
    $('screen').addEventListener('change', ev => { S.screen = ev.target.value; S.selection = rootLayout() ? null : screenPath(); save(); refresh(); });
    $('screen').addEventListener('dblclick', () => renameScreen(S.screen));
    $('screenAdd').addEventListener('click', async () => {
        const name = await ask('Nom du nouvel écran', '');
        if (!name || !/^[A-Za-z_][A-Za-z0-9_]*$/.test(name) || screens().includes(name)) { return; }
        S.screen = name;
        edit([{ op: 'set', path: ['screens', name], value: { title: name, layout: { type: 'column', spacing: 12, padding: 16, children: [] } } }], ['screens', name, 'layout']);
    });
    $('platform').addEventListener('click', ev => { const p = ev.target.dataset?.platform; if (p) { S.platform = p; save(); refresh(); } });
    $('theme').addEventListener('click', ev => { const t = ev.target.dataset?.theme; if (t) { S.theme = t; save(); refresh(); } });
    $('source').addEventListener('click', () => vscode.postMessage({ type: 'source' }));
    $('check').addEventListener('click', () => vscode.postMessage({ type: 'check' }));
    $('paletteFilter').addEventListener('input', renderPalette);
}

// ── aperçu : sélection, survol, édition du texte sur place ─────────────────

function place(box, target) {
    if (!target) { box.hidden = true; return; }
    const r = stageRect(target);
    box.hidden = false;
    Object.assign(box.style, { left: r.x + 'px', top: r.y + 'px', width: r.w + 'px', height: r.h + 'px' });
}

function nodeEl(path) {
    const k = key(path);
    return [...document.querySelectorAll('#canvas [data-path]')].find(e => e.dataset.path === k);
}

function renderOverlay() {
    const sel = S.selection && getIn(S.model, S.selection);
    const target = sel && sel.type ? nodeEl(S.selection) : null;
    place($('selectBox'), target);
    $('selectTag').textContent = target ? `${sel.type} ${summary(sel)}` : '';
}

const INLINE = { text: 'value', heading: 'value', button: 'text' };

function editInPlace(e, path) {
    const node = getIn(S.model, path);
    const prop = INLINE[node.type];
    if (!prop || node.bind) { return; }
    const target = node.type === 'button' ? [...e.childNodes].find(n => n.nodeType === Node.TEXT_NODE) : e;
    if (!target) { return; }
    const span = node.type === 'button' ? document.createElement('span') : e;
    if (node.type === 'button') { target.replaceWith(span); span.textContent = target.textContent; }
    span.contentEditable = 'true';
    span.focus();
    document.getSelection().selectAllChildren(span);
    const finish = commit => {
        span.contentEditable = 'false';
        span.removeEventListener('blur', onBlur);
        if (commit && span.textContent !== String(node[prop] ?? '')) { setProp(path, prop, span.textContent); } else { refresh(); }
    };
    const onBlur = () => finish(true);
    span.addEventListener('blur', onBlur);
    span.addEventListener('keydown', ev => {
        if (ev.key === 'Enter') { ev.preventDefault(); span.blur(); }
        if (ev.key === 'Escape') { ev.preventDefault(); finish(false); }
    });
}

function installCanvas() {
    const canvas = $('canvas');
    const hit = ev => nodeAt(ev.clientX, ev.clientY);
    let pressed = null; // nœud sous le bouton enfoncé : source d'un glisser
    canvas.addEventListener('mousedown', ev => { pressed = hit(ev); });
    canvas.addEventListener('click', ev => {
        if (ev.target.isContentEditable) { return; }
        const e = hit(ev);
        S.selection = e ? JSON.parse(e.dataset.path) : rootLayout() ? null : screenPath();
        S.tab = 'props';
        refresh();
    });
    canvas.addEventListener('dblclick', ev => { const e = hit(ev); if (e) { editInPlace(e, JSON.parse(e.dataset.path)); } });
    canvas.addEventListener('contextmenu', ev => { const e = hit(ev); if (e) { contextMenu(ev, JSON.parse(e.dataset.path)); } });
    canvas.addEventListener('mousemove', ev => place($('hoverBox'), hit(ev)));
    canvas.addEventListener('mouseleave', () => place($('hoverBox'), null));
    canvas.addEventListener('dragstart', ev => {
        const e = pressed || hit(ev);
        if (!e || ev.target.isContentEditable) { ev.preventDefault(); return; }
        startDrag(ev, { from: JSON.parse(e.dataset.path) });
        ev.dataTransfer.setDragImage(e, 8, 8);
    });
    canvas.addEventListener('dragend', endDrag);
    $('stage').addEventListener('scroll', renderOverlay);
    canvas.addEventListener('scroll', renderOverlay, true); // écran ou conteneur `scroll` défilé
    new ResizeObserver(renderOverlay).observe($('stage'));
    $('frame').addEventListener('click', ev => {
        if (ev.target === $('frame')) { S.selection = rootLayout() ? null : screenPath(); refresh(); }
    });
}

// ── erreurs ────────────────────────────────────────────────────────────────

function renderProblems() {
    const root = $('problems');
    root.replaceChildren();
    root.hidden = !S.problems.length;
    for (const p of S.problems) {
        const row = document.createElement('div');
        row.className = 'd-problem ' + p.severity;
        row.textContent = `${p.severity === 'error' ? '✖' : '⚠'} ${p.file ? p.file + ' ' : ''}l.${p.line} — ${p.message}`;
        if (p.path) {
            row.addEventListener('click', () => {
                let path = p.path;
                while (path.length && !getIn(S.model, path)?.type) { path = path.slice(0, -1); }
                const screen = path[0] === 'screens' ? path[1] : undefined;
                if (screen && screens().includes(screen)) { S.screen = screen; }
                S.selection = path.length ? path : null;
                refresh();
            });
        }
        root.appendChild(row);
    }
    const banner = $('banner');
    banner.hidden = !S.error;
    banner.textContent = S.error ? `YAML invalide : ${S.error} — corrigez-le en mode source.` : '';
}

// ── messages de l'extension ────────────────────────────────────────────────

window.addEventListener('message', ({ data }) => {
    if (data.type === 'init') {
        S.catalog = data.catalog;
        S.icons = data.icons;
        S.base = data.base;
        renderPalette();
        return;
    }
    if (data.type !== 'model') { return; }
    Object.assign(S, { file: data.file, includes: data.includes, variants: data.variants, problems: data.problems, error: data.error, context: data.context || null });
    if (data.model) { S.model = data.model; }
    // Fichier sans écran : le premier composant déposé crée `screens.main`.
    if (!screens().includes(S.screen)) { S.screen = S.model?.start && screens().includes(S.model.start) ? S.model.start : screens()[0] || 'main'; }
    if (S.selection && getIn(S.model, S.selection) === undefined) { S.selection = null; }
    document.title = S.file;
    refresh();
});

onChange(() => {
    if (!S.catalog) { return; }
    save();
    renderToolbar();
    renderCanvas();
    renderTree();
    renderInspector();
    fillActionList();
    renderProblems();
    document.querySelectorAll('#canvas [data-path]').forEach(e => { e.draggable = true; });
    requestAnimationFrame(renderOverlay);
});

const saved = vscode.getState() || {};
S.screen = saved.screen || '';
S.width = saved.width || 360;
S.platform = saved.platform || 'tauri';
S.theme = saved.theme || (document.body.classList.contains('vscode-dark') ? 'dark' : 'light');
S.collapsed = saved.collapsed || {};
installToolbar();
installCanvas();
installCanvasDrop();
installInspector();
installKeys();
vscode.postMessage({ type: 'ready' });
