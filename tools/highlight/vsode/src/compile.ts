import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { ocaraOutput, parseToolMessages, runTool, toDiagnostic, ToolResult } from './toolrunner';

// ─────────────────────────────────────────────────────────────────────────────
// Commandes « Compiler le script », « Compiler et lancer » et « Afficher le
// dump » (menu clic droit de l'éditeur et de l'arborescence, palette de
// commandes) — compilateur configurable (`ocara.compilerPath`). Voir
// docs/roadmap.d/tooling-vscode-lint-compile-dump.md et
// docs/roadmap.d/tooling-vscode-run-and-fix.md.
// ─────────────────────────────────────────────────────────────────────────────

const SOURCE = 'ocara';

/** Argument shell entre apostrophes (`'` échappé). */
function shellQuote(value: string): string {
    return `'${value.replace(/'/g, `'\\''`)}'`;
}

export class OcaraCompiler implements vscode.Disposable {
    private readonly output = ocaraOutput();
    private readonly diagnostics = vscode.languages.createDiagnosticCollection(SOURCE);
    /** Terminal « Ocara » unique, partagé par tous les lancements. */
    private terminal: vscode.Terminal | undefined;

    register(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
            this,
            vscode.commands.registerCommand('ocara.compile', (uri?: vscode.Uri) => this.compile(uri)),
            vscode.commands.registerCommand('ocara.compileAndRun', (uri?: vscode.Uri) => this.compileAndRun(uri)),
            vscode.window.onDidCloseTerminal(t => { if (t === this.terminal) { this.terminal = undefined; } }),
            vscode.commands.registerCommand('ocara.dump', (uri?: vscode.Uri) => this.dump(uri)),
        );
    }

    /** Script visé : fichier cliqué dans l'arborescence, sinon éditeur actif. */
    private async targetFile(uri?: vscode.Uri): Promise<vscode.Uri | undefined> {
        const target = uri ?? vscode.window.activeTextEditor?.document.uri;
        if (!target || target.scheme !== 'file' || path.extname(target.fsPath) !== '.oc') {
            vscode.window.showWarningMessage('Ocara : sélectionnez un script .oc.');
            return undefined;
        }
        // Compiler la version affichée, pas une version périmée sur disque.
        const open = vscode.workspace.textDocuments.find(d => d.uri.fsPath === target.fsPath);
        if (open?.isDirty) { await open.save(); }
        return target;
    }

    /** Nom du binaire (créé dans le dossier du script) ; `undefined` si annulé. */
    private async askBinary(file: vscode.Uri, title: string): Promise<string | undefined> {
        const dir = path.dirname(file.fsPath);
        const name = await vscode.window.showInputBox({
            title,
            prompt: `Nom du binaire (créé dans ${dir})`,
            value: path.basename(file.fsPath, '.oc'),
            validateInput: v => (v.trim() === '' || /[\\/]/.test(v) ? 'Nom de fichier simple attendu (sans dossier)' : undefined),
        });
        return name === undefined ? undefined : path.join(dir, name.trim());
    }

    /** Compile `file` en `binary` ; `true` si succès. */
    private async build(file: vscode.Uri, binary: string): Promise<boolean> {
        const result = await vscode.window.withProgress(
            { location: vscode.ProgressLocation.Notification, title: `Ocara : compilation de ${path.basename(file.fsPath)}…` },
            () => runTool('compilerPath', 'ocara', [file.fsPath, '-o', binary], path.dirname(file.fsPath), file),
        );
        return this.report(file, result, `compilation de ${file.fsPath}`);
    }

    private async compile(uri?: vscode.Uri): Promise<void> {
        const file = await this.targetFile(uri);
        const binary = file && await this.askBinary(file, 'Compiler le script Ocara');
        if (!file || !binary || !await this.build(file, binary)) { return; }
        vscode.window.showInformationMessage(`Ocara : binaire créé — ${binary}`);
    }

    /** Compile puis lance le binaire depuis le dossier du script, dans le
     *  terminal « Ocara » (le programme précédent y est d'abord arrêté). */
    private async compileAndRun(uri?: vscode.Uri): Promise<void> {
        const file = await this.targetFile(uri);
        const binary = file && await this.askBinary(file, 'Compiler et lancer le script Ocara');
        if (!file || !binary || !await this.build(file, binary)) { return; }
        if (this.terminal) {
            this.terminal.sendText('\x03', false);
        } else {
            this.terminal = vscode.window.createTerminal({ name: 'Ocara' });
        }
        this.terminal.show(true);
        this.terminal.sendText(`cd ${shellQuote(path.dirname(file.fsPath))} && ${shellQuote(binary)}`);
    }

    private async dump(uri?: vscode.Uri): Promise<void> {
        const file = await this.targetFile(uri);
        if (!file) { return; }
        const dir = path.dirname(file.fsPath);
        // `--dump` poursuit jusqu'au binaire (et son `.o`) : produits dans un
        // dossier temporaire supprimé aussitôt — seul le texte est conservé.
        const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'ocara-dump-'));
        try {
            const result = await vscode.window.withProgress(
                { location: vscode.ProgressLocation.Notification, title: `Ocara : dump de ${path.basename(file.fsPath)}…` },
                () => runTool('compilerPath', 'ocara', [file.fsPath, '--dump', '-o', path.join(tmpDir, 'dump')], dir, file),
            );
            if (!this.report(file, result, `dump de ${file.fsPath}`)) { return; }
            const doc = await vscode.workspace.openTextDocument({
                content: `// Dump Ocara — ${file.fsPath}\n\n${result.stdout}`,
                language: 'plaintext',
            });
            await vscode.window.showTextDocument(doc, { preview: false });
        } finally {
            fs.rmSync(tmpDir, { recursive: true, force: true });
        }
    }

    /** Erreurs du compilateur → canal de sortie + diagnostics ; `true` si succès. */
    private report(file: vscode.Uri, result: ToolResult, action: string): boolean {
        this.diagnostics.clear();
        if (result.launchError) {
            vscode.window.showErrorMessage(`Ocara : impossible de lancer le compilateur (${result.launchError}) — réglage « ocara.compilerPath ».`);
            return false;
        }
        if (result.code === 0) { return true; }

        const dir = path.dirname(file.fsPath);
        const output = `${result.stderr}\n${result.stdout}`;
        const byFile = new Map<string, vscode.Diagnostic[]>();
        for (const msg of parseToolMessages(output, dir)) {
            const doc = vscode.workspace.textDocuments.find(d => d.uri.fsPath === msg.file);
            const list = byFile.get(msg.file) ?? [];
            list.push(toDiagnostic(doc, msg, SOURCE));
            byFile.set(msg.file, list);
        }
        byFile.forEach((list, f) => this.diagnostics.set(vscode.Uri.file(f), list));

        this.output.clear();
        this.output.appendLine(`Échec — ${action}`);
        this.output.appendLine(output.replace(/\x1b\[[0-9;]*m/g, '').trim());
        this.output.show(true);
        vscode.window.showErrorMessage(`Ocara : échec — ${action} (voir le panneau Problèmes / la sortie « Ocara »).`);
        return false;
    }

    dispose(): void {
        this.diagnostics.dispose();
    }
}
