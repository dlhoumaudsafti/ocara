import * as vscode from 'vscode';
import * as fs from 'fs';
import * as path from 'path';
import { parseToolMessages, runTool } from './toolrunner';
import { coverageConfig, qualityRoots } from './projects';
import { LinterProvider, LintSummary } from './linterview';
import { CoverageProvider, CoverageSummary, coverageScope, formatPercent } from './coverage';

// ─────────────────────────────────────────────────────────────────────────────
// Voyants de qualité « prêt pour la release » de l'espace de travail, sur les
// projets testés (voir projects.ts) :
//   - linter       : l'encart Linter (`ocaracs <projet>`) ne signale rien ;
//   - couverture   : tous les tests passent et la couverture par ligne
//                    atteint `ocara.coverage.threshold` (100 % par défaut) ;
//   - architecture : `ocara main.oc --check-architecture` réussit pour chaque
//                    projet qui a un `main.oc`.
// LED du panneau : part des voyants au vert — rouge sous 33,33 %, orange
// jusqu'à 66,66 %, bleu jusqu'à 99,99 %, vert à 100 % (les boutons de
// compilation Release passent alors au vert).
// ─────────────────────────────────────────────────────────────────────────────

const DEBOUNCE_MS = 800;

export type Light = 'ok' | 'ko' | 'pending';

export interface Indicator { light: Light; detail: string }

export interface QualityState {
    lint: Indicator;
    coverage: Indicator;
    architecture: Indicator;
    /** Part des voyants au vert, en pourcentage. */
    score: number;
}

const PENDING: Indicator = { light: 'pending', detail: 'analyse en cours…' };

export class QualityMonitor implements vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<QualityState>();
    readonly onChange = this.changed.event;
    private lint: Indicator = PENDING;
    private architecture: Indicator = PENDING;
    private coverage: Indicator = PENDING;
    private timer: NodeJS.Timeout | undefined;
    private generation = 0;
    private readonly disposables: vscode.Disposable[] = [];

    constructor(private readonly coverageProvider: CoverageProvider, private readonly linter: LinterProvider) {}

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            this.coverageProvider.onResult(summary => this.setCoverage(summary)),
            this.linter.onResult(summary => this.setLint(summary)),
            vscode.workspace.onDidSaveTextDocument(doc => { if (doc.languageId === 'ocara') { this.schedule(); } }),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.coverage.threshold') || e.affectsConfiguration('ocara.coverage.scope')) { this.setCoverage(this.coverageProvider.current()); }
                if (e.affectsConfiguration('ocara')) { this.schedule(); }
            }),
        );
        context.subscriptions.push(this);
        this.setCoverage(this.coverageProvider.current());
        this.setLint(this.linter.current());
        this.schedule();
    }

    private setLint(summary: LintSummary | undefined): void {
        if (!summary) {
            const enabled = vscode.workspace.getConfiguration('ocara').get<boolean>('lint.enable', true);
            this.lint = { light: 'pending', detail: enabled ? 'analyse en cours…' : 'analyse désactivée (ocara.lint.enable)' };
        } else if (summary.launchError) {
            this.lint = { light: 'ko', detail: `ocaracs introuvable (${summary.launchError})` };
        } else if (summary.roots.length === 0) {
            this.lint = { light: 'ko', detail: 'aucun dossier à analyser' };
        } else {
            this.lint = summary.count === 0
                ? { light: 'ok', detail: 'aucun avertissement' }
                : { light: 'ko', detail: `${summary.count} avertissement(s) ocaracs` };
        }
        this.publish();
    }

    state(): QualityState {
        const lights = [this.lint, this.coverage, this.architecture];
        const score = lights.filter(i => i.light === 'ok').length / lights.length * 100;
        return { lint: this.lint, coverage: this.coverage, architecture: this.architecture, score };
    }

    private publish(): void {
        this.changed.fire(this.state());
    }

    private setCoverage(summary: CoverageSummary | undefined): void {
        const threshold = coverageConfig().get<number>('threshold', 100);
        if (!summary) {
            const auto = coverageConfig().get<boolean>('auto', true);
            const detail = coverageScope() === 'folder'
                ? 'non mesurée — couverture en mode dossier (réglage « ocara.coverage.scope » à « workspace » pour la LED)'
                : auto ? 'analyse en cours…' : 'non mesurée — lancer les tests et la couverture';
            this.coverage = { light: 'pending', detail };
        } else if (summary.projects === 0) {
            this.coverage = { light: 'ko', detail: 'aucun projet testé (dossier tests/ avec des *Test.oc)' };
        } else if (summary.fail > 0 || summary.errors > 0 || summary.failedProjects.length > 0) {
            this.coverage = { light: 'ko', detail: `tests en échec : ${summary.fail} FAIL, ${summary.errors} erreur(s)` };
        } else {
            const ok = summary.percent >= threshold;
            this.coverage = { light: ok ? 'ok' : 'ko', detail: `${formatPercent(summary.percent)} (seuil ${threshold} %)` };
        }
        this.publish();
    }

    private schedule(): void {
        clearTimeout(this.timer);
        this.timer = setTimeout(() => void this.refresh(), DEBOUNCE_MS);
    }

    /** Architecture, revérifiée à chaque enregistrement (le linter vient de l'encart Linter). */
    private async refresh(): Promise<void> {
        const generation = ++this.generation;
        const architecture = await checkArchitecture(await qualityRoots());
        if (generation !== this.generation) { return; }
        this.architecture = architecture;
        this.publish();
    }

    dispose(): void {
        clearTimeout(this.timer);
        this.disposables.forEach(d => d.dispose());
    }
}

async function checkArchitecture(roots: string[]): Promise<Indicator> {
    const entries = roots.map(root => ({ root, main: path.join(root, 'main.oc') })).filter(e => fs.existsSync(e.main));
    if (entries.length === 0) { return { light: 'ok', detail: 'aucun point d\'entrée main.oc à vérifier' }; }
    const errors: string[] = [];
    for (const { root, main } of entries) {
        const result = await runTool('compilerPath', 'ocara', [main, '--check-architecture', '--src', root], root, vscode.Uri.file(main));
        if (result.launchError) { return { light: 'ko', detail: `compilateur introuvable (${result.launchError})` }; }
        if (result.code !== 0) {
            errors.push(...parseToolMessages(result.stderr + result.stdout, root).map(m => `${path.relative(root, m.file)}:${m.line + 1} ${m.message}`));
            if (errors.length === 0) { errors.push(`${path.basename(root)} : vérification en échec`); }
        }
    }
    return errors.length === 0
        ? { light: 'ok', detail: 'architecture respectée' }
        : { light: 'ko', detail: `${errors.length} erreur(s) : ${errors[0]}` };
}
