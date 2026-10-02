import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { execFile } from 'child_process';

// ─────────────────────────────────────────────────────────────────────────────
// Lancement des outils en ligne de commande (`ocara`, `ocaracs`) — commande
// configurable par l'utilisateur (`ocara.compilerPath`/`ocara.ocaracsPath`) :
// un chemin (`/usr/local/bin/ocara`) ou une commande avec arguments
// (`"${workspaceFolder}/target/release/ocara" --src src`). Voir
// docs/roadmap.d/tooling-vscode-lint-compile-dump.md.
// ─────────────────────────────────────────────────────────────────────────────

export interface ToolResult {
    stdout: string;
    stderr: string;
    /** Code de sortie (`null` si le processus n'a pas pu démarrer). */
    code: number | null;
    /** Erreur de lancement (commande introuvable...), pas un code non nul. */
    launchError?: string;
}

/** Découpe une commande en programme + arguments (guillemets simples/doubles respectés). */
function splitCommand(command: string): string[] {
    const parts: string[] = [];
    const re = /"([^"]*)"|'([^']*)'|(\S+)/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(command)) !== null) {
        parts.push(m[1] ?? m[2] ?? m[3]);
    }
    return parts;
}

function substituteVariables(value: string, document?: vscode.Uri): string {
    const folder = (document && vscode.workspace.getWorkspaceFolder(document)) ?? vscode.workspace.workspaceFolders?.[0];
    return value
        .replace(/\$\{workspaceFolder\}/g, folder?.uri.fsPath ?? '')
        .replace(/\$\{fileDirname\}/g, document ? path.dirname(document.fsPath) : '');
}

/**
 * Commande effective d'un outil. Réglage laissé à sa valeur par défaut (nom
 * nu, ex. `ocara`) et introuvable dans le PATH : repli sur
 * `<workspace>/target/release/<outil>` s'il existe (dépôt du compilateur).
 */
function resolveCommand(setting: string, defaultName: string, document?: vscode.Uri): string[] {
    const configured = vscode.workspace.getConfiguration('ocara').get<string>(setting, defaultName).trim() || defaultName;
    const parts = splitCommand(substituteVariables(configured, document));
    if (parts.length === 1 && parts[0] === defaultName && !isOnPath(defaultName)) {
        for (const folder of vscode.workspace.workspaceFolders ?? []) {
            const local = path.join(folder.uri.fsPath, 'target', 'release', defaultName);
            if (fs.existsSync(local)) { return [local]; }
        }
    }
    return parts;
}

function isOnPath(name: string): boolean {
    return (process.env.PATH ?? '').split(path.delimiter).some(dir => dir && fs.existsSync(path.join(dir, name)));
}

let output: vscode.OutputChannel | undefined;

/** Canal de sortie « Ocara », partagé par la compilation et `--fix`. */
export function ocaraOutput(): vscode.OutputChannel {
    output ??= vscode.window.createOutputChannel('Ocara');
    return output;
}

/** Lance l'outil configuré par `setting` avec `args`, depuis `cwd`. */
export function runTool(setting: string, defaultName: string, args: string[], cwd: string, document?: vscode.Uri): Promise<ToolResult> {
    const [program, ...baseArgs] = resolveCommand(setting, defaultName, document);
    return new Promise(resolve => {
        execFile(program, [...baseArgs, ...args], { cwd, maxBuffer: 64 * 1024 * 1024 }, (error, stdout, stderr) => {
            const errno = error as NodeJS.ErrnoException | null;
            if (errno && typeof errno.code === 'string') {
                resolve({ stdout, stderr, code: null, launchError: `${program} : ${errno.message}` });
                return;
            }
            resolve({ stdout, stderr, code: error ? (error as { code?: number }).code ?? 1 : 0 });
        });
    });
}

/** `fichier:ligne:col: (warning|error): message` → diagnostic. */
export interface ToolMessage {
    file: string;
    line: number;
    col: number;
    severity: 'warning' | 'error';
    message: string;
}

export function parseToolMessages(output: string, cwd: string): ToolMessage[] {
    const messages: ToolMessage[] = [];
    const clean = output.replace(/\x1b\[[0-9;]*m/g, '');
    const re = /^(.+?):(\d+):(\d+): (warning|error): (.*)$/;
    for (const line of clean.split('\n')) {
        const m = line.match(re);
        if (!m) { continue; }
        messages.push({
            file: path.resolve(cwd, m[1]),
            line: Math.max(0, parseInt(m[2], 10) - 1),
            col: Math.max(0, parseInt(m[3], 10) - 1),
            severity: m[4] as 'warning' | 'error',
            message: m[5],
        });
    }
    return messages;
}

/** Diagnostic couvrant toute la ligne `msg.line` (surlignage de la ligne concernée). */
export function toDiagnostic(document: vscode.TextDocument | undefined, msg: ToolMessage, source: string): vscode.Diagnostic {
    const lineCount = document?.lineCount ?? msg.line + 1;
    const line = Math.min(msg.line, Math.max(0, lineCount - 1));
    const end = document ? document.lineAt(line).range.end.character : msg.col + 1;
    const range = new vscode.Range(line, 0, line, Math.max(end, 1));
    const severity = msg.severity === 'error' ? vscode.DiagnosticSeverity.Error : vscode.DiagnosticSeverity.Warning;
    const diagnostic = new vscode.Diagnostic(range, msg.message, severity);
    diagnostic.source = source;
    return diagnostic;
}
