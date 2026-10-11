import * as vscode from 'vscode';
import * as path from 'path';

// ─────────────────────────────────────────────────────────────────────────────
// Historique de navigation de la barre latérale Ocara : chaque ouverture
// depuis le panneau (arbre Workflow, graphe d'appels) s'y ajoute, avec la
// position de départ ; ← / → (en-tête du panneau et barre de titre de
// l'éditeur) y circulent. Ouvrir un autre fichier à la main efface
// l'historique : la barre de navigation disparaît.
// ─────────────────────────────────────────────────────────────────────────────

export interface NavEntry { file: string; line: number }

export interface NavState { entries: { label: string; detail: string }[]; index: number }

export class Navigation {
    private entries: NavEntry[] = [];
    private index = -1;
    private readonly changed = new vscode.EventEmitter<NavState>();
    readonly onChange = this.changed.event;

    register(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
            vscode.commands.registerCommand('ocara.nav.open', (file: string, line: number) => this.navigate(file, line)),
            vscode.commands.registerCommand('ocara.nav.back', () => this.go(this.index - 1)),
            vscode.commands.registerCommand('ocara.nav.forward', () => this.go(this.index + 1)),
            vscode.commands.registerCommand('ocara.nav.go', (index: number) => this.go(index)),
            vscode.commands.registerCommand('ocara.nav.clear', () => this.clear()),
            vscode.window.onDidChangeActiveTextEditor(editor => this.onEditor(editor)),
        );
        this.publish();
    }

    state(): NavState {
        return {
            entries: this.entries.map(e => ({ label: `${path.basename(e.file)}:${e.line}`, detail: vscode.workspace.asRelativePath(e.file) })),
            index: this.index,
        };
    }

    /** Ouverture depuis le panneau : la position courante devient le point
     *  de départ, les entrées « suivantes » sont abandonnées. */
    async navigate(file: string, line: number): Promise<void> {
        const here = current();
        if (this.index < 0 && here) {
            this.entries = [here];
            this.index = 0;
        } else if (here && this.entries[this.index]?.file === here.file) {
            this.entries[this.index] = here;
        }
        this.entries = this.entries.slice(0, this.index + 1);
        const last = this.entries[this.index];
        if (!last || last.file !== file || last.line !== line) {
            this.entries.push({ file, line });
            this.index = this.entries.length - 1;
        }
        await this.open(this.entries[this.index]);
    }

    private async go(index: number): Promise<void> {
        if (index < 0 || index >= this.entries.length) { return; }
        const here = current();
        if (here && this.entries[this.index]?.file === here.file) { this.entries[this.index] = here; }
        this.index = index;
        await this.open(this.entries[index]);
    }

    private async open(entry: NavEntry): Promise<void> {
        const position = new vscode.Position(Math.max(entry.line - 1, 0), 0);
        this.publish();
        await vscode.window.showTextDocument(vscode.Uri.file(entry.file), { selection: new vscode.Range(position, position) });
    }

    /** Un éditeur affiché hors de l'historique : ouverture manuelle. */
    private onEditor(editor: vscode.TextEditor | undefined): void {
        if (!editor || this.index < 0 || editor.document.uri.scheme !== 'file') { return; }
        if (editor.document.uri.fsPath !== this.entries[this.index]?.file) { this.clear(); }
    }

    private clear(): void {
        this.entries = [];
        this.index = -1;
        this.publish();
    }

    private publish(): void {
        void vscode.commands.executeCommand('setContext', 'ocara.nav.active', this.entries.length > 1);
        void vscode.commands.executeCommand('setContext', 'ocara.nav.canBack', this.index > 0);
        void vscode.commands.executeCommand('setContext', 'ocara.nav.canForward', this.index >= 0 && this.index < this.entries.length - 1);
        this.changed.fire(this.state());
    }
}

function current(): NavEntry | undefined {
    const editor = vscode.window.activeTextEditor;
    if (!editor || editor.document.uri.scheme !== 'file') { return undefined; }
    return { file: editor.document.uri.fsPath, line: editor.selection.active.line + 1 };
}
