import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import { runTool, ToolMessage } from './toolrunner';
import { gotoLine, isExcludedFrom, sectionExclusions } from './projects';
import { fileItem, FileStatus, headerPercent, statusOrder } from './filestatus';
import { AutoRunner, currentTarget, describeTarget, isWithin, Target } from './scope';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Architecture » (barre latérale Ocara) : pour chaque projet (dossier
// d'un `main.oc`, ou `ocara.architecture.projects`), le rapport du
// compilateur (`ocara main.oc --architecture-report`) — type d'architecture
// (hexagonale ou permissive, docs/EBNF.md §33), contextes du répertoire
// hexagonal et, dans chacun, les couches application, infrastructure et
// domain sous leur alias du projet (`infrastructure (infra)`), avec leurs
// scripts : vert si leurs imports et `wiring` respectent l'architecture,
// rouge sinon (dépliable, un clic ouvre le fichier sur la violation), gris
// s'ils sont ignorés (tests, blocs runtime, `ocara.architecture.exclude`).
// Les scripts hors du répertoire hexagonal sont regroupés à part. Une
// architecture permissive n'impose aucune contrainte : 100 %.
// Pourcentage de l'en-tête : scripts vérifiés sans violation. Périmètre
// (`ocara.architecture.scope`, voir scope.ts) : projets de l'espace de
// travail, projet(s) du dossier ou du script affiché (scripts du dossier ou
// script seul). Mode automatique (`ocara.architecture.auto`) et bouton ▶ ;
// le voyant Architecture de la LED « Release » reprend le résultat.
// ─────────────────────────────────────────────────────────────────────────────

const DEBOUNCE_MS = 600;
const IGNORED = '{**/node_modules/**,**/target/**,**/.ocaraunit_cache/**,**/.git/**}';
/** Ordre d'affichage des couches. */
const LAYERS = ['application', 'infrastructure', 'domain'];

interface ReportFile {
    path: string;
    place: { kind: 'layer' | 'outside' | 'unclassified' | 'external'; context?: string; layer?: string };
    errors: { file: string; line: number; col: number; message: string }[];
}

interface Report {
    mode: 'hexagonal' | 'permissive';
    src?: string;
    layers?: Record<string, string>;
    shared?: string[];
    configErrors?: { file: string; line: number; col: number; message: string }[];
    files?: ReportFile[];
}

export interface ArchitectureFile {
    file: string;
    status: FileStatus;
    /** Contexte et couche canonique (`layer`), ou `outside` / `unclassified`. */
    context?: string;
    layer?: string;
    messages: ToolMessage[];
}

export interface ArchitectureProject {
    root: string;
    mode: 'hexagonal' | 'permissive' | 'failed';
    src?: string;
    /** Couche canonique → alias du projet. */
    layers: Record<string, string>;
    shared: string[];
    configErrors: ToolMessage[];
    files: ArchitectureFile[];
    /** Rapport illisible (compilateur en échec). */
    error?: string;
}

export interface ArchitectureSummary {
    target: Target;
    projects: ArchitectureProject[];
    /** Scripts vérifiés (non ignorés) des projets hexagonaux. */
    checked: number;
    /** Violations (et erreurs de configuration) comptées. */
    count: number;
    /** Part des scripts vérifiés sans violation ; 100 % sans contrainte. */
    percent: number;
    launchError?: string;
}

type Item =
    | { type: 'running' }
    | { type: 'notice'; label: string; tooltip?: string; error: boolean }
    | { type: 'project'; project: ArchitectureProject }
    | { type: 'context'; project: ArchitectureProject; context: string }
    | { type: 'layer'; project: ArchitectureProject; context: string; layer: string }
    | { type: 'outside'; project: ArchitectureProject }
    | { type: 'file'; entry: ArchitectureFile }
    | { type: 'message'; message: ToolMessage };

export class ArchitectureProvider implements vscode.TreeDataProvider<Item>, vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<Item | undefined>();
    readonly onDidChangeTreeData = this.changed.event;
    private readonly result = new vscode.EventEmitter<ArchitectureSummary | undefined>();
    /** Nouveau résultat — voyant Architecture. */
    readonly onResult = this.result.event;
    private summary: ArchitectureSummary | undefined;
    private view: vscode.TreeView<Item> | undefined;
    private running = false;
    private pending = false;
    private readonly auto = new AutoRunner('architecture', 'architecture', () => void this.run(), DEBOUNCE_MS);
    private readonly disposables: vscode.Disposable[] = [];

    register(context: vscode.ExtensionContext): void {
        this.view = vscode.window.createTreeView('ocara.architecture', { treeDataProvider: this });
        this.disposables.push(
            this.view,
            vscode.commands.registerCommand('ocara.architecture.run', () => this.run()),
            vscode.commands.registerCommand('ocara.architecture.goto', (file: string, line: number, col: number) => gotoLine(file, line, col)),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.architecture.scope')) { this.setSummary(undefined); }
                if (e.affectsConfiguration('ocara.compilerPath') || e.affectsConfiguration('ocara.architecture.exclude')
                    || e.affectsConfiguration('ocara.architecture.projects')) { this.auto.schedule(true); }
            }),
        );
        context.subscriptions.push(this);
        this.auto.register(context);
    }

    current(): ArchitectureSummary | undefined {
        return this.summary;
    }

    private setSummary(summary: ArchitectureSummary | undefined): void {
        this.summary = summary;
        if (this.view) { headerPercent(this.view, 'Architecture', summary?.percent); }
        this.result.fire(summary);
        this.changed.fire(undefined);
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
            this.setSummary(await analyze(target));
        } finally {
            this.running = false;
            this.changed.fire(undefined);
            if (this.pending) { this.pending = false; void this.run(); }
        }
    }

    // ── Arbre ────────────────────────────────────────────────────────────────

    getChildren(item?: Item): Item[] {
        if (!item) {
            const items: Item[] = [];
            if (this.running) { items.push({ type: 'running' }); }
            const s = this.summary;
            if (!s) { return items; }
            if (s.launchError) {
                items.push({ type: 'notice', label: 'Compilateur introuvable', tooltip: `${s.launchError}\nRéglage « ocara.compilerPath »`, error: true });
                return items;
            }
            if (s.projects.length === 0) {
                items.push({ type: 'notice', label: `Aucun projet (main.oc) pour : ${describeTarget(s.target)}`, error: false });
            }
            s.projects.forEach(project => items.push({ type: 'project', project }));
            return items;
        }
        switch (item.type) {
            case 'project': {
                const p = item.project;
                if (p.mode === 'failed') { return [{ type: 'notice', label: 'Rapport d\'architecture indisponible', tooltip: p.error, error: true }]; }
                if (p.mode === 'permissive') { return [{ type: 'notice', label: 'Architecture permissive : aucune contrainte', error: false }]; }
                const items: Item[] = p.configErrors.map(message => ({ type: 'message', message }));
                contexts(p).forEach(context => items.push({ type: 'context', project: p, context }));
                if (p.files.some(f => !f.layer)) { items.push({ type: 'outside', project: p }); }
                return items;
            }
            case 'context':
                return LAYERS.filter(layer => item.project.files.some(f => f.context === item.context && f.layer === layer))
                    .map(layer => ({ type: 'layer', project: item.project, context: item.context, layer }));
            case 'layer':
                return sortFiles(item.project.files.filter(f => f.context === item.context && f.layer === item.layer))
                    .map(entry => ({ type: 'file', entry }));
            case 'outside':
                return sortFiles(item.project.files.filter(f => !f.layer)).map(entry => ({ type: 'file', entry }));
            case 'file':
                return item.entry.messages.map(message => ({ type: 'message', message }));
            default:
                return [];
        }
    }

    getTreeItem(item: Item): vscode.TreeItem {
        const expanded = vscode.TreeItemCollapsibleState.Expanded;
        switch (item.type) {
            case 'running': {
                const t = new vscode.TreeItem('Vérification de l\'architecture…');
                t.iconPath = new vscode.ThemeIcon('loading~spin');
                return t;
            }
            case 'notice': {
                const t = new vscode.TreeItem(item.label);
                t.tooltip = item.tooltip;
                t.iconPath = new vscode.ThemeIcon(item.error ? 'error' : 'info', item.error ? new vscode.ThemeColor('testing.iconFailed') : undefined);
                return t;
            }
            case 'project': {
                const p = item.project;
                const t = new vscode.TreeItem(path.basename(p.root), expanded);
                const stats = groupStats(p.files);
                t.description = p.mode === 'permissive' ? 'permissive · 100 %'
                    : p.mode === 'failed' ? 'rapport indisponible'
                    : `hexagonale · src: ${p.src} · ${percentText(stats)}`;
                t.tooltip = p.root;
                t.iconPath = new vscode.ThemeIcon(p.mode === 'permissive' ? 'unlock' : 'layers',
                    new vscode.ThemeColor(p.mode === 'failed' || stats.errors > 0 || p.configErrors.length > 0 ? 'testing.iconFailed' : 'testing.iconPassed'));
                return t;
            }
            case 'context': {
                const shared = item.project.files.some(f => f.context === item.context && isShared(item.project, f));
                const t = new vscode.TreeItem(item.context, expanded);
                const stats = groupStats(item.project.files.filter(f => f.context === item.context));
                t.description = `${shared ? 'partagé · ' : ''}${percentText(stats)}`;
                t.iconPath = new vscode.ThemeIcon('symbol-namespace', new vscode.ThemeColor(stats.errors > 0 ? 'testing.iconFailed' : 'testing.iconPassed'));
                return t;
            }
            case 'layer': {
                const alias = item.project.layers[item.layer] ?? item.layer;
                const t = new vscode.TreeItem(alias === item.layer ? item.layer : `${item.layer} (${alias})`, expanded);
                const stats = groupStats(item.project.files.filter(f => f.context === item.context && f.layer === item.layer));
                t.description = percentText(stats);
                t.iconPath = new vscode.ThemeIcon('folder', new vscode.ThemeColor(stats.errors > 0 ? 'testing.iconFailed' : 'testing.iconPassed'));
                return t;
            }
            case 'outside': {
                const t = new vscode.TreeItem(`Hors de ${item.project.src}/`, vscode.TreeItemCollapsibleState.Collapsed);
                const stats = groupStats(item.project.files.filter(f => !f.layer));
                t.description = percentText(stats);
                t.tooltip = 'Scripts du projet hors du répertoire hexagonal (configuration, points d\'entrée, tests…)';
                t.iconPath = new vscode.ThemeIcon('folder', new vscode.ThemeColor(stats.errors > 0 ? 'testing.iconFailed' : 'disabledForeground'));
                return t;
            }
            case 'file': {
                const { file, status, messages } = item.entry;
                const t = fileItem(file, status, status === 'error' ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None);
                const dir = path.dirname(vscode.workspace.asRelativePath(file));
                t.description = status === 'error' ? `${messages.length} violation(s)` : status === 'ignored' ? `ignoré · ${dir}` : dir;
                if (status !== 'error') {
                    t.command = { command: 'ocara.architecture.goto', title: 'Ouvrir', arguments: [file, 1, 1] };
                }
                return t;
            }
            case 'message': {
                const m = item.message;
                const t = new vscode.TreeItem(m.message);
                t.description = `${path.basename(m.file)}:${m.line + 1}`;
                t.tooltip = `${m.file}:${m.line + 1}:${m.col + 1}\n${m.message}`;
                t.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('testing.iconFailed'));
                t.command = { command: 'ocara.architecture.goto', title: 'Aller à la violation', arguments: [m.file, m.line + 1, m.col + 1] };
                return t;
            }
        }
    }

    dispose(): void {
        this.disposables.forEach(d => d.dispose());
    }
}

