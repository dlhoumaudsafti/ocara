import * as vscode from 'vscode';
import * as path from 'path';
import { parseToolMessages, runTool, ToolMessage } from './toolrunner';
import { gotoLine, isExcludedFrom, ocFiles, qualityRoots, sectionExclusions } from './projects';
import { fileItem, FileStatus, statusOrder } from './filestatus';
import { AutoRunner, currentTarget, describeTarget, Target } from './scope';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Linter » (barre latérale Ocara) : avertissements d'`ocaracs`,
// fichier par fichier — vert sans avertissement, rouge avec (dépliable, un
// clic ouvre le fichier sur l'avertissement), gris exclu (`ocara.lint.exclude`).
// En `workspace`, dossiers de `ocara.lint.projects`, sinon racines des projets
// testés, sinon dossiers de l'espace de travail (projects.ts). Périmètre
// (`ocara.lint.scope`) : `workspace`, `folder` (dossier du script affiché) ou
// `script` (script affiché seul) ; mode automatique (`ocara.lint.auto`,
// bouton de l'en-tête) et bouton ▶ — voir scope.ts. Le voyant Linter de la
// LED « Release » (quality.ts) reprend le résultat du périmètre courant.
// ─────────────────────────────────────────────────────────────────────────────

const DEBOUNCE_MS = 800;

export interface LintFile {
    file: string;
    status: FileStatus;
    messages: ToolMessage[];
}

export interface LintSummary {
    target: Target;
    roots: string[];
    /** Fichiers analysés (et ignorés), avec leurs avertissements. */
    files: LintFile[];
    /** Avertissements des fichiers non ignorés. */
    count: number;
    /** ocaracs n'a pas pu être lancé. */
    launchError?: string;
}

type Item =
    | { type: 'running' }
    | { type: 'summary' }
    | { type: 'file'; entry: LintFile }
    | { type: 'message'; message: ToolMessage };

export class LinterProvider implements vscode.TreeDataProvider<Item>, vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<Item | undefined>();
    readonly onDidChangeTreeData = this.changed.event;
    private readonly result = new vscode.EventEmitter<LintSummary | undefined>();
    /** Nouveau résultat — voyant Linter. */
    readonly onResult = this.result.event;
    private summary: LintSummary | undefined;
    private running = false;
    private pending = false;
    private readonly auto = new AutoRunner('lint', 'linter', () => void this.run(), DEBOUNCE_MS);
    private readonly disposables: vscode.Disposable[] = [];

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            vscode.window.createTreeView('ocara.linter', { treeDataProvider: this }),
            vscode.commands.registerCommand('ocara.linter.run', () => this.run()),
            vscode.commands.registerCommand('ocara.linter.goto', (file: string, line: number, col: number) => gotoLine(file, line, col)),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.lint.scope')) { this.summary = undefined; this.result.fire(undefined); this.changed.fire(undefined); }
                if (e.affectsConfiguration('ocara.ocaracsPath') || e.affectsConfiguration('ocara.lint.exclude')
                    || e.affectsConfiguration('ocara.lint.projects') || e.affectsConfiguration('ocara.coverage.projects')) { this.auto.schedule(true); }
            }),
        );
        context.subscriptions.push(this);
        this.auto.register(context);
    }

    current(): LintSummary | undefined {
        return this.summary;
    }

    /** Une analyse à la fois ; une demande pendant l'analyse la relance à la fin. */
    async run(): Promise<void> {
        if (this.running) { this.pending = true; return; }
        const target = currentTarget('lint');
        if (!target) {
            vscode.window.showWarningMessage('Ocara : ouvrez un script .oc — le linter porte sur le script affiché ou son dossier (réglage « ocara.lint.scope »).');
            return;
        }
        this.running = true;
        this.changed.fire(undefined);
        try {
            await vscode.workspace.saveAll(false);
            const roots = target.scope === 'workspace' ? await workspaceRoots() : [target.folder ?? target.file!];
            this.summary = { ...(await lint(roots)), target };
            this.result.fire(this.summary);
        } finally {
            this.running = false;
            this.changed.fire(undefined);
            if (this.pending) { this.pending = false; void this.run(); }
        }
    }

    getChildren(item?: Item): Item[] {
        if (item?.type === 'file') { return item.entry.messages.map(message => ({ type: 'message', message })); }
        if (item) { return []; }
        const items: Item[] = [];
        if (this.running) { items.push({ type: 'running' }); }
        if (!this.summary) { return items; }
        items.push({ type: 'summary' });
        this.summary.files.forEach(entry => items.push({ type: 'file', entry }));
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
                const where = s!.target.scope === 'workspace' ? '' : `${describeTarget(s!.target)} : `;
                const t = new vscode.TreeItem(`${where}${ok ? 'aucun avertissement' : `${s!.count} avertissement(s)`}`);
                const ignored = s!.files.filter(f => f.status === 'ignored').length;
                t.description = `${s!.files.length - ignored} fichier(s) analysé(s)${ignored ? `, ${ignored} ignoré(s)` : ''}`;
                t.tooltip = s!.roots.map(r => vscode.workspace.asRelativePath(r)).join('\n');
                t.iconPath = new vscode.ThemeIcon(ok ? 'pass' : 'warning', new vscode.ThemeColor(ok ? 'testing.iconPassed' : 'charts.yellow'));
                return t;
            }
            case 'file': {
                const { file, status, messages } = item.entry;
                const t = fileItem(file, status, status === 'error' ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None);
                const dir = path.dirname(vscode.workspace.asRelativePath(file));
                t.description = status === 'error' ? `${messages.length} · ${dir}` : status === 'ignored' ? `exclu · ${dir}` : dir;
                if (status !== 'error') {
                    t.command = { command: 'ocara.linter.goto', title: 'Ouvrir', arguments: [file, 1, 1] };
                }
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
        this.disposables.forEach(d => d.dispose());
    }
}

