import * as vscode from 'vscode';
import * as path from 'path';
import { parseToolMessages, runTool, toDiagnostic } from './toolrunner';

// ─────────────────────────────────────────────────────────────────────────────
// Analyse de style `ocaracs` automatique : à l'affichage, à l'ouverture et à
// l'enregistrement d'un fichier `.oc` — chaque avertissement devient un
// diagnostic (panneau Problèmes, message au survol de la ligne) et la ligne
// est surlignée en jaune. Les règles sont celles d'ocaracs : `.ocaracs` le
// plus proche en remontant depuis le dossier du script, sinon ses valeurs
// par défaut. Voir docs/roadmap.d/tooling-vscode-lint-compile-dump.md.
//
// ocaracs lit le fichier SUR DISQUE : l'analyse porte sur la dernière
// version enregistrée (relancée à chaque enregistrement).
//
// Les demandes rapprochées (ouverture + affichage) sont regroupées ; seul le
// résultat de la DERNIÈRE analyse d'un fichier encore ouvert est publié — un
// `ocaracs` en retard ne republie rien sur un fichier fermé ou ré-analysé.
// ─────────────────────────────────────────────────────────────────────────────

const SOURCE = 'ocaracs';
/** Délai de regroupement des demandes d'analyse d'un même fichier (ms). */
const DEBOUNCE_MS = 150;

export class OcaracsLinter implements vscode.Disposable {
    private readonly diagnostics = vscode.languages.createDiagnosticCollection(SOURCE);
    private readonly highlight = vscode.window.createTextEditorDecorationType({
        isWholeLine: true,
        backgroundColor: 'rgba(255, 200, 0, 0.15)',
        overviewRulerColor: 'rgba(255, 200, 0, 0.8)',
        overviewRulerLane: vscode.OverviewRulerLane.Right,
    });
    private readonly disposables: vscode.Disposable[] = [];
    /** Lignes à surligner par fichier (dernière analyse). */
    private readonly lines = new Map<string, number[]>();
    /** Analyse programmée par fichier (regroupement). */
    private readonly timers = new Map<string, NodeJS.Timeout>();
    /** Numéro de la dernière analyse lancée par fichier. */
    private readonly generations = new Map<string, number>();
    private launchErrorShown = false;

    constructor() {
        this.disposables.push(
            this.diagnostics,
            this.highlight,
            vscode.workspace.onDidOpenTextDocument(doc => this.schedule(doc)),
            vscode.workspace.onDidSaveTextDocument(doc => this.schedule(doc)),
            vscode.workspace.onDidCloseTextDocument(doc => this.clear(doc.uri)),
            vscode.window.onDidChangeActiveTextEditor(editor => { if (editor) { this.schedule(editor.document); } }),
            vscode.window.onDidChangeVisibleTextEditors(editors => editors.forEach(e => this.decorate(e))),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.lint') || e.affectsConfiguration('ocara.ocaracsPath')) { this.reconfigure(); }
            }),
        );
        this.lintVisible();
    }

    /** Relance l'analyse des éditeurs visibles (après `ocaracs --fix`). */
    relint(): void {
        this.lintVisible();
    }

    private lintVisible(): void {
        for (const editor of vscode.window.visibleTextEditors) { this.schedule(editor.document); }
    }

    /** Réglages modifiés : lint désactivé → tout effacer (diagnostics ET
     *  surlignage), sinon ré-analyser les éditeurs visibles. */
    private reconfigure(): void {
        if (enabled()) { this.lintVisible(); return; }
        for (const file of [...this.lines.keys(), ...this.timers.keys()]) { this.clear(vscode.Uri.file(file)); }
        this.diagnostics.clear();
    }

    /** Programme l'analyse de `document` (demandes rapprochées regroupées). */
    private schedule(document: vscode.TextDocument): void {
        if (document.languageId !== 'ocara' || document.uri.scheme !== 'file') { return; }
        const file = document.uri.fsPath;
        clearTimeout(this.timers.get(file));
        this.timers.set(file, setTimeout(() => {
            this.timers.delete(file);
            void this.lint(document);
        }, DEBOUNCE_MS));
    }

    private async lint(document: vscode.TextDocument): Promise<void> {
        if (!enabled()) {
            this.clear(document.uri);
            return;
        }
        const file = document.uri.fsPath;
        const generation = (this.generations.get(file) ?? 0) + 1;
        this.generations.set(file, generation);
        const cwd = path.dirname(file);
        const result = await runTool('ocaracsPath', 'ocaracs', [file], cwd, document.uri);
        // Résultat périmé : fichier fermé, analyse plus récente lancée, ou lint désactivé entre-temps.
        if (document.isClosed || this.generations.get(file) !== generation || !enabled()) { return; }
        if (result.launchError) {
            if (!this.launchErrorShown) {
                this.launchErrorShown = true;
                vscode.window.showWarningMessage(`Ocara : impossible de lancer ocaracs (${result.launchError}) — réglage « ocara.ocaracsPath ».`);
            }
            return;
        }
        // ocaracs analyse aussi les imports du fichier : ne garder que ses propres avertissements.
        const own = parseToolMessages(result.stderr, cwd).filter(m => m.file === file);
        this.diagnostics.set(document.uri, own.map(m => toDiagnostic(document, m, SOURCE)));
        this.lines.set(file, own.map(m => m.line));
        this.redecorate(file);
    }

    private redecorate(file: string): void {
        vscode.window.visibleTextEditors.filter(e => e.document.uri.fsPath === file).forEach(e => this.decorate(e));
    }

    private decorate(editor: vscode.TextEditor): void {
        const lines = this.lines.get(editor.document.uri.fsPath) ?? [];
        editor.setDecorations(this.highlight, lines.map(l => new vscode.Range(l, 0, l, 0)));
    }

    /** Efface diagnostics et surlignage de `uri` ; une analyse en cours ou
     *  programmée ne publiera rien. */
    private clear(uri: vscode.Uri): void {
        const file = uri.fsPath;
        clearTimeout(this.timers.get(file));
        this.timers.delete(file);
        this.generations.set(file, (this.generations.get(file) ?? 0) + 1);
        this.diagnostics.delete(uri);
        this.lines.delete(file);
        this.redecorate(file);
    }

    dispose(): void {
        this.timers.forEach(t => clearTimeout(t));
        this.disposables.forEach(d => d.dispose());
    }
}

function enabled(): boolean {
    return vscode.workspace.getConfiguration('ocara').get<boolean>('lint.enable', true);
}
