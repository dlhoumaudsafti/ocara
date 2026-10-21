import * as vscode from 'vscode';
import * as path from 'path';

// ─────────────────────────────────────────────────────────────────────────────
// Projets testés de l'espace de travail : un dossier `tests` contenant des
// `*Test.oc`, dont le parent est la racine du projet (même règle
// qu'ocaraunit). Réglage `ocara.coverage.projects` : dossiers de tests
// explicites ; `ocara.coverage.exclude` : dossiers ou fichiers ignorés,
// comparés par segments de chemin (`core` exclut `core/…` et `a/core/…`).
// ─────────────────────────────────────────────────────────────────────────────

export interface Project {
    root: string;
    testsDir: string;
}

const IGNORED = '{**/node_modules/**,**/target/**,**/.ocaraunit_cache/**,**/.git/**}';

export function coverageConfig(): vscode.WorkspaceConfiguration {
    return vscode.workspace.getConfiguration('ocara.coverage');
}

export function exclusions(): string[] {
    return coverageConfig().get<string[]>('exclude', []).map(e => e.trim()).filter(e => e.length > 0);
}

function segments(p: string): string[] {
    return p.split(/[\\/]+/).filter(s => s.length > 0 && s !== '.');
}

/** `file` (relatif à `base`) contient-il l'un des motifs, segment par segment ? */
export function isExcluded(file: string, base: string, patterns: string[]): boolean {
    const rel = segments(path.relative(base, file));
    return patterns.some(pattern => {
        const parts = segments(pattern);
        if (parts.length === 0) { return false; }
        for (let i = 0; i + parts.length <= rel.length; i++) {
            if (parts.every((p, j) => rel[i + j] === p)) { return true; }
        }
        return false;
    });
}

function folderOf(file: string): vscode.WorkspaceFolder | undefined {
    return vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file));
}

/** Dossier `tests` le plus haut contenant `file`. */
function testsDirOf(file: string): string | undefined {
    let found: string | undefined;
    for (let dir = path.dirname(file); dir !== path.dirname(dir); dir = path.dirname(dir)) {
        if (path.basename(dir) === 'tests') { found = dir; }
        if (folderOf(dir)?.uri.fsPath === dir) { break; }
    }
    return found;
}

export async function findProjects(): Promise<Project[]> {
    const patterns = exclusions();
    const explicit = coverageConfig().get<string[]>('projects', []).filter(p => p.trim().length > 0);
    const dirs = new Set<string>();
    if (explicit.length > 0) {
        const base = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '';
        explicit.forEach(p => dirs.add(path.resolve(base, p)));
    } else {
        for (const uri of await vscode.workspace.findFiles('**/tests/**/*Test.oc', IGNORED)) {
            const dir = testsDirOf(uri.fsPath);
            const folder = folderOf(uri.fsPath);
            if (dir && folder && !isExcluded(dir, folder.uri.fsPath, patterns)) { dirs.add(dir); }
        }
    }
    return [...dirs].sort().map(testsDir => ({ root: path.dirname(testsDir), testsDir }));
}

/** Dossiers analysés par le linter et l'architecture : racines des projets
 *  testés, sinon dossiers de l'espace de travail. */
export async function qualityRoots(): Promise<string[]> {
    const projects = await findProjects();
    return projects.length > 0
        ? projects.map(p => p.root)
        : (vscode.workspace.workspaceFolders ?? []).map(f => f.uri.fsPath);
}

/** Ouvre `file` et place le curseur ligne `line` (1-basée), colonne `col`
 *  (1-basée ; absente : début du code de la ligne). */
export async function gotoLine(file: string, line: number, col?: number): Promise<void> {
    const document = await vscode.workspace.openTextDocument(file);
    const editor = await vscode.window.showTextDocument(document);
    const target = Math.max(0, Math.min(line - 1, document.lineCount - 1));
    const character = col !== undefined ? Math.max(0, col - 1) : document.lineAt(target).firstNonWhitespaceCharacterIndex;
    const position = new vscode.Position(target, character);
    editor.selection = new vscode.Selection(position, position);
    editor.revealRange(new vscode.Range(position, position), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
}

/** Exclusions d'une section (`ocara.<section>.exclude`), comparées par
 *  segments de chemin depuis chacune des `bases` (racine analysée, dossier
 *  de l'espace de travail). */
export function isExcludedFrom(file: string, bases: string[], patterns: string[]): boolean {
    const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(file))?.uri.fsPath;
    return [...bases, ...(folder ? [folder] : [])].some(base => isExcluded(file, base, patterns));
}

export function sectionExclusions(section: string): string[] {
    return vscode.workspace.getConfiguration(`ocara.${section}`).get<string[]>('exclude', [])
        .map(e => e.trim()).filter(e => e.length > 0);
}

/** Scripts `.oc` d'un dossier (sous-dossiers compris), ou le script lui-même. */
export async function ocFiles(root: string): Promise<string[]> {
    if (root.endsWith('.oc')) { return [root]; }
    return (await vscode.workspace.findFiles(new vscode.RelativePattern(root, '**/*.oc'), IGNORED))
        .map(uri => uri.fsPath).filter(f => !path.basename(f).startsWith('__ocaraunit__'));
}
