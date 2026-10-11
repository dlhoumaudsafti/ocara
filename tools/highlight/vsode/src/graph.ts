import * as vscode from 'vscode';
import type { Workflow } from './workflow';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Graphe d'appels » : fonctions et méthodes atteignables depuis le
// script affiché (données `calls` de la requête ocara/workflow), dessinées
// dans une webview — disposition par forces, zoom (molette), déplacement du
// fond et des nœuds, survol (voisins mis en avant), clic = ouvrir la
// déclaration. Variantes `when` écartées estompées.
// ─────────────────────────────────────────────────────────────────────────────

/** Au-delà, les fonctions atteignables ne sont plus ajoutées au graphe. */
const MAX_NODES = 150;

interface GraphNode { id: string; label: string; file: string; line: number; root: boolean; local: boolean; inactive: boolean }

export class GraphProvider implements vscode.WebviewViewProvider {
    private view: vscode.WebviewView | undefined;
    private data: Workflow | undefined;

    register(context: vscode.ExtensionContext, onData: vscode.Event<Workflow>): void {
        context.subscriptions.push(
            vscode.window.registerWebviewViewProvider('ocara.graph', this, { webviewOptions: { retainContextWhenHidden: true } }),
            onData(data => { this.data = data; this.post(); }),
        );
    }

    resolveWebviewView(view: vscode.WebviewView): void {
        this.view = view;
        view.webview.options = { enableScripts: true };
        view.webview.html = html(nonce());
        view.webview.onDidReceiveMessage((msg: { type: string; file?: string; line?: number }) => {
            if (msg.type === 'open' && msg.file) {
                const position = new vscode.Position(Math.max((msg.line ?? 1) - 1, 0), 0);
                void vscode.window.showTextDocument(vscode.Uri.file(msg.file), { selection: new vscode.Range(position, position), preserveFocus: false });
            } else if (msg.type === 'ready') {
                this.post();
            }
        });
    }

    /** Nœuds atteignables depuis les fonctions du script (parcours en largeur). */
    private post(): void {
        const data = this.data;
        if (!this.view || !data) { return; }
        const nodes: GraphNode[] = [];
        const edges: [string, string][] = [];
        const seen = new Set<string>();
        const queue = [...data.calls.roots];
        queue.forEach(k => seen.add(k));
        while (queue.length && nodes.length < MAX_NODES) {
            const id = queue.shift()!;
            const n = data.calls.nodes[id];
            if (!n) { continue; }
            nodes.push({ id, label: n.label, file: n.file, line: n.line, root: data.calls.roots.includes(id), local: n.file === data.file, inactive: !!n.inactive });
            for (const callee of n.callees) {
                edges.push([id, callee]);
                if (!seen.has(callee)) { seen.add(callee); queue.push(callee); }
            }
        }
        const ids = new Set(nodes.map(n => n.id));
        void this.view.webview.postMessage({ nodes, edges: edges.filter(([a, b]) => ids.has(a) && ids.has(b)), truncated: seen.size > nodes.length });
    }
}

function nonce(): string {
    return Array.from({ length: 24 }, () => Math.floor(Math.random() * 36).toString(36)).join('');
}