// ── Statistiques ─────────────────────────────────────────────────────────────

interface Stats { checked: number; ok: number; errors: number }

function groupStats(files: ArchitectureFile[]): Stats {
    const checked = files.filter(f => f.status !== 'ignored');
    return { checked: checked.length, ok: checked.filter(f => f.status === 'ok').length, errors: checked.filter(f => f.status === 'error').length };
}

function percentOf(stats: Stats): number {
    return stats.checked === 0 ? 100 : stats.ok / stats.checked * 100;
}

function percentText(stats: Stats): string {
    return `${(Math.floor(percentOf(stats) * 10) / 10).toFixed(1)} % (${stats.ok}/${stats.checked})`;
}

function contexts(p: ArchitectureProject): string[] {
    return [...new Set(p.files.filter(f => f.layer).map(f => f.context!))].sort();
}

function isShared(p: ArchitectureProject, f: ArchitectureFile): boolean {
    return p.shared.some(s => isWithin(f.file, path.join(p.root, s.replace(/\./g, path.sep))));
}

function sortFiles(files: ArchitectureFile[]): ArchitectureFile[] {
    return [...files].sort((a, b) => statusOrder(a.status) - statusOrder(b.status) || a.file.localeCompare(b.file));
}

// ── Analyse ──────────────────────────────────────────────────────────────────