/** Dossiers analysés en `workspace` : `ocara.lint.projects`, sinon projets testés ou espace de travail. */
async function workspaceRoots(): Promise<string[]> {
    const explicit = vscode.workspace.getConfiguration('ocara.lint').get<string[]>('projects', []).filter(p => p.trim().length > 0);
    if (explicit.length === 0) { return qualityRoots(); }
    const base = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '';
    return explicit.map(p => path.resolve(base, p));
}

async function lint(roots: string[]): Promise<Omit<LintSummary, 'target'>> {
    const patterns = sectionExclusions('lint');
    const messages = new Map<string, ToolMessage[]>();
    const status = new Map<string, FileStatus>();
    // Projets imbriqués : un avertissement vu deux fois n'est compté qu'une fois.
    const seen = new Set<string>();
    for (const root of roots) {
        // Racine analysée : un dossier, ou un script seul (`script`).
        const cwd = root.endsWith('.oc') ? path.dirname(root) : root;
        for (const file of await ocFiles(root)) {
            status.set(file, isExcludedFrom(file, [cwd], patterns) ? 'ignored' : status.get(file) ?? 'ok');
        }
        const result = await runTool('ocaracsPath', 'ocaracs', [root], cwd);
        if (result.launchError) { return { roots, files: [], count: 0, launchError: result.launchError }; }
        for (const m of parseToolMessages(result.stderr, cwd)) {
            const key = `${m.file}:${m.line}:${m.col}:${m.message}`;
            // Seuls les fichiers du périmètre (ocaracs analyse aussi les imports d'un script) ;
            // fichiers générés par ocaraunit pendant une couverture : jamais listés.
            if (!status.has(m.file) || seen.has(key)) { continue; }
            seen.add(key);
            messages.set(m.file, [...(messages.get(m.file) ?? []), m]);
            if (status.get(m.file) === 'ok') { status.set(m.file, 'error'); }
        }
    }
    const files: LintFile[] = [...status.entries()]
        .map(([file, st]) => ({ file, status: st, messages: st === 'ignored' ? [] : (messages.get(file) ?? []).sort((a, b) => a.line - b.line || a.col - b.col) }))
        .sort((a, b) => statusOrder(a.status) - statusOrder(b.status) || a.file.localeCompare(b.file));
    return { roots, files, count: files.reduce((n, f) => n + f.messages.length, 0) };
}
