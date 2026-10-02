import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { OcaracsLinter } from './lint';
import { ocaraOutput, runTool, ToolResult } from './toolrunner';

// ─────────────────────────────────────────────────────────────────────────────
// Commande « Fixer la mise en forme » (menu clic droit de l'éditeur et de
// l'arborescence, palette) : `ocaracs --fix` sur le fichier ou le dossier.
// `--fix --dry-run` d'abord : si des identifiants doivent être renommés dans
// le projet, confirmation avec la liste. Compte rendu dans le canal « Ocara ».
// Voir docs/roadmap.d/tooling-vscode-run-and-fix.md.
// ─────────────────────────────────────────────────────────────────────────────

const RENAME_RE = /^ocaracs: renommé : (\S+) → (\S+)$/;
const MOVE_RE = /^ocaracs: fichier renommé : (.+) → (.+)$/;
const SUMMARY_RE = /^ocaracs --fix : .*$/m;
const MAX_LISTED = 15;

function lines(result: ToolResult): string[] {
    return result.stderr.replace(/\x1b\[[0-9;]*m/g, '').split('\n');
}

export function registerFix(context: vscode.ExtensionContext, linter: OcaracsLinter): void {
    context.subscriptions.push(
        vscode.commands.registerCommand('ocara.fixStyle', (uri?: vscode.Uri) => fixStyle(uri, linter)),
    );
}

async function fixStyle(uri: vscode.Uri | undefined, linter: OcaracsLinter): Promise<void> {
    const target = uri ?? vscode.window.activeTextEditor?.document.uri;
    if (!target || target.scheme !== 'file') {
        vscode.window.showWarningMessage('Ocara : sélectionnez un script .oc ou un dossier.');
        return;
    }
    const isDir = fs.statSync(target.fsPath).isDirectory();
    if (!isDir && path.extname(target.fsPath) !== '.oc') {
        vscode.window.showWarningMessage('Ocara : sélectionnez un script .oc ou un dossier.');
        return;
    }
    // Le renommage porte sur tout le projet : enregistrer tous les scripts modifiés.
    for (const doc of vscode.workspace.textDocuments.filter(d => d.isDirty && d.languageId === 'ocara')) {
        await doc.save();
    }
    const cwd = isDir ? target.fsPath : path.dirname(target.fsPath);
    const output = ocaraOutput();

    const preview = await runTool('ocaracsPath', 'ocaracs', ['--fix', '--dry-run', target.fsPath], cwd, target);
    if (preview.launchError) {
        vscode.window.showErrorMessage(`Ocara : impossible de lancer ocaracs (${preview.launchError}) — réglage « ocara.ocaracsPath ».`);
        return;
    }
    const renames = lines(preview).map(l => l.match(RENAME_RE)).filter((m): m is RegExpMatchArray => m !== null);
    if (renames.length > 0) {
        const listed = renames.slice(0, MAX_LISTED).map(m => `${m[1]} → ${m[2]}`);
        const more = renames.length > MAX_LISTED ? `\n… et ${renames.length - MAX_LISTED} autre(s)` : '';
        const choice = await vscode.window.showWarningMessage(
            `ocaracs --fix va renommer ${renames.length} identifiant(s) dans tout le projet (déclarations et usages).`,
            { modal: true, detail: listed.join('\n') + more },
            'Appliquer',
        );
        if (choice !== 'Appliquer') { return; }
    }

    const result = await vscode.window.withProgress(
        { location: vscode.ProgressLocation.Notification, title: `Ocara : mise en forme de ${path.basename(target.fsPath)}…` },
        () => runTool('ocaracsPath', 'ocaracs', ['--fix', target.fsPath], cwd, target),
    );
    output.clear();
    output.appendLine(`ocaracs --fix ${target.fsPath}`);
    output.appendLine(result.stderr.replace(/\x1b\[[0-9;]*m/g, '').trim());

    await reopenMovedFiles(lines(result), cwd);
    linter.relint();
    const summary = result.stderr.match(SUMMARY_RE)?.[0] ?? 'ocaracs --fix terminé.';
    const action = await vscode.window.showInformationMessage(`Ocara : ${summary.replace(/^ocaracs --fix : /, '')}`, 'Voir le détail');
    if (action) { output.show(true); }
}

/** Un fichier renommé par `--fix` (classe renommée) est rouvert sous son nouveau nom. */
async function reopenMovedFiles(output: string[], cwd: string): Promise<void> {
    for (const m of output.map(l => l.match(MOVE_RE)).filter((m): m is RegExpMatchArray => m !== null)) {
        const [from, to] = [path.resolve(cwd, m[1]), path.resolve(cwd, m[2])];
        const open = vscode.window.tabGroups.all.flatMap(g => g.tabs)
            .filter(t => t.input instanceof vscode.TabInputText && t.input.uri.fsPath === from);
        if (open.length === 0) { continue; }
        await vscode.window.tabGroups.close(open);
        await vscode.window.showTextDocument(vscode.Uri.file(to), { preview: false });
    }
}
