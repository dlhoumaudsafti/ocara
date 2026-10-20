import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { spawn } from 'child_process';
import { ocaraOutput, resolveCommand, runTool } from './toolrunner';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Benchmark » (barre latérale Ocara) pour le script affiché :
// temps et pic mémoire sur N exécutions (build Release), fuites (bilan des
// allocations `ocara --leak-check`, ou pic mémoire à deux valeurs du
// marqueur `__N__`), profil par fonction (`ocara --profile`), historique
// comparé. Méthode : docs/benchmarking.md.
// ─────────────────────────────────────────────────────────────────────────────

/** `leak` : nombre de valeurs non libérées ; `growth` : pic mémoire (Ko) de la
 *  méthode par croissance — deux unités, donc deux genres comparés à part. */
type Kind = 'time' | 'leak' | 'growth' | 'profile';

interface Entry {
    date: string;
    kind: Kind;
    /** Mesure comparée d'une exécution à l'autre (ms, nombre, Ko ou ms propre). */
    value: number;
    summary: string;
}

/** Fin d'une exécution mesurée. */
interface RunResult { ms: number; peakKb: number; code: number | null; signal: string | null }

/** Derniers résultats détaillés d'un script (profil, bilan de fuites). */
interface Details { profile: ProfileRow[]; leaks?: LeakRow[] }

interface ProfileRow { name: string; calls: number; totalMs: number; ownMs: number }

/** Bilan `--leak-check` : valeurs restantes d'un genre (classe, string…). */
interface LeakRow { label: string; live: number; allocated: number; freed: number; sites: LeakSite[] }

/** Fonction où des valeurs restantes ont été allouées. */
interface LeakSite { count: number; func: string; file: string; line: number }

type Item =
    | { type: 'action'; label: string; command: string; icon: string }
    | { type: 'group'; id: 'profile' | 'leaks' | 'history'; label: string }
    | { type: 'row'; label: string; description: string; icon: string; tooltip?: string; children?: Item[]; open?: { file: string; line: number } };

const TIME_BIN = '/usr/bin/time';

export class BenchmarkProvider implements vscode.TreeDataProvider<Item> {
    private readonly changed = new vscode.EventEmitter<void>();
    readonly onDidChangeTreeData = this.changed.event;
    private file: string | undefined;
    /** Résultats par script mesuré : un changement d'éditeur pendant une
     *  mesure ne les attache pas au script nouvellement affiché. */
    private readonly details = new Map<string, Details>();
    private running = false;

    constructor(private readonly state: vscode.Memento) {}

