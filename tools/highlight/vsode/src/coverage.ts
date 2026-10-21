import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { ocaraOutput, resolveCommand, runTool } from './toolrunner';
import { activeScriptFolder, coverageConfig, exclusions, findProjects, gotoLine, Project } from './projects';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Coverage » (barre latérale Ocara) : tests de chaque projet de
// l'espace de travail lancés par `ocaraunit --line-coverage --json`
// (binaires instrumentés par `ocara --coverage`), couverture par ligne
// fusionnée entre projets. L'encart affiche la couverture globale et, par
// fichier, les plages non couvertes — un clic place le curseur au début de
// la plage ; ces lignes sont surlignées dans l'éditeur ; les fichiers
// entièrement couverts sont listés en vert. Périmètre (`ocara.coverage.scope`) :
//   - `workspace` : tout l'espace de travail ; lancement par le bouton, la
//     commande « Lancer les tests et la couverture », et automatiquement à
//     l'ouverture et à l'enregistrement d'un `.oc` (`ocara.coverage.auto`) ;
//   - `folder` : le dossier du script affiché (sous-dossiers compris) —
//     lancement manuel uniquement. Les tests du projet qui contient le
//     dossier sont lancés, la couverture porte sur le dossier ; un dossier
//     de tests ne lance que ses tests ; un dossier parent de projets les
//     couvre entièrement.
// La LED « Release » (quality.ts) ne reçoit que les résultats `workspace`.
// ─────────────────────────────────────────────────────────────────────────────

/** Délai de regroupement des enregistrements rapprochés (ms). */
const DEBOUNCE_MS = 1500;

interface JsonFile { path: string; points: [number, number][] }
interface JsonReport {
    tests: { pass: number; fail: number; errors: number };
    coverage: { files: JsonFile[]; unanalyzed: { path: string; error: string }[] };
}

export type Scope = 'workspace' | 'folder';

export function coverageScope(): Scope {
    return coverageConfig().get<string>('scope', 'workspace') === 'folder' ? 'folder' : 'workspace';
}

/** Un lancement d'ocaraunit : tests à exécuter, périmètre de couverture. */
interface Run {
    project: Project;
    tests: string;
    /** Dossier couvert (`--line-coverage=<dossier>`) ; absent : tout le projet. */
    target?: string;
}

/** Schéma des éléments de fichier de l'encart : couleur verte des fichiers couverts. */
const FILE_SCHEME = 'ocara-coverage';

export interface CoverageSummary {
    scope: Scope;
    /** Dossier couvert (`folder`). */
    folder?: string;
    pass: number;
    fail: number;
    errors: number;
    covered: number;
    total: number;
    percent: number;
    files: FileCoverage[];
    unanalyzed: { path: string; error: string }[];
    /** Projets dont ocaraunit n'a pas produit de rapport. */
    failedProjects: { root: string; error: string }[];
    projects: number;
}

export interface FileCoverage {
    path: string;
    covered: number;
    total: number;
    /** Plages non couvertes (lignes 1-basées, incluses). */
    uncovered: { start: number; end: number }[];
}

type Item =
    | { type: 'summary' }
    | { type: 'running' }
    | { type: 'tests' }
    | { type: 'file'; file: FileCoverage }
    | { type: 'range'; file: string; start: number; end: number }
    | { type: 'unanalyzed'; path: string; error: string }
    | { type: 'failed'; root: string; error: string };

