import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

// ─────────────────────────────────────────────────────────────────────────────
// Contexte d'un fichier runtime (`runtime core.main is main` dans un
// programme) : les blocs runtime d'un programme partagent leur portée — une
// variable déclarée dans `core/init.runtime.oc` (`const server:Server = ...`)
// est utilisée dans `core/main.runtime.oc`, et la classe `Server` n'est
// importée que par le fichier principal. Pour naviguer/compléter depuis un
// fichier runtime, chercher aussi dans ce fichier principal et dans les
// autres fichiers runtime qu'il déclare.
// ─────────────────────────────────────────────────────────────────────────────

/** Extensions essayées pour `runtime chemin.vers.fichier`, dans l'ordre du compilateur. */
export const RUNTIME_EXTENSIONS = ['.runtime.oc', '.run.oc', '.rt.oc', '.oc'];

const RUNTIME_LINE_RE = /^\s*runtime\s+([\w.]+)/gm;

/** Fichier ciblé par `runtime <runtimePath>` écrit dans un fichier du dossier `fromDir`. */
export function resolveRuntimeFile(fromDir: string, runtimePath: string): string | undefined {
    const rel = runtimePath.split('.').join(path.sep);
    for (const ext of RUNTIME_EXTENSIONS) {
        const candidate = path.resolve(fromDir, rel + ext);
        if (fs.existsSync(candidate)) { return candidate; }
    }
    return undefined;
}

function runtimeFilesOf(programFile: string, text: string): string[] {
    const dir = path.dirname(programFile);
    return [...text.matchAll(RUNTIME_LINE_RE)]
        .map(m => resolveRuntimeFile(dir, m[1]))
        .filter((f): f is string => f !== undefined);
}

/**
 * Documents partageant la portée de `document` s'il est un fichier runtime :
 * programme(s) qui le déclarent, puis leurs autres fichiers runtime. Vide
 * pour un fichier qui n'est le runtime d'aucun programme du workspace.
 */
export async function runtimeContext(document: vscode.TextDocument): Promise<vscode.TextDocument[]> {
    const self = document.uri.fsPath;
    const related = new Set<string>();
    for (const uri of await vscode.workspace.findFiles('**/*.oc', '{**/node_modules/**,**/out/**,**/target/**}')) {
        if (uri.fsPath === self) { continue; }
        let text: string;
        try {
            text = fs.readFileSync(uri.fsPath, 'utf8');
        } catch {
            continue;
        }
        if (!text.includes('runtime ')) { continue; }
        const runtimes = runtimeFilesOf(uri.fsPath, text);
        if (!runtimes.includes(self)) { continue; }
        related.add(uri.fsPath);
        runtimes.filter(f => f !== self).forEach(f => related.add(f));
    }
    const docs: vscode.TextDocument[] = [];
    for (const file of related) {
        try {
            docs.push(await vscode.workspace.openTextDocument(vscode.Uri.file(file)));
        } catch {
            // fichier illisible : ignoré
        }
    }
    return docs;
}
