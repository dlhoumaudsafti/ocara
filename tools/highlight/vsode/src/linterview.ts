import * as vscode from 'vscode';
import * as path from 'path';
import { parseToolMessages, runTool, ToolMessage } from './toolrunner';
import { gotoLine, qualityRoots } from './projects';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Linter » (barre latérale Ocara) : avertissements d'`ocaracs` sur
// les projets de l'espace de travail (racines des projets testés, sinon
// dossiers de l'espace de travail — voir projects.ts), regroupés par
// fichier ; un clic ouvre le fichier sur l'avertissement. Relancé à
// l'ouverture, à chaque enregistrement d'un `.oc` (réglage
// `ocara.lint.enable`) et par le bouton de l'encart. Son résultat alimente
// le voyant Linter de la LED « Release » (quality.ts).
// ─────────────────────────────────────────────────────────────────────────────

const DEBOUNCE_MS = 800;

export interface LintSummary {
    roots: string[];
    /** Avertissements par fichier (chemins absolus), fichiers triés. */
    files: { file: string; messages: ToolMessage[] }[];
    count: number;
    /** ocaracs n'a pas pu être lancé. */
    launchError?: string;
}

type Item =
    | { type: 'running' }
    | { type: 'summary' }
    | { type: 'file'; file: string; messages: ToolMessage[] }
    | { type: 'message'; message: ToolMessage };

export class LinterProvider implements vscode.TreeDataProvider<Item>, vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<Item | undefined>();
    readonly onDidChangeTreeData = this.changed.event;
    private readonly result = new vscode.EventEmitter<LintSummary>();
    /** Nouveau résultat — voyant Linter. */
    readonly onResult = this.result.event;
    private summary: LintSummary | undefined;
    private running = false;
    private pending = false;
    private timer: NodeJS.Timeout | undefined;
    private readonly disposables: vscode.Disposable[] = [];

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            vscode.window.createTreeView('ocara.linter', { treeDataProvider: this }),
            vscode.commands.registerCommand('ocara.linter.run', () => this.run()),
            vscode.commands.registerCommand('ocara.linter.goto', (file: string, line: number, col: number) => gotoLine(file, line, col)),
            vscode.workspace.onDidSaveTextDocument(doc => { if (doc.languageId === 'ocara') { this.schedule(); } }),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.lint') || e.affectsConfiguration('ocara.ocaracsPath') || e.affectsConfiguration('ocara.coverage')) { this.schedule(); }
            }),
        );
        context.subscriptions.push(this);
        this.schedule();
    }

    current(): LintSummary | undefined {
        return this.summary;
    }

    private schedule(): void {
        if (!vscode.workspace.getConfiguration('ocara').get<boolean>('lint.enable', true)) { return; }
        clearTimeout(this.timer);
        this.timer = setTimeout(() => void this.run(), DEBOUNCE_MS);
    }

    /** Une analyse à la fois ; une demande pendant l'analyse la relance à la fin. */
    async run(): Promise<void> {
        if (this.running) { this.pending = true; return; }
        this.running = true;
        this.changed.fire(undefined);
        try {
            this.summary = await lint(await qualityRoots());
            this.result.fire(this.summary);
        } finally {
            this.running = false;
            this.changed.fire(undefined);
            if (this.pending) { this.pending = false; void this.run(); }
        }
    }

    getChildren(item?: Item): Item[] {
        if (item?.type === 'file') { return item.messages.map(message => ({ type: 'message', message })); }
        if (item) { return []; }
        const items: Item[] = [];
        if (this.running) { items.push({ type: 'running' }); }
        if (!this.summary) { return items; }
        items.push({ type: 'summary' });
        this.summary.files.forEach(f => items.push({ type: 'file', ...f }));
        return items;
    }

    getTreeItem(item: Item): vscode.TreeItem {
        const s = this.summary;
        switch (item.type) {
            case 'running': {
                const t = new vscode.TreeItem('Analyse ocaracs en cours…');
                t.iconPath = new vscode.ThemeIcon('loading~spin');
                return t;
            }
            case 'summary': {
                if (s!.launchError) {
                    const t = new vscode.TreeItem('ocaracs introuvable');
                    t.tooltip = `${s!.launchError}\nRéglage « ocara.ocaracsPath »`;
                    t.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('testing.iconFailed'));
                    return t;
                }
                const ok = s!.count === 0;
                const t = new vscode.TreeItem(ok ? 'Aucun avertissement' : `${s!.count} avertissement(s)`);
                t.description = `${s!.files.length} fichier(s) · ${s!.roots.length} dossier(s) analysé(s)`;
                t.tooltip = s!.roots.map(r => vscode.workspace.asRelativePath(r)).join('\n');
                t.iconPath = new vscode.ThemeIcon(ok ? 'pass' : 'warning', new vscode.ThemeColor(ok ? 'testing.iconPassed' : 'charts.yellow'));
                return t;
            }
            case 'file': {
                const t = new vscode.TreeItem(path.basename(item.file), vscode.TreeItemCollapsibleState.Collapsed);
                t.description = `${item.messages.length} · ${path.dirname(vscode.workspace.asRelativePath(item.file))}`;
                t.tooltip = item.file;
                t.resourceUri = vscode.Uri.file(item.file);
                t.iconPath = vscode.ThemeIcon.File;
                return t;
            }
            case 'message': {
                const m = item.message;
                const t = new vscode.TreeItem(m.message);
                t.description = `ligne ${m.line + 1}`;
                t.tooltip = `${m.file}:${m.line + 1}:${m.col + 1}\n${m.message}`;
                t.iconPath = new vscode.ThemeIcon(m.severity === 'error' ? 'error' : 'warning', new vscode.ThemeColor(m.severity === 'error' ? 'testing.iconFailed' : 'charts.yellow'));
                t.command = { command: 'ocara.linter.goto', title: 'Aller à l\'avertissement', arguments: [m.file, m.line + 1, m.col + 1] };
                return t;
            }
        }
    }

    dispose(): void {
        clearTimeout(this.timer);
        this.disposables.forEach(d => d.dispose());
    }
}

async function lint(roots: string[]): Promise<LintSummary> {
    const byFile = new Map<string, ToolMessage[]>();
    // Projets imbriqués : un fichier analysé deux fois n'est compté qu'une fois.
    const seen = new Set<string>();
    for (const root of roots) {
        const result = await runTool('ocaracsPath', 'ocaracs', [root], root);
        if (result.launchError) { return { roots, files: [], count: 0, launchError: result.launchError }; }
        // Fichiers générés par ocaraunit pendant une analyse de couverture : ignorés.
        for (const m of parseToolMessages(result.stderr, root)) {
            const key = `${m.file}:${m.line}:${m.col}:${m.message}`;
            if (path.basename(m.file).startsWith('__ocaraunit__') || seen.has(key)) { continue; }
            seen.add(key);
            byFile.set(m.file, [...(byFile.get(m.file) ?? []), m]);
        }
    }
    const files = [...byFile.entries()].sort(([a], [b]) => a.localeCompare(b))
        .map(([file, messages]) => ({ file, messages: messages.sort((a, b) => a.line - b.line || a.col - b.col) }));
    return { roots, files, count: files.reduce((n, f) => n + f.messages.length, 0) };
}
