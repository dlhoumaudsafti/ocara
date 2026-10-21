import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import { parseToolMessages, runTool, ToolMessage } from './toolrunner';
import { exclusions, gotoLine, isExcluded } from './projects';
import { AutoRunner, currentTarget, describeTarget, Target } from './scope';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Architecture » (barre latérale Ocara) : vérification de
// l'architecture hexagonale par le compilateur (`ocara <fichier>
// --check-architecture`, docs/EBNF.md §33) — couche de chaque fichier selon
// son chemin `<src>/<contexte>/<couche>/…`, sens des dépendances entre
// couches, isolement des contextes, `wiring` et bibliothèques externes, avec
// les règles du bloc `architecture` du fichier, sinon du `main.oc` du projet.
// Chaque fichier vérifié l'est avec tout ce qu'il importe. Périmètre
// (`ocara.architecture.scope`, voir scope.ts) :
//   - `workspace` : chaque point d'entrée `main.oc` de l'espace de travail ;
//   - `folder`    : chaque script du dossier du script affiché ;
//   - `script`    : le script affiché ;
// tests (`*Test.oc`, dossiers `tests`) et blocs runtime exclus. Mode
// automatique (`ocara.architecture.auto`, bouton de l'en-tête) et bouton ▶ ;
// le voyant Architecture de la LED « Release » reprend le résultat.
// ─────────────────────────────────────────────────────────────────────────────

const DEBOUNCE_MS = 600;
const IGNORED = '{**/node_modules/**,**/target/**,**/.ocaraunit_cache/**,**/.git/**}';

export interface ArchitectureSummary {
    target: Target;
    checked: number;
    /** Erreurs par fichier, fichiers triés. */
    files: { file: string; messages: ToolMessage[] }[];
    count: number;
    /** Compilateur introuvable. */
    launchError?: string;
}

type Item =
    | { type: 'running' }
    | { type: 'summary' }
    | { type: 'file'; file: string; messages: ToolMessage[] }
    | { type: 'message'; message: ToolMessage };

/** Fichier vérifié et racine de résolution de ses imports (`--src`). */
interface Entry { file: string; root: string }

export class ArchitectureProvider implements vscode.TreeDataProvider<Item>, vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<Item | undefined>();
    readonly onDidChangeTreeData = this.changed.event;
    private readonly result = new vscode.EventEmitter<ArchitectureSummary | undefined>();
    /** Nouveau résultat — voyant Architecture. */
    readonly onResult = this.result.event;
    private summary: ArchitectureSummary | undefined;
    private running = false;
    private pending = false;
    private readonly auto = new AutoRunner('architecture', 'architecture', () => void this.run(), DEBOUNCE_MS);
    private readonly disposables: vscode.Disposable[] = [];

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            vscode.window.createTreeView('ocara.architecture', { treeDataProvider: this }),
            vscode.commands.registerCommand('ocara.architecture.run', () => this.run()),
            vscode.commands.registerCommand('ocara.architecture.goto', (file: string, line: number, col: number) => gotoLine(file, line, col)),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.architecture.scope')) { this.summary = undefined; this.result.fire(undefined); this.changed.fire(undefined); }
                if (e.affectsConfiguration('ocara.compilerPath') || e.affectsConfiguration('ocara.coverage.exclude')) { this.auto.schedule(true); }
            }),
        );
        context.subscriptions.push(this);
        this.auto.register(context);
    }

    current(): ArchitectureSummary | undefined {
        return this.summary;
    }

    /** Une vérification à la fois ; une demande pendant la vérification la relance à la fin. */
    async run(): Promise<void> {
        if (this.running) { this.pending = true; return; }
        const target = currentTarget('architecture');
        if (!target) {
            vscode.window.showWarningMessage('Ocara : ouvrez un script .oc — l\'architecture porte sur le script affiché ou son dossier (réglage « ocara.architecture.scope »).');
            return;
        }
        this.running = true;
        this.changed.fire(undefined);
        try {
            await vscode.workspace.saveAll(false);
            const entries = target.scope === 'workspace' ? await workspaceEntries()
                : target.folder ? await folderEntries(target.folder)
                : scriptEntries(target.file!);
            this.summary = { ...(await check(entries)), target };
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
                const t = new vscode.TreeItem('Vérification de l\'architecture…');
                t.iconPath = new vscode.ThemeIcon('loading~spin');
                return t;
            }
            case 'summary': {
                if (s!.launchError) {
                    const t = new vscode.TreeItem('Compilateur introuvable');
                    t.tooltip = `${s!.launchError}\nRéglage « ocara.compilerPath »`;
                    t.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('testing.iconFailed'));
                    return t;
                }
                const where = s!.target.scope === 'workspace' ? '' : `${describeTarget(s!.target)} : `;
                const label = s!.checked === 0 ? 'aucun script à vérifier'
                    : s!.count === 0 ? 'architecture respectée' : `${s!.count} erreur(s)`;
                const t = new vscode.TreeItem(`${where}${label}`);
                t.description = `${s!.checked} script(s) vérifié(s)`;
                const ok = s!.count === 0;
                t.iconPath = new vscode.ThemeIcon(ok ? 'pass' : 'error', new vscode.ThemeColor(ok ? 'testing.iconPassed' : 'testing.iconFailed'));
                return t;
            }
            case 'file': {
                const t = new vscode.TreeItem(path.basename(item.file), vscode.TreeItemCollapsibleState.Expanded);
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
                t.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('testing.iconFailed'));
                t.command = { command: 'ocara.architecture.goto', title: 'Aller à l\'erreur', arguments: [m.file, m.line + 1, m.col + 1] };
                return t;
            }
        }
    }

    dispose(): void {
        this.disposables.forEach(d => d.dispose());
    }
}