    register(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
            vscode.window.createTreeView('ocara.benchmark', { treeDataProvider: this }),
            vscode.commands.registerCommand('ocara.bench.run', () => this.guard(f => this.measure(f))),
            vscode.commands.registerCommand('ocara.bench.leak', () => this.guard(f => this.leakCheck(f))),
            vscode.commands.registerCommand('ocara.bench.leakGrowth', () => this.guard(f => this.leak(f))),
            vscode.commands.registerCommand('ocara.bench.profile', () => this.guard(f => this.runProfile(f))),
            vscode.commands.registerCommand('ocara.bench.clear', () => this.clear()),
            vscode.window.onDidChangeActiveTextEditor(() => this.follow()),
        );
        this.follow();
    }

    /** Le script suivi est le dernier script Ocara affiché. */
    private follow(): void {
        const doc = vscode.window.activeTextEditor?.document;
        if (doc?.languageId === 'ocara' && doc.uri.scheme === 'file' && doc.uri.fsPath !== this.file) {
            this.file = doc.uri.fsPath;
            this.changed.fire();
        }
    }

    /** Résultats détaillés de `file` (créés au besoin). */
    private detailsOf(file: string): Details {
        let d = this.details.get(file);
        if (!d) { d = { profile: [] }; this.details.set(file, d); }
        return d;
    }

    private async guard(run: (file: string) => Promise<void>): Promise<void> {
        // Script figé au lancement : le résultat lui reste attaché même si
        // l'éditeur actif change pendant la mesure.
        const file = this.file;
        if (!file) { vscode.window.showWarningMessage('Ocara : ouvrez un script .oc à mesurer.'); return; }
        if (this.running) { vscode.window.showInformationMessage('Ocara : une mesure est déjà en cours.'); return; }
        const doc = vscode.workspace.textDocuments.find(d => d.uri.fsPath === file);
        if (doc?.isDirty) { await doc.save(); }
        this.running = true;
        try {
            await vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title: `Ocara : benchmark de ${path.basename(file)}…` }, () => run(file));
        } catch (err) {
            vscode.window.showErrorMessage(`Ocara : ${err instanceof Error ? err.message : err}`);
        } finally {
            this.running = false;
        }
    }

    // ── Mesures ──────────────────────────────────────────────────────────────

    private async measure(file: string): Promise<void> {
        const runs = Math.max(1, config().get<number>('runs', 5));
        await withTempDir(async dir => {
            const binary = await build(file, path.join(dir, 'bench'), ['--release']);
            const times: number[] = [];
            let peak = 0;
            for (let i = 0; i < runs; i++) {
                const r = requireSuccess(await execute(binary, path.dirname(file), dir));
                times.push(r.ms);
                peak = Math.max(peak, r.peakKb);
            }
            const mean = times.reduce((a, b) => a + b, 0) / times.length;
            const memory = peak ? ` · pic ${formatKb(peak)}` : '';
            this.add(file, { kind: 'time', value: mean, summary: `${formatMs(mean)} (min ${formatMs(Math.min(...times))}, max ${formatMs(Math.max(...times))}, ${runs} exécutions)${memory}` });
        });
    }

    /** Bilan des allocations (`ocara --leak-check`) : le script s'exécute
     *  tel quel, ce qui reste alloué à la sortie est listé par genre. */
    private async leakCheck(file: string): Promise<void> {
        const { rows, run } = await withTempDir(async dir => {
            const binary = await build(file, path.join(dir, 'leaks'), ['--release', '--leak-check']);
            const out = path.join(dir, 'leaks.tsv');
            const run = await execute(binary, path.dirname(file), dir, { OCARA_LEAK_OUT: out });
            return { run, rows: fs.existsSync(out) ? parseLeaks(fs.readFileSync(out, 'utf8')) : undefined };
        });
        if (!rows) { throw new Error(`bilan absent : le programme ne s'est pas terminé normalement${exitNote(run)}.`); }
        this.detailsOf(file).leaks = rows;
        const total = rows.reduce((a, r) => a + r.live, 0);
        // Code de sortie non nul : le bilan reste valable, la fin anormale est signalée.
        const summary = (total === 0
            ? 'aucune fuite — toute valeur allouée a été libérée'
            : `⚠ ${total} valeur(s) non libérée(s) — ${rows.slice(0, 3).map(r => `${r.live} × ${r.label}${r.sites[0] ? ` (${r.sites[0].func})` : ''}`).join(', ')}`) + exitNote(run);
        const out = ocaraOutput();
        for (const r of rows) {
            out.appendLine(`[fuites] ${r.live} × ${r.label} (allouées ${r.allocated}, libérées ${r.freed})`);
            for (const s of r.sites) { out.appendLine(`           ${s.count} dans ${s.func}${s.file ? ` — ${s.file}:${s.line}` : ''}`); }
        }
        this.add(file, { kind: 'leak', value: total, summary });
        (total ? vscode.window.showWarningMessage : vscode.window.showInformationMessage)(`Ocara : ${summary}`);
    }

    /** Méthode par croissance : pic mémoire à deux valeurs du marqueur `__N__`. */
    private async leak(file: string): Promise<void> {
        const source = fs.readFileSync(file, 'utf8');
        if (!source.includes('__N__')) {
            throw new Error('détection de fuite : le script doit contenir le marqueur __N__ (nombre d\'itérations), voir docs/benchmarking.md.');
        }
        if (!fs.existsSync(TIME_BIN)) { throw new Error(`détection de fuite : ${TIME_BIN} est requis pour mesurer le pic mémoire.`); }
        const [small, large] = config().get<number[]>('leakSizes', [20000, 200000]);
        const peaks: number[] = [];
        await withTempDir(async dir => {
            for (const n of [small, large]) {
                // Copie dans le dossier temporaire (jamais dans le projet) ;
                // `--src` garde la résolution des imports depuis le dossier du script.
                const copy = path.join(dir, `${path.basename(file, '.oc')}.bench${n}.oc`);
                fs.writeFileSync(copy, source.split('__N__').join(String(n)));
                const binary = await build(copy, path.join(dir, `leak${n}`), ['--release', ...srcFlag(file)], file);
                peaks.push(requireSuccess(await execute(binary, path.dirname(file), dir)).peakKb);
            }
        });
        const growth = peaks[1] - peaks[0];
        const leaking = growth > Math.max(1024, peaks[0] * 0.1);
        const verdict = leaking ? `⚠ fuite probable (+${formatKb(growth)})` : 'stable : pas de fuite';
        this.add(file, { kind: 'growth', value: peaks[1], summary: `${verdict} — N=${small} : ${formatKb(peaks[0])}, N=${large} : ${formatKb(peaks[1])}` });
        (leaking ? vscode.window.showWarningMessage : vscode.window.showInformationMessage)(`Ocara : ${verdict}`);
    }

    private async runProfile(file: string): Promise<void> {
        const { profile, run } = await withTempDir(async dir => {
            const binary = await build(file, path.join(dir, 'profile'), ['--release', '--profile']);
            const out = path.join(dir, 'profile.tsv');
            const run = await execute(binary, path.dirname(file), dir, { OCARA_PROFILE_OUT: out });
            return { run, profile: fs.existsSync(out) ? parseProfile(fs.readFileSync(out, 'utf8')) : [] };
        });
        if (!profile.length) { throw new Error(`profil vide : le programme ne s'est pas terminé normalement${exitNote(run)}.`); }
        this.detailsOf(file).profile = profile;
        const top = profile[0];
        const own = profile.reduce((a, r) => a + r.ownMs, 0);
        this.add(file, { kind: 'profile', value: own, summary: `${profile.length} fonctions, ${formatMs(own)} mesurées — plus coûteuse : ${top.name} (${formatMs(top.ownMs)})${exitNote(run)}` });
    }

    // ── Historique ───────────────────────────────────────────────────────────

    private history(file: string): Entry[] {
        // Anciennes entrées « par croissance » enregistrées sous `leak` (Ko).
        return this.state.get<Entry[]>(`ocara.bench:${file}`, [])
            .map(e => (e.kind === 'leak' && / N=\d+ : /.test(e.summary) ? { ...e, kind: 'growth' as Kind } : e));
    }

    private add(file: string, entry: Omit<Entry, 'date'>): void {
        const list = [{ ...entry, date: new Date().toLocaleString() }, ...this.history(file)].slice(0, 30);
        void this.state.update(`ocara.bench:${file}`, list);
        ocaraOutput().appendLine(`[benchmark] ${path.basename(file)} — ${entry.summary}`);
        this.changed.fire();
    }

    private async clear(): Promise<void> {
        if (this.file) {
            await this.state.update(`ocara.bench:${this.file}`, undefined);
            this.details.delete(this.file);
        }
        this.changed.fire();
    }

    // ── Arbre ────────────────────────────────────────────────────────────────

    getChildren(item?: Item): Item[] {
        if (!this.file) { return []; }
        const details = this.detailsOf(this.file);
        if (!item) {
            const items: Item[] = [
                { type: 'action', label: 'Mesurer temps et mémoire', command: 'ocara.bench.run', icon: 'dashboard' },
                { type: 'action', label: 'Détecter les fuites', command: 'ocara.bench.leak', icon: 'debug-alt' },
                { type: 'action', label: 'Fuite par croissance (__N__)', command: 'ocara.bench.leakGrowth', icon: 'graph-line' },
                { type: 'action', label: 'Profiler par fonction', command: 'ocara.bench.profile', icon: 'flame' },
            ];
            if (details.leaks) { items.push({ type: 'group', id: 'leaks', label: details.leaks.length ? 'Fuites détectées' : 'Fuites : aucune' }); }
            if (details.profile.length) { items.push({ type: 'group', id: 'profile', label: 'Dernier profil' }); }
            items.push({ type: 'group', id: 'history', label: 'Historique' });
            return items;
        }
        if (item.type === 'group' && item.id === 'leaks') {
            return (details.leaks ?? []).map(r => ({
                type: 'row', label: `${r.live} × ${r.label}`, icon: 'warning',
                description: `allouées ${r.allocated}, libérées ${r.freed}`,
                tooltip: `${r.live} valeur(s) « ${r.label} » encore allouée(s) à la sortie du programme — dépliez pour voir où`,
                // Lieux d'allocation : un clic ouvre la fonction qui a alloué.
                children: r.sites.map(s => ({
                    type: 'row' as const, label: s.func, icon: 'symbol-method',
                    description: `${s.count} alloué(s)${s.file ? ` — ${path.basename(s.file)}:${s.line}` : ''}`,
                    tooltip: s.file ? `${s.count} « ${r.label} » alloué(s) dans ${s.func} — ${s.file}:${s.line}` : `${s.count} « ${r.label} » alloué(s) hors d'une fonction connue`,
                    open: s.file ? { file: s.file, line: s.line } : undefined,
                })),
            }));
        }
        if (item.type === 'group' && item.id === 'profile') {
            return details.profile.map(r => ({
                type: 'row', label: r.name, icon: 'symbol-method',
                description: `propre ${formatMs(r.ownMs)} · total ${formatMs(r.totalMs)} · ${r.calls} appel(s)`,
            }));
        }
        if (item.type === 'group') {
            const list = this.history(this.file);
            return list.map((e, i) => {
                const previous = list.slice(i + 1).find(p => p.kind === e.kind);
                return { type: 'row', label: e.date, description: `${delta(e, previous)}${e.summary}`, icon: ICONS[e.kind], tooltip: e.summary };
            });
        }
        if (item.type === 'row') {
            return item.children ?? [];
        }
        return [];
    }

    getTreeItem(item: Item): vscode.TreeItem {
        if (item.type === 'action') {
            const t = new vscode.TreeItem(item.label);
            t.iconPath = new vscode.ThemeIcon(item.icon);
            t.command = { command: item.command, title: item.label };
            t.description = this.file ? path.basename(this.file) : '';
            return t;
        }
        if (item.type === 'group') {
            const t = new vscode.TreeItem(item.label, vscode.TreeItemCollapsibleState.Expanded);
            t.iconPath = new vscode.ThemeIcon(item.id === 'profile' ? 'flame' : item.id === 'leaks' ? (this.file && this.details.get(this.file)?.leaks?.length ? 'warning' : 'pass') : 'history');
            return t;
        }
        const t = new vscode.TreeItem(item.label, item.children?.length ? vscode.TreeItemCollapsibleState.Expanded : vscode.TreeItemCollapsibleState.None);
        t.description = item.description;
        t.tooltip = item.tooltip ?? `${item.label} — ${item.description}`;
        t.iconPath = new vscode.ThemeIcon(item.icon);
        if (item.open) {
            t.command = { command: 'ocara.nav.open', title: 'Ouvrir', arguments: [item.open.file, item.open.line] };
        }
        return t;
    }
}

