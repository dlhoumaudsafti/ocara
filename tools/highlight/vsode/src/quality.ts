import * as vscode from 'vscode';
import { coverageConfig } from './projects';
import { LinterProvider, LintSummary } from './linterview';
import { CoverageProvider, CoverageSummary, formatPercent } from './coverage';
import { ArchitectureProvider, ArchitectureSummary } from './architecture';
import { autoEnabled, currentTarget, describeTarget, sameTarget, Target } from './scope';

// ─────────────────────────────────────────────────────────────────────────────
// LED « Release » du panneau Configuration et ses trois voyants, chacun repris
// de son encart et du périmètre de son réglage (`ocara.<section>.scope` :
// espace de travail, dossier du script affiché ou script affiché — voir
// scope.ts) :
//   - Linter       : dernier résultat de l'encart Linter, sans avertissement ;
//   - Coverage     : dernier résultat de l'encart Coverage, tous les tests
//                    passent et la couverture par ligne atteint
//                    `ocara.coverage.threshold` (100 %) ;
//   - Architecture : dernier résultat de l'encart Architecture, sans erreur.
// Un résultat ne compte que s'il porte sur la cible courante du périmètre
// (même dossier ou même script affiché) ; sinon le voyant est « non mesuré ».
// LED : part des voyants au vert — rouge sous 33,33 %, orange jusqu'à
// 66,66 %, bleu jusqu'à 99,99 %, vert à 100 % (boutons de compilation
// Release alors au vert).
// ─────────────────────────────────────────────────────────────────────────────

export type Light = 'ok' | 'ko' | 'pending';

export interface Indicator { light: Light; detail: string }

export interface QualityState {
    lint: Indicator;
    coverage: Indicator;
    architecture: Indicator;
    /** Part des voyants au vert, en pourcentage. */
    score: number;
}

/** Voyant d'un encart sans résultat pour la cible courante. */
function notMeasured(section: string, label: string): Indicator {
    const target = currentTarget(section);
    const how = autoEnabled(section) ? 'analyse automatique en attente' : `bouton ▶ de l'encart ${label}`;
    return { light: 'pending', detail: `non mesuré pour : ${describeTarget(target, section)} — ${how}` };
}

/** Le résultat porte-t-il sur la cible courante du périmètre de `section` ? */
function current(section: string, target: Target | undefined): boolean {
    return sameTarget(target, currentTarget(section));
}

function where(target: Target): string {
    return target.scope === 'workspace' ? '' : ` — ${describeTarget(target)}`;
}

export class QualityMonitor implements vscode.Disposable {
    private readonly changed = new vscode.EventEmitter<QualityState>();
    readonly onChange = this.changed.event;
    private lint: Indicator = notMeasured('lint', 'Linter');
    private coverage: Indicator = notMeasured('coverage', 'Coverage');
    private architecture: Indicator = notMeasured('architecture', 'Architecture');
    private readonly disposables: vscode.Disposable[] = [];

    constructor(
        private readonly coverageProvider: CoverageProvider,
        private readonly linter: LinterProvider,
        private readonly architectureProvider: ArchitectureProvider,
    ) {}

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            this.coverageProvider.onResult(() => this.reevaluate()),
            this.linter.onResult(() => this.reevaluate()),
            this.architectureProvider.onResult(() => this.reevaluate()),
            vscode.window.onDidChangeActiveTextEditor(() => this.reevaluate()),
            vscode.workspace.onDidChangeConfiguration(e => { if (e.affectsConfiguration('ocara')) { this.reevaluate(); } }),
        );
        context.subscriptions.push(this);
        this.reevaluate();
    }

    state(): QualityState {
        const lights = [this.lint, this.coverage, this.architecture];
        const score = lights.filter(i => i.light === 'ok').length / lights.length * 100;
        return { lint: this.lint, coverage: this.coverage, architecture: this.architecture, score };
    }

    /** Derniers résultats des encarts, jugés selon les périmètres et le script affiché courants. */
    private reevaluate(): void {
        this.lint = lintIndicator(this.linter.current());
        this.coverage = coverageIndicator(this.coverageProvider.current());
        this.architecture = architectureIndicator(this.architectureProvider.current());
        this.changed.fire(this.state());
    }

    dispose(): void {
        this.disposables.forEach(d => d.dispose());
    }
}

function lintIndicator(s: LintSummary | undefined): Indicator {
    if (!s || !current('lint', s.target)) { return notMeasured('lint', 'Linter'); }
    if (s.launchError) { return { light: 'ko', detail: `ocaracs introuvable (${s.launchError})` }; }
    if (s.roots.length === 0) { return { light: 'ko', detail: 'aucun dossier à analyser' }; }
    return s.count === 0
        ? { light: 'ok', detail: `aucun avertissement${where(s.target)}` }
        : { light: 'ko', detail: `${s.count} avertissement(s) ocaracs${where(s.target)}` };
}

function coverageIndicator(s: CoverageSummary | undefined): Indicator {
    if (!s || !current('coverage', s.target)) { return notMeasured('coverage', 'Coverage'); }
    if (s.projects === 0) { return { light: 'ko', detail: `aucun projet testé (dossier tests/ avec des *Test.oc)${where(s.target)}` }; }
    if (s.fail > 0 || s.errors > 0 || s.failedProjects.length > 0) {
        return { light: 'ko', detail: `tests en échec : ${s.fail} FAIL, ${s.errors} erreur(s)${where(s.target)}` };
    }
    const threshold = coverageConfig().get<number>('threshold', 100);
    return { light: s.percent >= threshold ? 'ok' : 'ko', detail: `${formatPercent(s.percent)} (seuil ${threshold} %)${where(s.target)}` };
}

function architectureIndicator(s: ArchitectureSummary | undefined): Indicator {
    if (!s || !current('architecture', s.target)) { return notMeasured('architecture', 'Architecture'); }
    if (s.launchError) { return { light: 'ko', detail: `compilateur introuvable (${s.launchError})` }; }
    if (s.count > 0) { return { light: 'ko', detail: `${s.count} erreur(s)${where(s.target)}` }; }
    return { light: 'ok', detail: s.checked === 0
        ? `aucun script à vérifier${where(s.target)}`
        : `architecture respectée (${s.checked} script(s))${where(s.target)}` };
}