/** Racine du projet d'un fichier : dossier le plus proche contenant un `main.oc`. */
function projectRoot(file: string): string {
    const top = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file))?.uri.fsPath;
    for (let dir = path.dirname(file); ; dir = path.dirname(dir)) {
        if (fs.existsSync(path.join(dir, 'main.oc'))) { return dir; }
        if (dir === top || dir === path.dirname(dir)) { return path.dirname(file); }
    }
}

/** Script vérifiable : ni test, ni bloc runtime, ni fichier généré, ni exclu. */
function checkable(file: string): boolean {
    const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file))?.uri.fsPath;
    const rel = folder ? path.relative(folder, file) : file;
    return !file.endsWith('.runtime.oc') && !file.endsWith('Test.oc') && !path.basename(file).startsWith('__ocaraunit__')
        && !rel.split(path.sep).includes('tests') && (!folder || !isExcluded(file, folder, exclusions()));
}

async function workspaceEntries(): Promise<Entry[]> {
    return (await vscode.workspace.findFiles('**/main.oc', IGNORED))
        .map(uri => uri.fsPath).filter(checkable).sort()
        .map(file => ({ file, root: path.dirname(file) }));
}

async function folderEntries(folder: string): Promise<Entry[]> {
    return (await vscode.workspace.findFiles(new vscode.RelativePattern(folder, '**/*.oc'), IGNORED))
        .map(uri => uri.fsPath).filter(checkable).sort()
        .map(file => ({ file, root: projectRoot(file) }));
}

function scriptEntries(file: string): Entry[] {
    return checkable(file) ? [{ file, root: projectRoot(file) }] : [];
}

async function check(entries: Entry[]): Promise<Omit<ArchitectureSummary, 'target'>> {
    const byFile = new Map<string, ToolMessage[]>();
    const seen = new Set<string>();
    for (const { file, root } of entries) {
        const result = await runTool('compilerPath', 'ocara', [file, '--check-architecture', '--src', root], root, vscode.Uri.file(file));
        if (result.launchError) { return { checked: entries.length, files: [], count: 0, launchError: result.launchError }; }
        if (result.code === 0) { continue; }
        const messages = parseToolMessages(result.stderr + result.stdout, root);
        const found = messages.length > 0 ? messages
            : [{ file, line: 0, col: 0, severity: 'error' as const, message: 'vérification en échec' }];
        for (const m of found) {
            const key = `${m.file}:${m.line}:${m.col}:${m.message}`;
            if (seen.has(key)) { continue; }
            seen.add(key);
            byFile.set(m.file, [...(byFile.get(m.file) ?? []), m]);
        }
    }
    const files = [...byFile.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([file, messages]) => ({ file, messages }));
    return { checked: entries.length, files, count: files.reduce((n, f) => n + f.messages.length, 0) };
}