const ICONS: Record<Kind, string> = { time: 'dashboard', leak: 'debug-alt', growth: 'graph-line', profile: 'flame' };

/** `K genre restants alloués libérés`, puis ses `S genre nombre fonction fichier ligne`. */
function parseLeaks(text: string): LeakRow[] {
    const rows: LeakRow[] = [];
    for (const line of text.split('\n').filter(Boolean)) {
        const f = line.split('\t');
        if (f[0] === 'K') {
            rows.push({ label: f[1], live: Number(f[2]), allocated: Number(f[3]), freed: Number(f[4]), sites: [] });
        } else if (f[0] === 'S' && rows.length) {
            rows[rows.length - 1].sites.push({ count: Number(f[2]), func: f[3], file: f[4], line: Number(f[5]) });
        }
    }
    return rows;
}

/** `appels total propre nom` (nanosecondes) par ligne. */
function parseProfile(text: string): ProfileRow[] {
    return text.split('\n').filter(Boolean).map(line => {
        const [calls, total, own, ...name] = line.split('\t');
        return { name: name.join('\t'), calls: Number(calls), totalMs: Number(total) / 1e6, ownMs: Number(own) / 1e6 };
    });
}

/** Mention d'une fin anormale (` (code 3)`), vide si le programme a réussi. */
function exitNote(run: RunResult | undefined): string {
    if (!run || run.code === 0) { return ''; }
    return run.signal ? ` (arrêté par ${run.signal})` : ` (code de sortie ${run.code})`;
}