/** Racine du projet d'un fichier : dossier le plus proche contenant un `main.oc`. */
function projectRoot(file: string): string | undefined {
    const top = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file))?.uri.fsPath;
    for (let dir = path.dirname(file); ; dir = path.dirname(dir)) {
        if (fs.existsSync(path.join(dir, 'main.oc'))) { return dir; }
        if (dir === top || dir === path.dirname(dir)) { return undefined; }
    }
}

/** Projets de l'espace de travail : `ocara.architecture.projects`, sinon dossier de chaque `main.oc`. */
async function workspaceProjects(): Promise<string[]> {
    const explicit = vscode.workspace.getConfiguration('ocara.architecture').get<string[]>('projects', []).filter(p => p.trim().length > 0);
    if (explicit.length > 0) {
        const base = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '';
        return explicit.map(p => path.resolve(base, p));
    }
    const patterns = sectionExclusions('architecture');
    return (await vscode.workspace.findFiles('**/main.oc', IGNORED))
        .map(uri => path.dirname(uri.fsPath))
        .filter(dir => !isExcludedFrom(path.join(dir, 'main.oc'), [], patterns))
        .sort();
}

/** Projets à analyser et filtre des scripts listés selon le périmètre. */
async function projectsFor(target: Target): Promise<{ roots: string[]; keep: (file: string) => boolean }> {
    const all = await workspaceProjects();
    if (target.scope === 'workspace') { return { roots: all, keep: () => true }; }
    const place = target.folder ?? target.file!;
    const owner = projectRoot(target.file ?? path.join(target.folder!, '_'));
    const inside = target.folder ? all.filter(root => isWithin(root, target.folder!)) : [];
    const roots = [...new Set([...(owner ? [owner] : []), ...inside])];
    return { roots, keep: target.file ? f => f === target.file : f => isWithin(f, place) };
}

