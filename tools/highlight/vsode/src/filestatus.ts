import * as vscode from 'vscode';
import * as path from 'path';

// ─────────────────────────────────────────────────────────────────────────────
// Couleur des fichiers listés dans les encarts Linter, Coverage et
// Architecture : vert (conforme : aucun avertissement, entièrement couvert,
// architecture respectée), rouge (avertissement, lignes non couvertes,
// erreur), gris (ignoré : exclu par la configuration, ou hors du champ de
// l'analyse). Décoration d'un schéma d'URI propre aux encarts : l'explorateur
// de fichiers n'est pas coloré.
// ─────────────────────────────────────────────────────────────────────────────

export type FileStatus = 'ok' | 'error' | 'ignored';

const SCHEME = 'ocara-status';

const DECORATIONS: Record<FileStatus, vscode.FileDecoration> = {
    ok: { color: new vscode.ThemeColor('testing.iconPassed'), tooltip: 'Conforme' },
    error: { color: new vscode.ThemeColor('testing.iconFailed'), tooltip: 'À corriger' },
    ignored: { color: new vscode.ThemeColor('disabledForeground'), tooltip: 'Ignoré' },
};

export function registerFileStatus(context: vscode.ExtensionContext): void {
    context.subscriptions.push(vscode.window.registerFileDecorationProvider({
        provideFileDecoration: uri => uri.scheme === SCHEME ? DECORATIONS[uri.query as FileStatus] : undefined,
    }));
}

/** Élément d'arbre d'un fichier coloré selon `status` (icône du thème de fichiers conservée). */
export function fileItem(file: string, status: FileStatus, collapsible: vscode.TreeItemCollapsibleState): vscode.TreeItem {
    const t = new vscode.TreeItem(path.basename(file), collapsible);
    t.resourceUri = vscode.Uri.file(file).with({ scheme: SCHEME, query: status });
    t.iconPath = status === 'ok' ? new vscode.ThemeIcon('pass-filled', new vscode.ThemeColor('testing.iconPassed'))
        : status === 'ignored' ? new vscode.ThemeIcon('circle-slash', new vscode.ThemeColor('disabledForeground'))
        : vscode.ThemeIcon.File;
    t.tooltip = file;
    return t;
}

/** Ordre d'affichage : à corriger, puis conformes, puis ignorés. */
export function statusOrder(status: FileStatus): number {
    return status === 'error' ? 0 : status === 'ok' ? 1 : 2;
}
