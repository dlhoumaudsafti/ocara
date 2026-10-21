import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { ocaraOutput, resolveCommand, runTool } from './toolrunner';
import { exclusions, findProjects, gotoLine, isExcludedFrom, ocFiles, Project } from './projects';
import { fileItem } from './filestatus';
import { AutoRunner, currentTarget, describeTarget, isWithin, Target } from './scope';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Coverage » (barre latérale Ocara) : tests de chaque projet de
// l'espace de travail lancés par `ocaraunit --line-coverage --json`
// (binaires instrumentés par `ocara --coverage`), couverture par ligne
// fusionnée entre projets. L'encart affiche la couverture globale et, par
// fichier, les plages non couvertes — un clic place le curseur au début de
// la plage ; ces lignes sont surlignées dans l'éditeur ; les fichiers
// entièrement couverts sont listés en vert, les autres en rouge, les fichiers
// exclus (`ocara.coverage.exclude`) en gris. Périmètre (`ocara.coverage.scope`,
// voir scope.ts ; mode automatique `ocara.coverage.auto` et bouton ▶) :
//   - `workspace` : tous les projets testés de l'espace de travail ;
//   - `folder` : dossier du script affiché — tests du projet qui le contient,
//     couverture limitée au dossier ; un dossier de tests ne lance que ses
//     tests ; un dossier parent de projets les couvre entièrement ;
//   - `script` : script affiché — tests de son projet, couverture limitée au
//     script ; un `*Test.oc` ne lance que lui-même.
// Le voyant Coverage de la LED « Release » (quality.ts) reprend le résultat
// du périmètre courant.
// ─────────────────────────────────────────────────────────────────────────────

/** Délai de regroupement des enregistrements rapprochés (ms). */
const DEBOUNCE_MS = 1500;

interface JsonFile { path: string; points: [number, number][] }
interface JsonReport {
    tests: { pass: number; fail: number; errors: number };
    coverage: { files: JsonFile[]; unanalyzed: { path: string; error: string }[] };
}

/** Un lancement d'ocaraunit : tests à exécuter, périmètre de couverture. */
interface Run {
    project: Project;
    tests: string;
    /** Dossier ou script couvert (`--line-coverage=<chemin>`) ; absent : tout le projet. */
    target?: string;
}

export interface CoverageSummary {
    target: Target;
    pass: number;
    fail: number;
    errors: number;
    covered: number;
    total: number;
    percent: number;
    files: FileCoverage[];
    unanalyzed: { path: string; error: string }[];
    /** Scripts du périmètre exclus par `ocara.coverage.exclude` (listés en gris). */
    ignored: string[];
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
    | { type: 'ignored'; path: string }
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
    private running = false;
    private pending = false;
    private readonly auto = new AutoRunner('coverage', 'coverage', () => void this.run(), DEBOUNCE_MS);
    private readonly disposables: vscode.Disposable[] = [];

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            this.highlight,
            vscode.window.createTreeView('ocara.coverage', { treeDataProvider: this }),
            vscode.commands.registerCommand('ocara.coverage.run', () => this.run()),
            vscode.commands.registerCommand('ocara.coverage.goto', (file: string, line: number) => gotoLine(file, line)),
            vscode.commands.registerCommand('ocara.coverage.output', () => ocaraOutput().show(true)),
            vscode.window.onDidChangeVisibleTextEditors(editors => editors.forEach(e => this.decorate(e))),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.coverage.scope')) { this.reset(); }
                if (e.affectsConfiguration('ocara.coverage.exclude') || e.affectsConfiguration('ocara.coverage.projects')) { this.auto.schedule(true); }
            }),
        );
        context.subscriptions.push(this);
        this.auto.register(context);
    }

    current(): CoverageSummary | undefined {
        return this.summary;
    }

    /** Périmètre changé : l'ancien résultat ne s'applique plus. */
    private reset(): void {
        this.summary = undefined;
        this.result.fire(undefined);
        this.changed.fire(undefined);
        vscode.window.visibleTextEditors.forEach(e => this.decorate(e));
    }

    /** Une seule analyse à la fois ; une demande pendant l'analyse la relance à la fin. */
    async run(): Promise<void> {
        if (this.running) { this.pending = true; return; }
        const target = currentTarget('coverage');
        if (!target) {
            vscode.window.showWarningMessage('Ocara : ouvrez un script .oc — la couverture porte sur le script affiché ou son dossier (réglage « ocara.coverage.scope »).');
            return;
        }
        this.running = true;
        this.changed.fire(undefined);
        try {
            await vscode.workspace.saveAll(false);
            const projects = await findProjects();
            const runs = target.folder ? folderRuns(projects, target.folder)
                : target.file ? scriptRuns(projects, target.file)
                : projects.map(project => ({ project, tests: project.testsDir }));
            const roots = target.folder ?? target.file ?? undefined;
            this.summary = { ...(await analyze(runs)), target, ignored: await ignoredFiles(roots ? [roots] : projects.map(p => p.root)) };
            this.result.fire(this.summary);
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
        s.ignored.forEach(p => items.push({ type: 'ignored', path: p }));
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
                const where = s!.target.scope === 'workspace' ? '' : ` (${describeTarget(s!.target)})`;
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
                const t = fileItem(item.file.path, covered ? 'ok' : 'error',
                    covered ? vscode.TreeItemCollapsibleState.None : vscode.TreeItemCollapsibleState.Collapsed);
                const pct = item.file.total ? item.file.covered / item.file.total * 100 : 100;
                t.description = `${formatPercent(pct)} · ${relative(item.file.path)}`;
                t.tooltip = `${item.file.path}\n${item.file.covered}/${item.file.total} lignes couvertes`;
                if (covered) {
                    t.command = { command: 'ocara.coverage.goto', title: 'Ouvrir', arguments: [item.file.path, 1] };
                }
                return t;
            }
            case 'ignored': {
                const t = fileItem(item.path, 'ignored', vscode.TreeItemCollapsibleState.None);
                t.description = `exclu · ${relative(item.path)}`;
                t.command = { command: 'ocara.coverage.goto', title: 'Ouvrir', arguments: [item.path, 1] };
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
        this.disposables.forEach(d => d.dispose());
    }
}