/** Script ignoré : test, bloc runtime, ou exclu par `ocara.architecture.exclude`. */
function ignored(file: string, root: string): boolean {
    const rel = path.relative(root, file);
    return file.endsWith('.runtime.oc') || file.endsWith('Test.oc') || rel.split(path.sep).includes('tests')
        || isExcludedFrom(file, [root], sectionExclusions('architecture'));
}

function toMessage(e: { file: string; line: number; col: number; message: string }): ToolMessage {
    return { file: e.file, line: Math.max(0, e.line - 1), col: Math.max(0, e.col - 1), severity: 'error', message: e.message };
}

async function analyze(target: Target): Promise<ArchitectureSummary> {
    const { roots, keep } = await projectsFor(target);
    const projects: ArchitectureProject[] = [];
    for (const root of roots) {
        const entry = path.join(root, 'main.oc');
        const result = await runTool('compilerPath', 'ocara', [entry, '--architecture-report', '--src', root], root, vscode.Uri.file(entry));
        if (result.launchError) { return { target, projects: [], checked: 0, count: 0, percent: 0, launchError: result.launchError }; }
        projects.push(toProject(root, result.stdout, result.stderr, keep));
    }
    const hexagonal = projects.filter(p => p.mode === 'hexagonal');
    const stats = groupStats(hexagonal.flatMap(p => p.files));
    const failed = projects.filter(p => p.mode === 'failed').length;
    const configErrors = hexagonal.reduce((n, p) => n + p.configErrors.length, 0);
    const violations = hexagonal.flatMap(p => p.files).filter(f => f.status !== 'ignored').reduce((n, f) => n + f.messages.length, 0);
    const percent = failed > 0 || configErrors > 0 ? 0 : percentOf(stats);
    return { target, projects, checked: stats.checked, count: violations + configErrors + failed, percent };
}

function toProject(root: string, stdout: string, stderr: string, keep: (file: string) => boolean): ArchitectureProject {
    const base: ArchitectureProject = { root, mode: 'failed', layers: {}, shared: [], configErrors: [], files: [] };
    let report: Report;
    try {
        report = JSON.parse(stdout) as Report;
    } catch {
        return { ...base, error: (stderr || stdout).trim().split('\n').slice(0, 5).join('\n') || 'sortie illisible' };
    }
    if (report.mode === 'permissive') { return { ...base, mode: 'permissive' }; }
    const files: ArchitectureFile[] = (report.files ?? []).filter(f => keep(f.path)).map(f => {
        const skip = ignored(f.path, root);
        const messages = skip ? [] : f.errors.map(toMessage);
        const status: FileStatus = skip ? 'ignored' : messages.length > 0 ? 'error' : 'ok';
        return f.place.kind === 'layer'
            ? { file: f.path, status, context: f.place.context, layer: f.place.layer, messages }
            : { file: f.path, status, messages };
    });
    return {
        ...base,
        mode: 'hexagonal',
        src: report.src,
        layers: report.layers ?? {},
        shared: report.shared ?? [],
        configErrors: (report.configErrors ?? []).map(toMessage),
        files,
    };
}