function html(n: string): string {
    return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${n}';">
<style>
  html, body { margin: 0; padding: 0; height: 100%; overflow: hidden; color: var(--vscode-foreground); font-family: var(--vscode-font-family); }
  canvas { display: block; width: 100%; height: 100%; cursor: grab; }
  canvas.dragging { cursor: grabbing; }
  .bar { position: absolute; top: 4px; right: 4px; display: flex; gap: 4px; }
  .bar button { background: var(--vscode-button-secondaryBackground); color: var(--vscode-button-secondaryForeground); border: none; padding: 2px 6px; cursor: pointer; border-radius: 2px; font: inherit; font-size: 11px; }
  .info { position: absolute; left: 6px; bottom: 4px; font-size: 11px; opacity: .7; pointer-events: none; }
  .empty { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; opacity: .7; padding: 12px; text-align: center; }
</style></head>
<body>
  <canvas id="c"></canvas>
  <div class="bar"><button id="fit" title="Recentrer le graphe">Recentrer</button><button id="relayout" title="Recalculer la disposition">Disposer</button></div>
  <div class="info" id="info"></div>
  <div class="empty" id="empty">Ouvrez un script Ocara : ses appels s'affichent ici.</div>
<script nonce="${n}">
  const vscode = acquireVsCodeApi();
  const canvas = document.getElementById('c'), ctx = canvas.getContext('2d');
  const info = document.getElementById('info'), empty = document.getElementById('empty');
  const css = name => getComputedStyle(document.body).getPropertyValue(name).trim();
  let nodes = [], edges = [], byId = new Map(), hover = null, drag = null, pan = null, heat = 0;
  let view = { x: 0, y: 0, k: 1 };

  function resize() {
    const r = window.devicePixelRatio || 1;
    canvas.width = canvas.clientWidth * r; canvas.height = canvas.clientHeight * r;
    ctx.setTransform(r, 0, 0, r, 0, 0); draw();
  }

  function load(msg) {
    const old = byId;
    nodes = msg.nodes.map((n, i) => {
      const prev = old.get(n.id);
      const a = (i / Math.max(msg.nodes.length, 1)) * Math.PI * 2;
      return { ...n, x: prev ? prev.x : Math.cos(a) * 120 + Math.random() * 10, y: prev ? prev.y : Math.sin(a) * 120 + Math.random() * 10, vx: 0, vy: 0 };
    });
    byId = new Map(nodes.map(n => [n.id, n]));
    edges = msg.edges.map(([a, b]) => [byId.get(a), byId.get(b)]).filter(([a, b]) => a && b);
    ctx.font = '12px ' + css('--vscode-font-family');
    for (const n of nodes) { n.w = ctx.measureText(n.label).width + 16; n.h = 22; }
    empty.style.display = nodes.length ? 'none' : 'flex';
    info.textContent = nodes.length + ' fonction(s), ' + edges.length + ' appel(s)' + (msg.truncated ? ' — graphe tronqué' : '');
    heat = old.size ? 120 : 300;
    if (!old.size) { fit(); }
    tick();
  }

  // Forces : répulsion entre nœuds, ressorts sur les appels, gravité au centre.
  function step() {
    for (const a of nodes) {
      for (const b of nodes) {
        if (a === b) continue;
        let dx = a.x - b.x, dy = a.y - b.y, d2 = dx * dx + dy * dy || 1;
        const f = 2400 / d2;
        a.vx += dx * f / Math.sqrt(d2); a.vy += dy * f / Math.sqrt(d2);
      }
      a.vx -= a.x * 0.004; a.vy -= a.y * 0.004;
    }
    for (const [a, b] of edges) {
      if (a === b) continue;
      const dx = b.x - a.x, dy = b.y - a.y, d = Math.sqrt(dx * dx + dy * dy) || 1;
      const f = (d - 140) * 0.02;
      a.vx += dx / d * f; a.vy += dy / d * f; b.vx -= dx / d * f; b.vy -= dy / d * f;
    }
    for (const n of nodes) {
      if (n === drag?.node) { n.vx = n.vy = 0; continue; }
      n.vx *= 0.6; n.vy *= 0.6; n.x += n.vx; n.y += n.vy;
    }
  }

  function tick() {
    if (heat <= 0) { draw(); return; }
    heat--; step(); draw(); requestAnimationFrame(tick);
  }

  function neighbors(n) {
    const set = new Set([n]);
    for (const [a, b] of edges) { if (a === n) set.add(b); if (b === n) set.add(a); }
    return set;
  }

  function draw() {
    const w = canvas.clientWidth, h = canvas.clientHeight;
    ctx.clearRect(0, 0, w, h);
    ctx.save();
    ctx.translate(w / 2 + view.x, h / 2 + view.y); ctx.scale(view.k, view.k);
    const near = hover ? neighbors(hover) : null;
    const fg = css('--vscode-foreground'), accent = css('--vscode-button-background') || '#0e639c';
    const local = css('--vscode-charts-blue') || '#3794ff', other = css('--vscode-charts-purple') || '#b180d7';
    for (const [a, b] of edges) {
      const lit = near && near.has(a) && near.has(b) && (a === hover || b === hover);
      ctx.globalAlpha = near && !lit ? 0.12 : (a.inactive || b.inactive ? 0.3 : 0.7);
      ctx.strokeStyle = lit ? accent : fg; ctx.lineWidth = lit ? 2 : 1;
      arrow(a, b);
    }
    ctx.font = '12px ' + css('--vscode-font-family'); ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    for (const n of nodes) {
      ctx.globalAlpha = (near && !near.has(n) ? 0.2 : 1) * (n.inactive ? 0.4 : 1);
      ctx.fillStyle = css('--vscode-editor-background') || '#1e1e1e';
      ctx.strokeStyle = n.root ? accent : n.local ? local : other;
      ctx.lineWidth = n === hover ? 3 : n.root ? 2 : 1.5;
      ctx.setLineDash(n.inactive ? [4, 3] : []);
      roundRect(n.x - n.w / 2, n.y - n.h / 2, n.w, n.h, 5); ctx.fill(); ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = fg; ctx.fillText(n.label, n.x, n.y + 1);
    }
    ctx.restore(); ctx.globalAlpha = 1;
  }

  function roundRect(x, y, w, h, r) {
    ctx.beginPath(); ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
  }

  /** Flèche de a vers le bord du rectangle de b. */
  function arrow(a, b) {
    if (a === b) { ctx.beginPath(); ctx.arc(a.x + a.w / 2, a.y - a.h / 2, 8, 0, Math.PI * 1.5); ctx.stroke(); return; }
    const dx = b.x - a.x, dy = b.y - a.y;
    const t = Math.min(Math.abs((b.w / 2) / (dx || 1e-6)), Math.abs((b.h / 2) / (dy || 1e-6)));
    const ex = b.x - dx * t, ey = b.y - dy * t, ang = Math.atan2(dy, dx);
    ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(ex, ey); ctx.stroke();
    ctx.beginPath(); ctx.moveTo(ex, ey);
    ctx.lineTo(ex - 8 * Math.cos(ang - 0.4), ey - 8 * Math.sin(ang - 0.4));
    ctx.lineTo(ex - 8 * Math.cos(ang + 0.4), ey - 8 * Math.sin(ang + 0.4));
    ctx.closePath(); ctx.fillStyle = ctx.strokeStyle; ctx.fill();
  }

  function toWorld(e) {
    const r = canvas.getBoundingClientRect();
    return { x: (e.clientX - r.left - r.width / 2 - view.x) / view.k, y: (e.clientY - r.top - r.height / 2 - view.y) / view.k };
  }

  function nodeAt(p) {
    for (let i = nodes.length - 1; i >= 0; i--) {
      const n = nodes[i];
      if (Math.abs(p.x - n.x) <= n.w / 2 && Math.abs(p.y - n.y) <= n.h / 2) return n;
    }
    return null;
  }

  function fit() {
    if (!nodes.length) return;
    const xs = nodes.map(n => n.x), ys = nodes.map(n => n.y);
    const minX = Math.min(...xs) - 80, maxX = Math.max(...xs) + 80, minY = Math.min(...ys) - 30, maxY = Math.max(...ys) + 30;
    view.k = Math.min(2, Math.max(0.2, Math.min(canvas.clientWidth / (maxX - minX), canvas.clientHeight / (maxY - minY))));
    view.x = -((minX + maxX) / 2) * view.k; view.y = -((minY + maxY) / 2) * view.k;
    draw();
  }

  canvas.addEventListener('mousedown', e => {
    const p = toWorld(e), n = nodeAt(p);
    if (n) { drag = { node: n, moved: false }; } else { pan = { x: e.clientX - view.x, y: e.clientY - view.y }; }
    canvas.classList.add('dragging');
  });
  window.addEventListener('mousemove', e => {
    if (drag) {
      const p = toWorld(e); drag.node.x = p.x; drag.node.y = p.y; drag.moved = true;
      if (heat < 30) { heat = 30; tick(); } else draw();
      return;
    }
    if (pan) { view.x = e.clientX - pan.x; view.y = e.clientY - pan.y; draw(); return; }
    const n = nodeAt(toWorld(e));
    if (n !== hover) {
      hover = n; canvas.title = n ? n.label + ' — ' + n.file.split(/[\\\\/]/).pop() + ':' + n.line + (n.inactive ? ' (variante when écartée)' : '') : '';
      draw();
    }
  });
  window.addEventListener('mouseup', () => {
    if (drag && !drag.moved) vscode.postMessage({ type: 'open', file: drag.node.file, line: drag.node.line });
    drag = null; pan = null; canvas.classList.remove('dragging');
  });
  canvas.addEventListener('wheel', e => {
    e.preventDefault();
    const r = canvas.getBoundingClientRect(), mx = e.clientX - r.left - r.width / 2, my = e.clientY - r.top - r.height / 2;
    const k = Math.min(3, Math.max(0.15, view.k * (e.deltaY < 0 ? 1.12 : 1 / 1.12)));
    view.x = mx - (mx - view.x) * k / view.k; view.y = my - (my - view.y) * k / view.k; view.k = k;
    draw();
  }, { passive: false });
  document.getElementById('fit').addEventListener('click', fit);
  document.getElementById('relayout').addEventListener('click', () => {
    nodes.forEach((n, i) => { const a = i / nodes.length * Math.PI * 2; n.x = Math.cos(a) * 150; n.y = Math.sin(a) * 150; });
    heat = 300; tick(); setTimeout(fit, 600);
  });
  window.addEventListener('message', e => load(e.data));
  new ResizeObserver(resize).observe(canvas);
  resize();
  vscode.postMessage({ type: 'ready' });
</script></body></html>`;
}
