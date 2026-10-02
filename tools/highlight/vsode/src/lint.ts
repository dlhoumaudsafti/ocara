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
// ─────────────────────────────────────────────────────────────────────────────

const SOURCE = 'ocaracs';

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
    private launchErrorShown = false;

    constructor() {
        this.disposables.push(
            this.diagnostics,
            this.highlight,
            vscode.workspace.onDidOpenTextDocument(doc => this.lint(doc)),
            vscode.workspace.onDidSaveTextDocument(doc => this.lint(doc)),
            vscode.workspace.onDidCloseTextDocument(doc => this.clear(doc.uri)),
            vscode.window.onDidChangeActiveTextEditor(editor => { if (editor) { this.lint(editor.document); } }),
            vscode.window.onDidChangeVisibleTextEditors(editors => editors.forEach(e => this.decorate(e))),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara.lint') || e.affectsConfiguration('ocara.ocaracsPath')) { this.lintVisible(); }
            }),
        );
        this.lintVisible();
    }

    /** Relance l'analyse des éditeurs visibles (après `ocaracs --fix`). */
    relint(): void {
        this.lintVisible();
    }

    private lintVisible(): void {
        for (const editor of vscode.window.visibleTextEditors) { this.lint(editor.document); }
    }

    async lint(document: vscode.TextDocument): Promise<void> {
        if (document.languageId !== 'ocara' || document.uri.scheme !== 'file') { return; }
        if (!vscode.workspace.getConfiguration('ocara').get<boolean>('lint.enable', true)) {
            this.clear(document.uri);
            return;
        }
        const file = document.uri.fsPath;
        const cwd = path.dirname(file);
        const result = await runTool('ocaracsPath', 'ocaracs', [file], cwd, document.uri);
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
        vscode.window.visibleTextEditors.filter(e => e.document.uri.fsPath === file).forEach(e => this.decorate(e));
    }

    private decorate(editor: vscode.TextEditor): void {
        const lines = this.lines.get(editor.document.uri.fsPath) ?? [];
        editor.setDecorations(this.highlight, lines.map(l => new vscode.Range(l, 0, l, 0)));
    }

    private clear(uri: vscode.Uri): void {
        this.diagnostics.delete(uri);
        this.lines.delete(uri.fsPath);
    }

    dispose(): void {
        this.disposables.forEach(d => d.dispose());
    }
}
