import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import type { Workflow } from './workflow';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Graphe d'appels » : fonctions, méthodes et blocs runtime
// atteignables depuis le script affiché (données `calls` de ocara/workflow),
// regroupés par type (class, struct, interface, module, generic, fonctions
// d'un fichier, runtime) et disposés en colonnes par niveau d'appel. Rendu
// et interactions : media/graph.js. Export Markdown (Mermaid) ou PNG,
// copie Markdown dans le presse-papiers.
// ─────────────────────────────────────────────────────────────────────────────

/** Au-delà, les fonctions atteignables ne sont plus ajoutées au graphe. */
const MAX_MEMBERS = 200;

interface Member { id: string; name: string; params: string; ret: string; static: boolean; file: string; line: number; root: boolean; inactive: boolean }
/** `context`/`layer` : place dans l'architecture hexagonale (absentes hors `src`). */
interface Group { id: string; kind: string; title: string; file: string; context?: string | null; layer?: string | null; members: Member[] }

type Message =
    | { type: 'ready' }
    | { type: 'open'; file: string; line: number }
    | { type: 'copy'; markdown: string }
    | { type: 'save-md'; markdown: string }
    | { type: 'save-png'; data: string };

export class GraphProvider implements vscode.WebviewViewProvider {
    private view: vscode.WebviewView | undefined;
    private data: Workflow | undefined;

    constructor(private readonly extensionUri: vscode.Uri) {}

    register(context: vscode.ExtensionContext, onData: vscode.Event<Workflow>): void {
        context.subscriptions.push(
            vscode.window.registerWebviewViewProvider('ocara.graph', this, { webviewOptions: { retainContextWhenHidden: true } }),
            onData(data => { this.data = data; this.post(); }),
        );
    }

    resolveWebviewView(view: vscode.WebviewView): void {
        this.view = view;
        const media = vscode.Uri.joinPath(this.extensionUri, 'media');
        view.webview.options = { enableScripts: true, localResourceRoots: [media] };
        view.webview.html = html(view.webview, media);
        view.webview.onDidReceiveMessage((msg: Message) => void this.handle(msg));
    }

    private async handle(msg: Message): Promise<void> {
        switch (msg.type) {
            case 'ready':
                this.post();
                break;
            case 'open':
                await vscode.commands.executeCommand('ocara.nav.open', msg.file, msg.line);
                break;
            case 'copy':
                await vscode.env.clipboard.writeText(msg.markdown);
                vscode.window.showInformationMessage('Ocara : graphe d\'appels copié (Markdown, Mermaid).');
                break;
            case 'save-md':
                await this.save('md', Buffer.from(msg.markdown, 'utf8'));
                break;
            case 'save-png':
                await this.save('png', Buffer.from(msg.data.replace(/^data:image\/png;base64,/, ''), 'base64'));
                break;
        }
    }

    private async save(ext: 'md' | 'png', content: Buffer): Promise<void> {
        const base = this.data ? path.join(path.dirname(this.data.file), `${path.basename(this.data.file, '.oc')}.calls.${ext}`) : `calls.${ext}`;
        const target = await vscode.window.showSaveDialog({
            defaultUri: vscode.Uri.file(base),
            filters: ext === 'md' ? { Markdown: ['md'] } : { 'Image PNG': ['png'] },
        });
        if (!target) { return; }
        fs.writeFileSync(target.fsPath, content);
        vscode.window.showInformationMessage(`Ocara : graphe exporté — ${target.fsPath}`);
    }

    /** Membres atteignables depuis les racines, regroupés par type. */
    private post(): void {
        const data = this.data;
        if (!this.view || !data) { return; }
        const order: string[] = [];
        const seen = new Set(data.calls.roots);
        const queue = [...data.calls.roots];
        while (queue.length && order.length < MAX_MEMBERS) {
            const id = queue.shift()!;
            const n = data.calls.nodes[id];
            if (!n) { continue; }
            order.push(id);
            for (const c of n.callees) {
                if (!seen.has(c)) { seen.add(c); queue.push(c); }
            }
        }
        const groups = new Map<string, Group>();
        for (const id of order) {
            const n = data.calls.nodes[id];
            const kind = n.kind ?? 'function';
            const gid = kind === 'function' ? `fn:${n.file}` : kind === 'runtime' ? id : `${n.owner}@${n.file}`;
            const title = kind === 'function' ? `fonctions · ${path.basename(n.file)}` : n.owner ?? n.label;
            if (!groups.has(gid)) { groups.set(gid, { id: gid, kind, title, file: n.file, context: n.context, layer: n.layer, members: [] }); }
            groups.get(gid)!.members.push({
                id, name: n.name ?? n.label, params: n.params ?? '', ret: n.ret ?? '', static: !!n.static,
                file: n.file, line: n.line, root: data.calls.roots.includes(id), inactive: !!n.inactive,
            });
        }
        const ids = new Set(order);
        const edges = order.flatMap(id => data.calls.nodes[id].callees.filter(c => ids.has(c)).map(c => [id, c]));
        void this.view.webview.postMessage({
            file: data.file, title: path.basename(data.file), groups: [...groups.values()], edges, truncated: seen.size > order.length,
        });
    }
}

function html(webview: vscode.Webview, media: vscode.Uri): string {
    const n = Array.from({ length: 24 }, () => Math.floor(Math.random() * 36).toString(36)).join('');
    const script = webview.asWebviewUri(vscode.Uri.joinPath(media, 'graph.js'));
    const style = webview.asWebviewUri(vscode.Uri.joinPath(media, 'graph.css'));
    return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${webview.cspSource} data:; style-src ${webview.cspSource}; script-src 'nonce-${n}';">
<link rel="stylesheet" href="${style}">
</head><body>
  <canvas id="c"></canvas>
  <div class="bar">
    <div class="toggle" title="Style des liens">
      <button id="smooth" class="on" title="Liens souples (courbes)">∿ Souple</button><button id="straight" title="Liens droits en angle">⌐ Droit</button>
    </div>
    <button id="fit" title="Recentrer le graphe">⤢</button>
    <button id="relayout" title="Réorganiser">↻</button>
    <button id="copy" title="Copier en Markdown (Mermaid)">⧉</button>
    <div class="menu">
      <button id="export" title="Exporter le graphe">Exporter ▾</button>
      <div class="items" id="exportItems" hidden>
        <button data-export="md">Markdown (.md)</button><button data-export="png">Image (.png)</button>
      </div>
    </div>
  </div>
  <div class="info" id="info"></div>
  <div class="empty" id="empty">Ouvrez un script Ocara : ses appels s'affichent ici.</div>
  <script nonce="${n}" src="${script}"></script>
</body></html>`;
}