/** Mesure de temps ou de pic : un programme en erreur la fausserait. */
function requireSuccess(run: RunResult): RunResult {
    if (run.code !== 0) { throw new Error(`le programme s'est terminé en erreur${exitNote(run)}`); }
    return run;
}

/** `--src <dossier du script>` pour compiler une copie temporaire, sauf si
 *  la commande configurée fixe déjà son propre `--src`. */
function srcFlag(file: string): string[] {
    return resolveCommand('compilerPath', 'ocara', vscode.Uri.file(file)).includes('--src') ? [] : ['--src', path.dirname(file)];
}

/** Écart avec la mesure précédente du même genre (plus bas = mieux). */
function delta(entry: Entry, previous: Entry | undefined): string {
    if (!previous || previous.value <= 0) { return ''; }
    const pct = ((entry.value - previous.value) / previous.value) * 100;
    if (Math.abs(pct) < 1) { return '= '; }
    return `${pct > 0 ? '▲ +' : '▼ '}${pct.toFixed(1)} % · `;
}

function config(): vscode.WorkspaceConfiguration {
    return vscode.workspace.getConfiguration('ocara.benchmark');
}

async function withTempDir<T>(run: (dir: string) => Promise<T>): Promise<T> {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'ocara-bench-'));
    try {
        return await run(dir);
    } finally {
        fs.rmSync(dir, { recursive: true, force: true });
    }
}

