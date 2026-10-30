import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import * as os from 'os';
import { ocaraOutput, parseToolMessages, runTool, toDiagnostic, ToolResult } from './toolrunner';

// ─────────────────────────────────────────────────────────────────────────────
// Commandes « Compiler (Debug/Release) », « Compiler et lancer
// (Debug/Release) » et « Afficher le dump » (menu clic droit de l'éditeur et
// de l'arborescence, palette de commandes ; Release = `--release`) — compilateur configurable (`ocara.compilerPath`). Voir
// docs/roadmap.d/tooling-vscode-lint-compile-dump.md et
// docs/roadmap.d/tooling-vscode-run-and-fix.md.
// « Compiler et lancer » vise le script de démarrage du projet s'il est
// réglé (`ocara.startupScript`, commande « Choisir le script de démarrage »),
// sinon le script cliqué ou affiché. Binaire proposé : `oc.out.bin` (sortie
// par défaut du compilateur, ignorée par git).
// ─────────────────────────────────────────────────────────────────────────────

const SOURCE = 'ocara';

/** Nom de binaire proposé : sortie par défaut du compilateur (src/core/cli.rs). */
const DEFAULT_BINARY = 'oc.out.bin';

export class OcaraCompiler implements vscode.Disposable {
    private readonly output = ocaraOutput();
    private readonly diagnostics = vscode.languages.createDiagnosticCollection(SOURCE);
    /** Terminal « Ocara » du dernier lancement (remplacé au suivant). */
    private terminal: vscode.Terminal | undefined;

    register(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
            this,
            vscode.commands.registerCommand('ocara.compile', (uri?: vscode.Uri) => this.compile(uri, false)),
            vscode.commands.registerCommand('ocara.compileRelease', (uri?: vscode.Uri) => this.compile(uri, true)),
            vscode.commands.registerCommand('ocara.compileAndRun', (uri?: vscode.Uri) => this.compileAndRun(uri, false)),
            vscode.commands.registerCommand('ocara.compileAndRunRelease', (uri?: vscode.Uri) => this.compileAndRun(uri, true)),
            vscode.window.onDidCloseTerminal(t => { if (t === this.terminal) { this.terminal = undefined; } }),
            vscode.commands.registerCommand('ocara.dump', (uri?: vscode.Uri) => this.dump(uri)),
            vscode.commands.registerCommand('ocara.chooseStartupScript', () => chooseStartupScript()),
            // Diagnostics de compilation périmés dès que le fichier change.
            vscode.workspace.onDidChangeTextDocument(e => { if (e.contentChanges.length) { this.diagnostics.delete(e.document.uri); } }),
            vscode.workspace.onDidSaveTextDocument(doc => this.diagnostics.delete(doc.uri)),
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
            value: DEFAULT_BINARY,
            validateInput: v => (v.trim() === '' || /[\\/]/.test(v) ? 'Nom de fichier simple attendu (sans dossier)' : undefined),
        });
        return name === undefined ? undefined : path.join(dir, name.trim());
    }

    /** Compile `file` en `binary` (`--release` si `release`) ; `true` si succès. */
    private async build(file: vscode.Uri, binary: string, release: boolean): Promise<boolean> {
        const args = [file.fsPath, '-o', binary, ...(release ? ['--release'] : [])];
        const mode = release ? 'Release' : 'Debug';
        const result = await vscode.window.withProgress(
            { location: vscode.ProgressLocation.Notification, title: `Ocara : compilation (${mode}) de ${path.basename(file.fsPath)}…` },
            () => runTool('compilerPath', 'ocara', args, path.dirname(file.fsPath), file),
        );
        return this.report(file, result, `compilation de ${file.fsPath}`);
    }

    private async compile(uri: vscode.Uri | undefined, release: boolean): Promise<void> {
        const file = await this.targetFile(uri);
        const binary = file && await this.askBinary(file, `Compiler le script Ocara (${release ? 'Release' : 'Debug'})`);
        if (!file || !binary || !await this.build(file, binary, release)) { return; }
        vscode.window.showInformationMessage(`Ocara : binaire créé — ${binary}`);
    }

    /** Compile puis lance le binaire depuis le dossier du script, dans le
     *  terminal « Ocara » (le programme précédent est d'abord arrêté). Le
     *  binaire EST le processus du terminal : aucun shell, donc aucune
     *  syntaxe propre à un shell (POSIX, PowerShell, cmd). */
    private async compileAndRun(uri: vscode.Uri | undefined, release: boolean): Promise<void> {
        const startup = startupScript();
        if (startup === null) { return; }
        const file = startup ? await this.targetFile(startup) : await this.targetFile(uri);
        const binary = file && await this.askBinary(file, `Compiler et lancer le script Ocara (${release ? 'Release' : 'Debug'})`);
        if (!file || !binary || !await this.build(file, binary, release)) { return; }
        this.terminal?.dispose();
        this.terminal = vscode.window.createTerminal({ name: 'Ocara', cwd: path.dirname(file.fsPath), shellPath: binary });
        this.terminal.show(true);
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

/** Script de démarrage réglé (`ocara.startupScript`, relatif au dossier de
 *  l'espace de travail) ; `undefined` s'il n'est pas réglé, `null` s'il est
 *  introuvable (message affiché). */
function startupScript(): vscode.Uri | undefined | null {
    const value = vscode.workspace.getConfiguration('ocara').get<string>('startupScript', '').trim();
    if (!value) { return undefined; }
    const candidates = path.isAbsolute(value) ? [value] : (vscode.workspace.workspaceFolders ?? []).map(f => path.join(f.uri.fsPath, value));
    const found = candidates.find(f => fs.existsSync(f));
    if (!found) {
        vscode.window.showErrorMessage(`Ocara : script de démarrage introuvable — ${value} (réglage « ocara.startupScript »).`);
        return null;
    }
    return vscode.Uri.file(found);
}

/** Choix du script de démarrage parmi les `.oc` de l'espace de travail
 *  (ceux qui déclarent `function main` d'abord) ; enregistré dans les
 *  réglages de l'espace de travail. */
async function chooseStartupScript(): Promise<void> {
    const files = await vscode.workspace.findFiles('**/*.oc', '**/{node_modules,target,out,tests,.git}/**', 5000);
    const current = vscode.workspace.getConfiguration('ocara').get<string>('startupScript', '');
    const items = files.map(uri => {
        let main = false;
        try { main = /^\s*function\s+main\s*\(/m.test(fs.readFileSync(uri.fsPath, 'utf8')); } catch { /* illisible */ }
        const relative = vscode.workspace.asRelativePath(uri, false);
        return { label: relative, description: main ? 'function main' : '', main, picked: relative === current };
    }).sort((a, b) => Number(b.main) - Number(a.main) || a.label.localeCompare(b.label));
    const none = { label: '$(close) Aucun', description: 'compiler et lancer le script affiché', main: false, picked: false };
    const choice = await vscode.window.showQuickPick([none, ...items], {
        title: 'Ocara : script de démarrage (Compiler et lancer)',
        placeHolder: current ? `Actuel : ${current}` : 'Aucun : le script affiché est lancé',
    });
    if (!choice) { return; }
    const target = vscode.workspace.workspaceFolders ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
    await vscode.workspace.getConfiguration('ocara').update('startupScript', choice === none ? undefined : choice.label, target);
}