// ── Exécution d'ocaraunit et fusion ──────────────────────────────────────────

/** Lancements du mode `folder` : projet qui contient le dossier (le plus
 *  profond) couvert sur ce dossier, sinon projets contenus dans le dossier. */
export function folderRuns(projects: Project[], folder: string): Run[] {
    const owner = ownerOf(projects, folder);
    if (owner) {
        return isWithin(folder, owner.testsDir)
            ? [{ project: owner, tests: folder }]
            : [{ project: owner, tests: owner.testsDir, target: folder }];
    }
    return projects.filter(p => isWithin(p.root, folder)).map(project => ({ project, tests: project.testsDir }));
}

/** Projet testé qui contient `p` (le plus profond). */
function ownerOf(projects: Project[], p: string): Project | undefined {
    return projects.filter(project => isWithin(p, project.root)).sort((a, b) => b.root.length - a.root.length)[0];
}

/** Lancements du mode `script` : un `*Test.oc` seul, sinon les tests du projet couverts sur le script. */
export function scriptRuns(projects: Project[], file: string): Run[] {
    const owner = ownerOf(projects, file);
    if (!owner) { return []; }
    return isWithin(file, owner.testsDir)
        ? [{ project: owner, tests: file }]
        : [{ project: owner, tests: owner.testsDir, target: file }];
}

/** Scripts des `roots` exclus par `ocara.coverage.exclude` (hors dossiers de tests). */
async function ignoredFiles(roots: string[]): Promise<string[]> {
    const patterns = exclusions();
    if (patterns.length === 0) { return []; }
    const files = new Set<string>();
    for (const root of roots) {
        const base = root.endsWith('.oc') ? path.dirname(root) : root;
        for (const file of await ocFiles(root)) {
            const inTests = path.relative(base, file).split(path.sep).includes('tests');
            if (!inTests && isExcludedFrom(file, [base], patterns)) { files.add(file); }
        }
    }
    return [...files].sort();
}

async function analyze(runs: Run[]): Promise<Omit<CoverageSummary, 'target' | 'ignored'>> {
    const merged = new Map<string, Map<number, boolean>>();
    const summary: Omit<CoverageSummary, 'target' | 'ignored'> = { pass: 0, fail: 0, errors: 0, covered: 0, total: 0, percent: 100, files: [], unanalyzed: [], failedProjects: [], projects: runs.length };
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