export class CoverageProvider implements vscode.TreeDataProvider<Item>, vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<Item | undefined>();
    readonly onDidChangeTreeData = this.changed.event;
    private readonly result = new vscode.EventEmitter<CoverageSummary | undefined>();
    /** Nouveau résultat (ou résultat effacé) — voyant de couverture. */
    readonly onResult = this.result.event;
    private readonly highlight = vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        backgroundColor: 'rgba(255, 70, 70, 0.13)',
        overviewRulerColor: 'rgba(255, 70, 70, 0.8)',
        overviewRulerLane: vscode.OverviewRulerLane.Left,
    });
    private summary: CoverageSummary | undefined;
    /** Dernier résultat sur tout l'espace de travail (voyant de la LED). */
    private workspaceSummary: CoverageSummary | undefined;
    private running = false;
    private pending = false;
    private timer: NodeJS.Timeout | undefined;
    private readonly disposables: vscode.Disposable[] = [];

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            this.highlight,
            vscode.window.createTreeView('ocara.coverage', { treeDataProvider: this }),
            vscode.window.registerFileDecorationProvider({
                provideFileDecoration: uri => uri.scheme === FILE_SCHEME && uri.query === 'covered'
                    ? { color: new vscode.ThemeColor('testing.iconPassed'), tooltip: 'Entièrement couvert' }
                    : undefined,
            }),
            vscode.commands.registerCommand('ocara.coverage.run', () => this.run()),
            vscode.commands.registerCommand('ocara.coverage.goto', (file: string, line: number) => gotoLine(file, line)),
            vscode.commands.registerCommand('ocara.coverage.output', () => ocaraOutput().show(true)),
            vscode.workspace.onDidSaveTextDocument(doc => { if (doc.languageId === 'ocara') { this.schedule(); } }),
            vscode.window.onDidChangeVisibleTextEditors(editors => editors.forEach(e => this.decorate(e))),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.coverage.scope')) { this.reset(); }
                if (e.affectsConfiguration('ocara.coverage.exclude') || e.affectsConfiguration('ocara.coverage.projects')) { this.schedule(); }
            }),
        );
        context.subscriptions.push(this);
        this.schedule();
    }

    current(): CoverageSummary | undefined {
        return this.workspaceSummary;
    }

    /** Périmètre changé : l'ancien résultat ne s'applique plus. */
    private reset(): void {
        this.summary = undefined;
        this.changed.fire(undefined);
        vscode.window.visibleTextEditors.forEach(e => this.decorate(e));
        this.schedule();
    }

    /** Lancement automatique (réglage `ocara.coverage.auto`, mode `workspace`), regroupé. */
    private schedule(): void {
        if (coverageScope() === 'folder' || !coverageConfig().get<boolean>('auto', true)) { return; }
        clearTimeout(this.timer);
        this.timer = setTimeout(() => void this.run(), DEBOUNCE_MS);
    }

    /** Une seule analyse à la fois ; une demande pendant l'analyse la relance à la fin. */
    async run(): Promise<void> {
        if (this.running) { this.pending = true; return; }
        const scope = coverageScope();
        const folder = scope === 'folder' ? activeScriptFolder() : undefined;
        if (scope === 'folder' && !folder) {
            vscode.window.showWarningMessage('Ocara : ouvrez un script .oc — la couverture porte sur son dossier (réglage « ocara.coverage.scope »).');
            return;
        }
        this.running = true;
        this.changed.fire(undefined);
        try {
            await vscode.workspace.saveAll(false);
            const projects = await findProjects();
            const runs = folder ? folderRuns(projects, folder) : projects.map(project => ({ project, tests: project.testsDir }));
            this.summary = { ...(await analyze(runs)), scope, folder };
            if (scope === 'workspace') {
                this.workspaceSummary = this.summary;
                this.result.fire(this.summary);
            }
        } catch (err) {
            vscode.window.showErrorMessage(`Ocara : couverture — ${err instanceof Error ? err.message : err}`);
        } finally {
            this.running = false;
            this.changed.fire(undefined);
            vscode.window.visibleTextEditors.forEach(e => this.decorate(e));
            if (this.pending) { this.pending = false; void this.run(); }
        }
    }

    private decorate(editor: vscode.TextEditor): void {
        const file = this.summary?.files.find(f => samePath(f.path, editor.document.uri.fsPath));
        const last = editor.document.lineCount - 1;
        const ranges = (file?.uncovered ?? []).map(r =>
            new vscode.Range(Math.min(r.start - 1, last), 0, Math.min(r.end - 1, last), 0));
        editor.setDecorations(this.highlight, ranges);
    }

    // ── Arbre ────────────────────────────────────────────────────────────────

    getChildren(item?: Item): Item[] {
        if (item?.type === 'file') {
            return item.file.uncovered.map(r => ({ type: 'range', file: item.file.path, start: r.start, end: r.end }));
        }
        if (item) { return []; }
        const s = this.summary;
        const items: Item[] = [];
        if (this.running) { items.push({ type: 'running' }); }
        if (!s) { return items; }
        items.push({ type: 'summary' });
        if (s.fail > 0 || s.errors > 0) { items.push({ type: 'tests' }); }
        s.failedProjects.forEach(p => items.push({ type: 'failed', ...p }));
        // Non couverts d'abord, puis les fichiers entièrement couverts (en vert).
        const files = [...s.files.filter(f => f.uncovered.length > 0), ...s.files.filter(f => f.uncovered.length === 0)];
        files.forEach(file => items.push({ type: 'file', file }));
        s.unanalyzed.forEach(u => items.push({ type: 'unanalyzed', ...u }));
        return items;
    }

    getTreeItem(item: Item): vscode.TreeItem {
        const s = this.summary;
        switch (item.type) {
            case 'running': {
                const t = new vscode.TreeItem('Tests et couverture en cours…');
                t.iconPath = new vscode.ThemeIcon('loading~spin');
                return t;
            }
            case 'summary': {
                const where = s!.scope === 'folder' && s!.folder ? ` de ${vscode.workspace.asRelativePath(s!.folder)}` : '';
                const t = new vscode.TreeItem(`Couverture${where} : ${formatPercent(s!.percent)}`);
                t.description = `${s!.covered}/${s!.total} lignes · ${s!.pass} PASS`;
                t.tooltip = `${s!.projects} projet(s) testé(s) — ${s!.pass} PASS, ${s!.fail} FAIL, ${s!.errors} erreur(s)`;
                t.iconPath = new vscode.ThemeIcon(s!.percent >= 100 ? 'pass' : 'graph', new vscode.ThemeColor(s!.percent >= 100 ? 'testing.iconPassed' : 'charts.yellow'));
                return t;
            }
            case 'tests': {
                const t = new vscode.TreeItem(`${s!.fail} FAIL, ${s!.errors} erreur(s)`);
                t.tooltip = 'Détail dans la sortie « Ocara »';
                t.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('testing.iconFailed'));
                t.command = { command: 'ocara.coverage.output', title: 'Afficher la sortie' };
                return t;
            }
            case 'failed': {
                const t = new vscode.TreeItem(path.basename(item.root), vscode.TreeItemCollapsibleState.None);
                t.description = 'ocaraunit en échec';
                t.tooltip = item.error;
                t.iconPath = new vscode.ThemeIcon('error', new vscode.ThemeColor('testing.iconFailed'));
                t.command = { command: 'ocara.coverage.output', title: 'Afficher la sortie' };
                return t;
            }
            case 'file': {
                const covered = item.file.uncovered.length === 0;
                const t = new vscode.TreeItem(path.basename(item.file.path),
                    covered ? vscode.TreeItemCollapsibleState.None : vscode.TreeItemCollapsibleState.Collapsed);
                const pct = item.file.total ? item.file.covered / item.file.total * 100 : 100;
                t.description = `${formatPercent(pct)} · ${relative(item.file.path)}`;
                t.tooltip = `${item.file.path}\n${item.file.covered}/${item.file.total} lignes couvertes`;
                t.resourceUri = vscode.Uri.file(item.file.path).with({ scheme: FILE_SCHEME, query: covered ? 'covered' : 'partial' });
                t.iconPath = covered ? new vscode.ThemeIcon('pass-filled', new vscode.ThemeColor('testing.iconPassed')) : vscode.ThemeIcon.File;
                if (covered) {
                    t.command = { command: 'ocara.coverage.goto', title: 'Ouvrir', arguments: [item.file.path, 1] };
                }
                return t;
            }
            case 'range': {
                const t = new vscode.TreeItem(item.start === item.end ? `Ligne ${item.start}` : `Lignes ${item.start}–${item.end}`);
                t.description = 'non couverte';
                t.iconPath = new vscode.ThemeIcon('circle-slash', new vscode.ThemeColor('testing.iconFailed'));
                t.command = { command: 'ocara.coverage.goto', title: 'Aller au code non couvert', arguments: [item.file, item.start] };
                return t;
            }
            case 'unanalyzed': {
                const t = new vscode.TreeItem(path.basename(item.path));
                t.description = 'non analysé';
                t.tooltip = item.error;
                t.iconPath = new vscode.ThemeIcon('warning', new vscode.ThemeColor('charts.yellow'));
                t.command = { command: 'ocara.coverage.goto', title: 'Ouvrir', arguments: [item.path, 1] };
                return t;
            }
        }
    }

    dispose(): void {
        clearTimeout(this.timer);
        this.disposables.forEach(d => d.dispose());
    }
}