/** Compile `file` en `binary` ; erreur du compilateur sinon. `script` :
 *  script d'origine (réglages `${workspaceFolder}`) quand `file` est une copie. */
async function build(file: string, binary: string, flags: string[], script = file): Promise<string> {
    const uri = vscode.Uri.file(script);
    const r = await runTool('compilerPath', 'ocara', [file, '-o', binary, ...flags], path.dirname(script), uri);
    if (r.launchError) { throw new Error(`impossible de lancer le compilateur (${r.launchError})`); }
    if (r.code !== 0) {
        ocaraOutput().appendLine(`${r.stderr}\n${r.stdout}`.replace(/\x1b\[[0-9;]*m/g, '').trim());
        ocaraOutput().show(true);
        throw new Error('la compilation a échoué (voir la sortie « Ocara »).');
    }
    return binary;
}

/** Exécute `binary` depuis `cwd` : durée (ms), pic mémoire (Ko, 0 si
 *  inconnu) et code de sortie. Délai dépassé : tout le groupe de processus
 *  est tué (`/usr/bin/time` ET le programme mesuré), puis erreur. */
function execute(binary: string, cwd: string, tmp: string, env: Record<string, string> = {}): Promise<RunResult> {
    const timeout = Math.max(1, config().get<number>('timeout', 30)) * 1000;
    const memFile = path.join(tmp, 'mem.txt');
    const measured = fs.existsSync(TIME_BIN);
    const [program, args] = measured ? [TIME_BIN, ['-f', '%M', '-o', memFile, binary]] : [binary, []];
    return new Promise((resolve, reject) => {
        const start = process.hrtime.bigint();
        // `detached` : groupe de processus propre, tuable d'un bloc.
        const child = spawn(program, args, { cwd, env: { ...process.env, ...env }, detached: process.platform !== 'win32', stdio: ['ignore', 'ignore', 'pipe'] });
        let stderr = '';
        let timedOut = false;
        child.stderr?.on('data', (chunk: Buffer) => { if (stderr.length < 64 * 1024) { stderr += chunk.toString(); } });
        const timer = setTimeout(() => { timedOut = true; killTree(child.pid); }, timeout);
        child.on('error', err => { clearTimeout(timer); reject(new Error(`impossible de lancer le programme (${err.message})`)); });
        child.on('close', (code, signal) => {
            clearTimeout(timer);
            if (timedOut) {
                reject(new Error(`délai dépassé (${timeout / 1000} s, réglage « ocara.benchmark.timeout »)`));
                return;
            }
            if (code !== 0 && stderr.trim()) { ocaraOutput().appendLine(stderr.trim()); }
            resolve({ ms: Number(process.hrtime.bigint() - start) / 1e6, peakKb: readPeak(measured, memFile), code, signal });
        });
    });
}

/** Tue le groupe de processus `pid` (repli : le seul processus). */
function killTree(pid: number | undefined): void {
    if (pid === undefined) { return; }
    try {
        process.kill(process.platform === 'win32' ? pid : -pid, 'SIGKILL');
    } catch {
        try { process.kill(pid, 'SIGKILL'); } catch { /* déjà terminé */ }
    }
}

/** Pic mémoire écrit par `/usr/bin/time -f %M` (dernière ligne), 0 sinon. */
function readPeak(measured: boolean, memFile: string): number {
    if (!measured || !fs.existsSync(memFile)) { return 0; }
    return parseInt(fs.readFileSync(memFile, 'utf8').trim().split('\n').pop() ?? '0', 10) || 0;
}

function formatMs(ms: number): string {
    return ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${ms.toFixed(ms < 10 ? 2 : 1)} ms`;
}

function formatKb(kb: number): string {
    return kb >= 1024 ? `${(kb / 1024).toFixed(1)} Mo` : `${kb} Ko`;
}
