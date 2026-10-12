// Graphe d'appels Ocara (webview de l'encart « Graphe d'appels », voir
// src/graph.ts) : boîtes par type (class, struct, interface, module,
// generic, fonctions d'un fichier, runtime) listant les méthodes utilisées,
// disposées en colonnes par niveau d'appel ; liens souples ou droits ;
// zoom, déplacement, survol, clic pour ouvrir ; export Markdown / PNG.

(function () {
    const vscode = acquireVsCodeApi();
    const canvas = document.getElementById('c');
    const ctx = canvas.getContext('2d');
    const $ = id => document.getElementById(id);
    const css = name => getComputedStyle(document.body).getPropertyValue(name).trim();

    const ROW = 20, HEADER = 26, PAD = 10, COL_GAP = 90, ROW_GAP = 26, MAX_PARAMS = 46;
    const KINDS = {
        class: ['--vscode-charts-blue', '#3794ff', 'class'],
        struct: ['', '#2bb3a3', 'struct'],
        interface: ['--vscode-charts-green', '#89d185', 'interface'],
        module: ['--vscode-charts-orange', '#d18616', 'module'],
        generic: ['--vscode-charts-purple', '#b180d7', 'generic'],
        function: ['--vscode-charts-yellow', '#cca700', ''],
        runtime: ['--vscode-charts-red', '#f14c4c', ''],
        builtin: ['--vscode-disabledForeground', '#8b8b8b', ''],
    };
    const WIRING = ['--vscode-charts-purple', '#c586c0'];

    const saved = vscode.getState() || {};
    let linkStyle = saved.linkStyle || 'smooth';
    let showBuiltins = saved.showBuiltins !== false;
    let lastMsg = null;
    let data = null, groups = [], members = new Map(), edges = [];
    let view = { x: 0, y: 0, k: 1 }, hover = null, drag = null, pan = null;
    let positions = new Map();

    const color = kind => { const [v, fallback] = KINDS[kind] || KINDS.function; return (v && css(v)) || fallback; };
    const font = (weight) => `${weight || ''} 12px ${css('--vscode-font-family') || 'sans-serif'}`.trim();

    // ── Données et disposition ────────────────────────────────────────────

    function memberText(m) {
        if (m.group.kind === 'runtime') return m.name;
        if (m.group.kind === 'builtin') return `${m.name}(…)${m.ret && m.ret !== 'void' ? ': ' + m.ret : ''}`;
        let params = m.params.length > MAX_PARAMS ? m.params.slice(0, MAX_PARAMS - 1) + '…' : m.params;
        return `${m.static ? '::' : ''}${m.name}(${params})${m.ret ? ': ' + m.ret : ''}`;
    }

    function headerText(g) {
        const label = KINDS[g.kind] ? KINDS[g.kind][2] : '';
        return label ? `${label} ${g.title}` : g.title;
    }

    function load(raw) {
        lastMsg = raw;
        // Appels builtin (ocara.*) masquables depuis la barre d'outils.
        const hidden = new Set(showBuiltins ? [] : raw.groups.filter(g => g.kind === 'builtin').flatMap(g => g.members.map(m => m.id)));
        const msg = { ...raw, groups: raw.groups.filter(g => showBuiltins || g.kind !== 'builtin'), edges: raw.edges.filter(([a, b]) => !hidden.has(a) && !hidden.has(b)) };
        data = msg;
        members = new Map();
        groups = msg.groups.map(g => ({ ...g }));
        ctx.font = font();
        for (const g of groups) {
            g.members = g.members.map(m => { const mm = { ...m, group: g }; members.set(m.id, mm); return mm; });
            ctx.font = font('bold');
            let w = ctx.measureText(headerText(g)).width;
            ctx.font = font();
            for (const m of g.members) w = Math.max(w, ctx.measureText(memberText(m)).width + 14);
            g.w = Math.ceil(w + PAD * 2);
            g.h = HEADER + g.members.length * ROW + 6;
        }
        edges = msg.edges.map(([a, b, kind]) => [members.get(a), members.get(b), kind || 'call']).filter(([a, b]) => a && b);
        $('empty').style.display = groups.length ? 'none' : 'flex';
        $('info').textContent = `${msg.title} — ${members.size} fonction(s), ${edges.length} appel(s)${msg.truncated ? ' — graphe tronqué' : ''}`;
        const fresh = groups.some(g => !positions.has(g.id));
        layout(!fresh);
        if (fresh) fit(); else draw();
    }

    // ── Architecture hexagonale : cadres par contexte et par couche ───────

    const LAYERS = ['infrastructure', 'application', 'domain'];
    const FPAD = 14, LAYER_HEAD = 22, CTX_HEAD = 26, LAYER_GAP = 60, CTX_GAP = 44;
    const layered = () => groups.some(g => g.layer);
    const yellow = () => document.body.classList.contains('vscode-light') ? '#A86500' : '#FFCC00';
    const contexts = () => [...new Set(groups.filter(g => g.layer).map(g => g.context))];

    function place(g, x, y, keep) {
        const old = positions.get(g.id);
        if (keep && old) { g.x = old.x; g.y = old.y; } else { g.x = x; g.y = y; }
        positions.set(g.id, { x: g.x, y: g.y });
    }

    /** Hors `src` (runtimes, configuration) en première colonne ; puis une
     *  rangée par contexte : couches infrastructure → application → domain,
     *  dans le sens des dépendances. */
    function layeredLayout(keep) {
        const outside = groups.filter(g => !g.layer && g.kind !== 'builtin');
        let y = 0;
        for (const g of outside) { place(g, 0, y, keep); y += g.h + ROW_GAP; }
        let right = 0;
        const x0 = outside.length ? Math.max(...outside.map(g => g.w)) + COL_GAP : 0;
        y = 0;
        for (const ctx of contexts()) {
            let x = x0 + FPAD, rowH = 0;
            for (const layer of LAYERS) {
                const gs = groups.filter(g => g.context === ctx && g.layer === layer);
                if (!gs.length) continue;
                let gy = y + CTX_HEAD + LAYER_HEAD + FPAD;
                for (const g of gs) { place(g, x + FPAD, gy, keep); gy += g.h + ROW_GAP; }
                rowH = Math.max(rowH, gy - y);
                x += Math.max(...gs.map(g => g.w)) + FPAD * 2 + LAYER_GAP;
            }
            right = Math.max(right, x);
            y += rowH + FPAD + CTX_GAP;
        }
        // Builtins appelés : dernière colonne, hors cadres.
        y = 0;
        for (const g of groups.filter(g => g.kind === 'builtin')) { place(g, right + COL_GAP, y, keep); y += g.h + ROW_GAP; }
    }

    /** Cadres recalculés depuis la position des boîtes (suivent le glisser). */
    function frames() {
        const box = gs => {
            const x0 = Math.min(...gs.map(g => g.x)), y0 = Math.min(...gs.map(g => g.y));
            const x1 = Math.max(...gs.map(g => g.x + g.w)), y1 = Math.max(...gs.map(g => g.y + g.h));
            return { x0, y0, x1, y1 };
        };
        const out = [];
        for (const ctx of contexts()) {
            const layers = [];
            for (const layer of LAYERS) {
                const gs = groups.filter(g => g.context === ctx && g.layer === layer);
                if (!gs.length) continue;
                const b = box(gs);
                layers.push({ kind: 'layer', label: layer, x: b.x0 - FPAD, y: b.y0 - FPAD - LAYER_HEAD, w: b.x1 - b.x0 + FPAD * 2, h: b.y1 - b.y0 + FPAD * 2 + LAYER_HEAD });
            }
            const x0 = Math.min(...layers.map(f => f.x)), y0 = Math.min(...layers.map(f => f.y));
            const x1 = Math.max(...layers.map(f => f.x + f.w)), y1 = Math.max(...layers.map(f => f.y + f.h));
            out.push({ kind: 'context', label: `contexte ${ctx}`, x: x0 - FPAD, y: y0 - CTX_HEAD, w: x1 - x0 + FPAD * 2, h: y1 - y0 + CTX_HEAD + FPAD }, ...layers);
        }
        return out;
    }

    function paintFrames(c) {
        const fg = css('--vscode-foreground') || '#ccc';
        c.textBaseline = 'middle'; c.textAlign = 'left';
        for (const f of frames()) {
            const layer = f.kind === 'layer';
            c.globalAlpha = layer ? 1 : 0.45;
            c.strokeStyle = layer ? yellow() : fg; c.lineWidth = layer ? 1.6 : 1;
            c.setLineDash(layer ? [] : [6, 4]);
            roundRect(c, f.x, f.y, f.w, f.h, layer ? 8 : 10); c.stroke();
            c.setLineDash([]);
            c.globalAlpha = layer ? 1 : 0.8;
            c.fillStyle = layer ? yellow() : fg; c.font = font('bold');
            c.fillText(f.label, f.x + 10, f.y + (layer ? LAYER_HEAD : CTX_HEAD) / 2 + 2);
        }
        c.globalAlpha = 1;
    }

    /** Colonnes par niveau d'appel (plus long chemin depuis les racines),
     *  ordre dans une colonne par barycentre des voisins. */
    function layout(keep) {
        if (layered()) { layeredLayout(keep); return; }
        const out = new Map(groups.map(g => [g.id, new Set()]));
        const inn = new Map(groups.map(g => [g.id, new Set()]));
        for (const [a, b] of edges) {
            if (a.group !== b.group) { out.get(a.group.id).add(b.group.id); inn.get(b.group.id).add(a.group.id); }
        }
        // Arcs retour (cycles) ignorés pour le calcul des niveaux.
        const state = new Map(), forward = new Map(groups.map(g => [g.id, []]));
        const visit = id => {
            state.set(id, 1);
            for (const t of out.get(id)) {
                if (state.get(t) === 1) continue;
                forward.get(id).push(t);
                if (!state.has(t)) visit(t);
            }
            state.set(id, 2);
        };
        const roots = groups.filter(g => g.members.some(m => m.root)).map(g => g.id);
        roots.forEach(id => { if (!state.has(id)) visit(id); });
        groups.forEach(g => { if (!state.has(g.id)) visit(g.id); });
        const level = new Map(groups.map(g => [g.id, 0]));
        for (let pass = 0; pass < groups.length; pass++) {
            let changed = false;
            for (const [id, targets] of forward) {
                for (const t of targets) {
                    if (level.get(t) < level.get(id) + 1) { level.set(t, level.get(id) + 1); changed = true; }
                }
            }
            if (!changed) break;
        }
        const columns = [];
        for (const g of groups) (columns[level.get(g.id)] ||= []).push(g);
        const index = new Map();
        const reindex = () => columns.forEach(col => col && col.forEach((g, i) => index.set(g.id, i)));
        reindex();
        for (let sweep = 0; sweep < 6; sweep++) {
            const down = sweep % 2 === 0;
            const order = down ? columns : [...columns].reverse();
            for (const col of order) {
                if (!col) continue;
                const bary = g => {
                    const near = [...(down ? inn : out).get(g.id)].map(id => index.get(id));
                    return near.length ? near.reduce((a, b) => a + b, 0) / near.length : index.get(g.id);
                };
                col.sort((a, b) => bary(a) - bary(b));
                reindex();
            }
        }
        let x = 0;
        for (const col of columns) {
            if (!col) continue;
            const width = Math.max(...col.map(g => g.w));
            const height = col.reduce((s, g) => s + g.h, 0) + ROW_GAP * (col.length - 1);
            let y = -height / 2;
            for (const g of col) {
                const old = positions.get(g.id);
                if (keep && old) { g.x = old.x; g.y = old.y; } else { g.x = x; g.y = y; }
                positions.set(g.id, { x: g.x, y: g.y });
                y += g.h + ROW_GAP;
            }
            x += width + COL_GAP;
        }
    }

    // ── Dessin ────────────────────────────────────────────────────────────

    const rowY = m => m.group.y + HEADER + m.group.members.indexOf(m) * ROW + ROW / 2 + 2;

    function roundRect(c, x, y, w, h, r) {
        c.beginPath(); c.moveTo(x + r, y); c.arcTo(x + w, y, x + w, y + h, r); c.arcTo(x + w, y + h, x, y + h, r);
        c.arcTo(x, y + h, x, y, r); c.arcTo(x, y, x + w, y, r); c.closePath();
    }

    function head(c, x, y, angle) {
        c.beginPath(); c.moveTo(x, y);
        c.lineTo(x - 8 * Math.cos(angle - 0.38), y - 8 * Math.sin(angle - 0.38));
        c.lineTo(x - 8 * Math.cos(angle + 0.38), y - 8 * Math.sin(angle + 0.38));
        c.closePath(); c.fill();
    }

    /** Lien de la ligne de l'appelant à celle de l'appelé. */
    function edge(c, a, b) {
        const ay = rowY(a), by = rowY(b);
        const ax = a.group.x + a.group.w, bxLeft = b.group.x;
        if (a.group === b.group) {
            // Appel interne au type : boucle sur le flanc droit.
            const x = ax, bend = 18 + Math.abs(by - ay) * 0.15;
            c.beginPath(); c.moveTo(x, ay);
            if (linkStyle === 'smooth') c.bezierCurveTo(x + bend * 2, ay, x + bend * 2, by, x + 2, by);
            else { c.lineTo(x + bend, ay); c.lineTo(x + bend, by); c.lineTo(x + 2, by); }
            c.stroke(); head(c, x + 1, by, Math.PI);
            return;
        }
        if (bxLeft > ax + 10) {
            c.beginPath(); c.moveTo(ax, ay);
            if (linkStyle === 'smooth') {
                const dx = Math.max(30, (bxLeft - ax) / 2);
                c.bezierCurveTo(ax + dx, ay, bxLeft - dx, by, bxLeft - 1, by);
            } else {
                const xm = ax + (bxLeft - ax) / 2;
                c.lineTo(xm, ay); c.lineTo(xm, by); c.lineTo(bxLeft - 1, by);
            }
            c.stroke(); head(c, bxLeft - 1, by, 0);
            return;
        }
        // Appel vers une colonne précédente : contournement par le bas.
        const below = Math.max(a.group.y + a.group.h, b.group.y + b.group.h) + 18;
        c.beginPath(); c.moveTo(ax, ay);
        if (linkStyle === 'smooth') {
            c.bezierCurveTo(ax + 60, ay, ax + 60, below, (ax + bxLeft) / 2, below);
            c.bezierCurveTo(bxLeft - 60, below, bxLeft - 60, by, bxLeft - 1, by);
        } else {
            c.lineTo(ax + 16, ay); c.lineTo(ax + 16, below); c.lineTo(bxLeft - 16, below); c.lineTo(bxLeft - 16, by); c.lineTo(bxLeft - 1, by);
        }
        c.stroke(); head(c, bxLeft - 1, by, 0);
    }

    function paint(c, highlight) {
        const fg = css('--vscode-foreground') || '#ccc';
        const bg = css('--vscode-editor-background') || '#1e1e1e';
        const accent = css('--vscode-focusBorder') || css('--vscode-button-background') || '#0e639c';
        const lit = new Set();
        if (highlight) {
            for (const e of edges) {
                const [a, b] = e;
                if (highlight === a || highlight === b || highlight === a.group || highlight === b.group) lit.add(e);
            }
        }
        if (layered()) paintFrames(c);
        for (const e of edges) {
            const on = lit.has(e);
            const wiring = e[2] === 'wiring';
            c.globalAlpha = highlight && !on ? 0.15 : (e[0].inactive || e[1].inactive ? 0.35 : wiring ? 0.95 : 0.75);
            c.strokeStyle = c.fillStyle = on ? accent : wiring ? ((WIRING[0] && css(WIRING[0])) || WIRING[1]) : fg;
            c.lineWidth = on ? 2 : wiring ? 1.6 : 1.1;
            // wiring (interface → implémentation) : tirets longs ; variante écartée : pointillés.
            c.setLineDash(wiring ? [8, 4] : e[0].inactive || e[1].inactive ? [5, 4] : []);
            edge(c, e[0], e[1]);
        }
        c.setLineDash([]);
        for (const g of groups) {
            const col = color(g.kind);
            c.globalAlpha = 1;
            c.fillStyle = bg; c.strokeStyle = col; c.lineWidth = highlight === g ? 2.5 : 1.5;
            roundRect(c, g.x, g.y, g.w, g.h, 6); c.fill(); c.stroke();
            c.save(); roundRect(c, g.x, g.y, g.w, HEADER, 6); c.clip();
            c.globalAlpha = 0.28; c.fillStyle = col; c.fillRect(g.x, g.y, g.w, HEADER); c.restore();
            c.globalAlpha = 1; c.fillStyle = fg; c.font = font('bold'); c.textBaseline = 'middle'; c.textAlign = 'left';
            c.fillText(headerText(g), g.x + PAD, g.y + HEADER / 2 + 1);
            c.font = font();
            for (const m of g.members) {
                const y = rowY(m);
                if (highlight === m) { c.globalAlpha = 0.18; c.fillStyle = accent; c.fillRect(g.x + 1, y - ROW / 2, g.w - 2, ROW); }
                c.globalAlpha = m.inactive ? 0.4 : 1;
                if (m.root) { c.fillStyle = accent; c.beginPath(); c.arc(g.x + PAD - 2, y, 3, 0, Math.PI * 2); c.fill(); }
                c.fillStyle = fg;
                c.font = m.inactive ? `italic ${font()}` : font();
                c.fillText(memberText(m), g.x + PAD + 6, y + 1);
            }
        }
        c.globalAlpha = 1;
    }

    function draw() {
        const w = canvas.clientWidth, h = canvas.clientHeight;
        ctx.clearRect(0, 0, w, h);
        ctx.save();
        ctx.translate(w / 2 + view.x, h / 2 + view.y); ctx.scale(view.k, view.k);
        paint(ctx, hover);
        ctx.restore();
    }

    function bounds() {
        const fs = layered() ? frames() : [];
        const xs = groups.flatMap(g => [g.x, g.x + g.w]).concat(fs.flatMap(f => [f.x, f.x + f.w]));
        const ys = groups.flatMap(g => [g.y, g.y + g.h]).concat(fs.flatMap(f => [f.y, f.y + f.h]));
        return { x0: Math.min(...xs) - 40, x1: Math.max(...xs) + 60, y0: Math.min(...ys) - 30, y1: Math.max(...ys) + 40 };
    }

    function fit() {
        if (!groups.length) { draw(); return; }
        const b = bounds();
        view.k = Math.min(1.5, Math.max(0.15, Math.min(canvas.clientWidth / (b.x1 - b.x0), canvas.clientHeight / (b.y1 - b.y0))));
        view.x = -((b.x0 + b.x1) / 2) * view.k; view.y = -((b.y0 + b.y1) / 2) * view.k;
        draw();
    }

    function resize() {
        const r = window.devicePixelRatio || 1;
        canvas.width = canvas.clientWidth * r; canvas.height = canvas.clientHeight * r;
        ctx.setTransform(r, 0, 0, r, 0, 0);
        draw();
    }

    // ── Interactions ──────────────────────────────────────────────────────

    function toWorld(e) {
        const r = canvas.getBoundingClientRect();
        return { x: (e.clientX - r.left - r.width / 2 - view.x) / view.k, y: (e.clientY - r.top - r.height / 2 - view.y) / view.k };
    }

    function hit(p) {
        for (let i = groups.length - 1; i >= 0; i--) {
            const g = groups[i];
            if (p.x < g.x || p.x > g.x + g.w || p.y < g.y || p.y > g.y + g.h) continue;
            const m = g.members.find(m => Math.abs(rowY(m) - p.y) <= ROW / 2);
            return { group: g, member: m || null };
        }
        return null;
    }

    canvas.addEventListener('mousedown', e => {
        const p = toWorld(e), h = hit(p);
        if (h) drag = { group: h.group, member: h.member, dx: p.x - h.group.x, dy: p.y - h.group.y, moved: false };
        else pan = { x: e.clientX - view.x, y: e.clientY - view.y };
        canvas.classList.add('dragging');
    });
    window.addEventListener('mousemove', e => {
        if (drag) {
            const p = toWorld(e);
            drag.group.x = p.x - drag.dx; drag.group.y = p.y - drag.dy; drag.moved = true;
            positions.set(drag.group.id, { x: drag.group.x, y: drag.group.y });
            draw(); return;
        }
        if (pan) { view.x = e.clientX - pan.x; view.y = e.clientY - pan.y; draw(); return; }
        const h = hit(toWorld(e));
        const target = h ? (h.member || h.group) : null;
        canvas.classList.toggle('pointer', !!(h && h.member));
        if (target !== hover) {
            hover = target;
            canvas.title = h && h.member ? `${h.member.name} — ${h.member.file ? h.member.file.split(/[\\/]/).pop() + ':' + h.member.line : 'builtin ' + h.group.title}${h.member.inactive ? ' (variante when écartée)' : ''}` : '';
            draw();
        }
    });
    window.addEventListener('mouseup', () => {
        if (drag && !drag.moved && drag.member && drag.member.file) vscode.postMessage({ type: 'open', file: drag.member.file, line: drag.member.line });
        drag = null; pan = null; canvas.classList.remove('dragging');
    });
    canvas.addEventListener('wheel', e => {
        e.preventDefault();
        const r = canvas.getBoundingClientRect(), mx = e.clientX - r.left - r.width / 2, my = e.clientY - r.top - r.height / 2;
        const k = Math.min(3, Math.max(0.1, view.k * (e.deltaY < 0 ? 1.12 : 1 / 1.12)));
        view.x = mx - (mx - view.x) * k / view.k; view.y = my - (my - view.y) * k / view.k; view.k = k;
        draw();
    }, { passive: false });

    function setLinkStyle(style) {
        linkStyle = style;
        $('smooth').classList.toggle('on', style === 'smooth');
        $('straight').classList.toggle('on', style === 'straight');
        vscode.setState({ ...vscode.getState(), linkStyle: style });
        draw();
    }
    $('builtins').classList.toggle('on', showBuiltins);
    $('builtins').addEventListener('click', () => {
        showBuiltins = !showBuiltins;
        $('builtins').classList.toggle('on', showBuiltins);
        vscode.setState({ ...vscode.getState(), showBuiltins });
        if (lastMsg) { positions.clear(); load(lastMsg); fit(); }
    });
    $('smooth').addEventListener('click', () => setLinkStyle('smooth'));
    $('straight').addEventListener('click', () => setLinkStyle('straight'));
    $('fit').addEventListener('click', fit);
    $('relayout').addEventListener('click', () => { positions.clear(); layout(false); fit(); });
    $('copy').addEventListener('click', () => groups.length && vscode.postMessage({ type: 'copy', markdown: markdown() }));
    $('export').addEventListener('click', e => { e.stopPropagation(); $('exportItems').hidden = !$('exportItems').hidden; });
    window.addEventListener('click', () => { $('exportItems').hidden = true; });
    document.querySelectorAll('[data-export]').forEach(b => b.addEventListener('click', () => {
        if (!groups.length) return;
        if (b.dataset.export === 'md') vscode.postMessage({ type: 'save-md', markdown: markdown() });
        else vscode.postMessage({ type: 'save-png', data: png() });
    }));

    // ── Exports ───────────────────────────────────────────────────────────

    const esc = s => s.replace(/"/g, '#quot;').replace(/</g, '#lt;').replace(/>/g, '#gt;');

    /** Graphe Mermaid (flowchart) dans un document Markdown. */
    function markdown() {
        const id = new Map();
        const lines = ['flowchart LR'];
        const styles = [];
        const group = (g, indent) => {
            const gi = groups.indexOf(g);
            lines.push(`${indent}subgraph g${gi}["${esc(headerText(g))}"]`, `${indent}    direction TB`);
            g.members.forEach(m => { id.set(m, `n${id.size}`); lines.push(`${indent}    ${id.get(m)}["${esc(memberText(m))}"]`); });
            lines.push(`${indent}end`);
            styles.push(`    style g${gi} stroke:${KINDS[g.kind] ? KINDS[g.kind][1] : '#888'},stroke-width:2px`);
        };
        // Architecture hexagonale : contexte > couche (cadre jaune) > types.
        groups.filter(g => !g.layer).forEach(g => group(g, '    '));
        contexts().forEach((ctx, ci) => {
            lines.push(`    subgraph c${ci}["contexte ${esc(ctx)}"]`, '        direction LR');
            LAYERS.forEach((layer, li) => {
                const gs = groups.filter(g => g.context === ctx && g.layer === layer);
                if (!gs.length) return;
                lines.push(`        subgraph c${ci}l${li}["${layer}"]`, '            direction TB');
                gs.forEach(g => group(g, '            '));
                lines.push('        end');
                styles.push(`    style c${ci}l${li} stroke:#FFCC00,stroke-width:2px`);
            });
            lines.push('    end');
            styles.push(`    style c${ci} stroke-dasharray:6 4`);
        });
        lines.push(...styles);
        for (const [a, b, kind] of edges) lines.push(`    ${id.get(a)} ${kind === 'wiring' ? '-. wiring .->' : a.inactive || b.inactive ? '-.->' : '-->'} ${id.get(b)}`);
        const roots = [...id].filter(([m]) => m.root).map(([, n]) => n);
        const inactive = [...id].filter(([m]) => m.inactive).map(([, n]) => n);
        lines.push('    classDef root font-weight:bold', '    classDef inactive stroke-dasharray:4 3,opacity:0.5');
        if (roots.length) lines.push(`    class ${roots.join(',')} root`);
        if (inactive.length) lines.push(`    class ${inactive.join(',')} inactive`);
        return `# Graphe d'appels — ${data.title}\n\n\`\`\`mermaid\n${lines.join('\n')}\n\`\`\`\n`;
    }

    /** Image PNG du graphe entier (échelle 2), fond de l'éditeur. */
    function png() {
        const b = bounds(), scale = 2;
        const off = document.createElement('canvas');
        off.width = (b.x1 - b.x0) * scale; off.height = (b.y1 - b.y0) * scale;
        const c = off.getContext('2d');
        c.fillStyle = css('--vscode-editor-background') || '#1e1e1e';
        c.fillRect(0, 0, off.width, off.height);
        c.scale(scale, scale); c.translate(-b.x0, -b.y0);
        paint(c, null);
        return off.toDataURL('image/png');
    }

    window.addEventListener('message', e => load(e.data));
    new ResizeObserver(resize).observe(canvas);
    setLinkStyle(linkStyle);
    resize();
    vscode.postMessage({ type: 'ready' });
})();