// ── Exécution d'ocaraunit et fusion ──────────────────────────────────────────

function isWithin(child: string, parent: string): boolean {
    const rel = path.relative(parent, child);
    return rel === '' || (!rel.startsWith('..') && !path.isAbsolute(rel));
}

/** Lancements du mode `folder` : projet qui contient le dossier (le plus
 *  profond) couvert sur ce dossier, sinon projets contenus dans le dossier. */
export function folderRuns(projects: Project[], folder: string): Run[] {
    const owner = projects.filter(p => isWithin(folder, p.root)).sort((a, b) => b.root.length - a.root.length)[0];
    if (owner) {
        return isWithin(folder, owner.testsDir)
            ? [{ project: owner, tests: folder }]
            : [{ project: owner, tests: owner.testsDir, target: folder }];
    }
    return projects.filter(p => isWithin(p.root, folder)).map(project => ({ project, tests: project.testsDir }));
}

async function analyze(runs: Run[]): Promise<Omit<CoverageSummary, 'scope' | 'folder'>> {
    const merged = new Map<string, Map<number, boolean>>();
    const summary: Omit<CoverageSummary, 'scope' | 'folder'> = { pass: 0, fail: 0, errors: 0, covered: 0, total: 0, percent: 100, files: [], unanalyzed: [], failedProjects: [], projects: runs.length };
    const excludeArgs = exclusions().flatMap(e => ['--exclude', e]);
    const compiler = resolveCommand('compilerPath', 'ocara');
    const env = compiler.length === 1 ? { OCARA: compiler[0] } : undefined;
    const out = ocaraOutput();
    for (const { project, tests, target } of runs) {
        const json = path.join(os.tmpdir(), `ocaraunit-${process.pid}-${Date.now()}.json`);
        const coverage = target ? `--line-coverage=${target}` : '--line-coverage';
        const args = [coverage, '--json', json, ...excludeArgs, tests];
        const result = await runTool('ocaraunitPath', 'ocaraunit', args, project.root, undefined, env);
        const report = readReport(json);
        if (result.launchError || !report) {
            const error = result.launchError ?? (result.stderr || result.stdout).trim().split('\n').slice(0, 5).join('\n');
            summary.failedProjects.push({ root: project.root, error: error || 'aucun rapport produit' });
            out.appendLine(`── ocaraunit ${tests} : échec\n${result.stdout}${result.stderr}`);
            continue;
        }
        if (report.tests.fail > 0 || report.tests.errors > 0) {
            out.appendLine(`── ocaraunit ${tests}\n${stripAnsi(result.stdout)}`);
        }
        summary.pass += report.tests.pass;
        summary.fail += report.tests.fail;
        summary.errors += report.tests.errors;
        for (const file of report.coverage.files) {
            const lines = merged.get(file.path) ?? new Map<number, boolean>();
            for (const [line, hit] of file.points) { lines.set(line, (lines.get(line) ?? false) || hit === 1); }
            merged.set(file.path, lines);
        }
        summary.unanalyzed.push(...report.coverage.unanalyzed);
    }
    summary.unanalyzed = summary.unanalyzed.filter((u, i, all) => !merged.has(u.path) && all.findIndex(o => o.path === u.path) === i);
    summary.files = [...merged.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([file, lines]) => toFileCoverage(file, lines));
    summary.covered = summary.files.reduce((n, f) => n + f.covered, 0);
    summary.total = summary.files.reduce((n, f) => n + f.total, 0);
    summary.percent = summary.total ? summary.covered / summary.total * 100 : 100;
    return summary;
}

