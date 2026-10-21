import * as vscode from 'vscode';
import * as path from 'path';

// ─────────────────────────────────────────────────────────────────────────────
// Périmètre et mode automatique des encarts Linter, Coverage et Architecture
// (réglages `ocara.<section>.scope` et `ocara.<section>.auto`) :
//   - `workspace` : tout l'espace de travail ;
//   - `folder`    : dossier du script affiché, sous-dossiers compris ;
//   - `script`    : script affiché seul.
// Mode automatique (bouton de l'en-tête de l'encart) : en `workspace`, à
// l'ouverture et à chaque enregistrement d'un `.oc` ; en `folder`, à
// l'enregistrement d'un `.oc` du dossier et quand le script affiché change de
// dossier ; en `script`, à l'enregistrement du script et quand le script
// affiché change. Le bouton ▶ lance toujours l'analyse à la demande.
// ─────────────────────────────────────────────────────────────────────────────

export type Scope = 'workspace' | 'folder' | 'script';

/** Ce qu'une analyse a couvert : périmètre, et dossier ou script visé. */
export interface Target {
    scope: Scope;
    folder?: string;
    file?: string;
}

function config(section: string): vscode.WorkspaceConfiguration {
    return vscode.workspace.getConfiguration(`ocara.${section}`);
}

export function readScope(section: string): Scope {
    const value = config(section).get<string>('scope', 'workspace');
    return value === 'folder' || value === 'script' ? value : 'workspace';
}

export function autoEnabled(section: string): boolean {
    return config(section).get<boolean>('auto', true);
}

/** Script Ocara affiché. */
export function activeScript(): string | undefined {
    const doc = vscode.window.activeTextEditor?.document;
    return doc?.languageId === 'ocara' && doc.uri.scheme === 'file' ? doc.uri.fsPath : undefined;
}

/** Cible courante du périmètre de `section` ; `undefined` si elle exige un
 *  script affiché et qu'il n'y en a pas. */
export function currentTarget(section: string): Target | undefined {
    const scope = readScope(section);
    if (scope === 'workspace') { return { scope }; }
    const file = activeScript();
    if (!file) { return undefined; }
    return scope === 'folder' ? { scope, folder: path.dirname(file) } : { scope, file };
}

export function sameTarget(a: Target | undefined, b: Target | undefined): boolean {
    return !!a && !!b && a.scope === b.scope && a.folder === b.folder && a.file === b.file;
}

/** « espace de travail », « dossier context/car », « script CarRepository.oc ». */
export function describeTarget(target: Target | undefined, section?: string): string {
    if (!target) {
        const scope = section ? readScope(section) : 'script';
        return scope === 'folder' ? 'dossier du script affiché (aucun script .oc affiché)' : 'script affiché (aucun script .oc affiché)';
    }
    if (target.scope === 'folder') { return `dossier ${vscode.workspace.asRelativePath(target.folder!)}`; }
    if (target.scope === 'script') { return `script ${vscode.workspace.asRelativePath(target.file!)}`; }
    return 'espace de travail';
}

export function isWithin(child: string, parent: string): boolean {
    const rel = path.relative(parent, child);
    return rel === '' || (!rel.startsWith('..') && !path.isAbsolute(rel));
}

/**
 * Lancements automatiques d'un encart et bouton de l'en-tête qui les active
 * (`ocara.<view>.autoOn` / `autoOff`, clé de contexte `ocara.<view>.auto`).
 */
export class AutoRunner implements vscode.Disposable {
    private timer: NodeJS.Timeout | undefined;
    private readonly disposables: vscode.Disposable[] = [];
    /** Cible de la dernière demande, pour ne relancer qu'au changement de dossier/script. */
    private lastTarget: Target | undefined;

    constructor(
        private readonly section: string,
        private readonly view: string,
        private readonly run: () => void,
        private readonly debounceMs: number,
    ) {}

    register(context: vscode.ExtensionContext): void {
        this.disposables.push(
            vscode.commands.registerCommand(`ocara.${this.view}.autoOn`, () => this.setAuto(true)),
            vscode.commands.registerCommand(`ocara.${this.view}.autoOff`, () => this.setAuto(false)),
            vscode.workspace.onDidSaveTextDocument(doc => this.onSave(doc)),
            vscode.window.onDidChangeActiveTextEditor(() => this.onEditor()),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration(`ocara.${this.section}.auto`) || e.affectsConfiguration(`ocara.${this.section}.scope`)) {
                    this.updateContext();
                    this.lastTarget = undefined;
                    this.schedule();
                }
            }),
        );
        context.subscriptions.push(this);
        this.updateContext();
        this.schedule();
    }

    private updateContext(): void {
        void vscode.commands.executeCommand('setContext', `ocara.${this.view}.auto`, autoEnabled(this.section));
    }

    private async setAuto(on: boolean): Promise<void> {
        const target = vscode.workspace.workspaceFolders ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
        await config(this.section).update('auto', on, target);
    }

    private onSave(doc: vscode.TextDocument): void {
        if (doc.languageId !== 'ocara') { return; }
        const target = currentTarget(this.section);
        if (!target) { return; }
        const relevant = target.scope === 'workspace'
            || (target.scope === 'folder' && isWithin(doc.uri.fsPath, target.folder!))
            || (target.scope === 'script' && doc.uri.fsPath === target.file);
        if (relevant) { this.schedule(true); }
    }

    private onEditor(): void {
        if (readScope(this.section) === 'workspace') { return; }
        if (!sameTarget(currentTarget(this.section), this.lastTarget)) { this.schedule(); }
    }

    /** Lancement automatique regroupé (`force` : même cible qu'avant, contenu modifié). */
    schedule(force = false): void {
        if (!autoEnabled(this.section)) { return; }
        const target = currentTarget(this.section);
        if (!target || (!force && sameTarget(target, this.lastTarget))) { return; }
        this.lastTarget = target;
        clearTimeout(this.timer);
        this.timer = setTimeout(() => this.run(), this.debounceMs);
    }

    dispose(): void {
        clearTimeout(this.timer);
        this.disposables.forEach(d => d.dispose());
    }
}