function readReport(file: string): JsonReport | undefined {
    try {
        return JSON.parse(fs.readFileSync(file, 'utf8')) as JsonReport;
    } catch {
        return undefined;
    } finally {
        fs.rm(file, { force: true }, () => undefined);
    }
}

export function toFileCoverage(file: string, lines: Map<number, boolean>): FileCoverage {
    const sorted = [...lines.entries()].sort(([a], [b]) => a - b);
    const uncovered: { start: number; end: number }[] = [];
    let open: { start: number; end: number } | undefined;
    for (const [line, hit] of sorted) {
        if (hit) {
            if (open) { uncovered.push(open); open = undefined; }
        } else if (open) {
            open.end = line;
        } else {
            open = { start: line, end: line };
        }
    }
    if (open) { uncovered.push(open); }
    return { path: file, covered: sorted.filter(([, hit]) => hit).length, total: sorted.length, uncovered };
}

function samePath(a: string, b: string): boolean {
    return path.resolve(a) === path.resolve(b);
}

function relative(file: string): string {
    return path.dirname(vscode.workspace.asRelativePath(file));
}

export function formatPercent(pct: number): string {
    return `${(Math.floor(pct * 10) / 10).toFixed(1)} %`;
}

function stripAnsi(text: string): string {
    return text.replace(/\x1b\[[0-9;]*m/g, '');
}
