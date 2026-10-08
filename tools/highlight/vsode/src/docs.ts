import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

// ─────────────────────────────────────────────────────────────────────────────
// Documentation embarquée : `docs/EBNF.md` et `docs/builtins/*.md` sont
// copiés dans l'extension à chaque compilation (scripts/copy-docs.js) — le
// survol y renvoie par des liens `command:ocara.openDoc`, qui ouvrent le
// fichier en APERÇU Markdown (jamais en édition), sur la bonne section.
// ─────────────────────────────────────────────────────────────────────────────

let docsRoot = '';

export const OPEN_DOC_COMMAND = 'ocara.openDoc';

export function registerDocs(context: vscode.ExtensionContext): void {
    docsRoot = path.join(context.extensionPath, 'docs');
    context.subscriptions.push(vscode.commands.registerCommand(OPEN_DOC_COMMAND, openDoc));
}

/** Ouvre `docs/<relPath>` de l'extension en aperçu, positionné sur `heading` (titre brut) si fourni. */
async function openDoc(relPath: string, heading?: string): Promise<void> {
    const file = path.join(docsRoot, relPath);
    if (!fs.existsSync(file)) {
        vscode.window.showWarningMessage(`Ocara : documentation introuvable dans l'extension (${relPath}) — recompiler l'extension.`);
        return;
    }
    const uri = vscode.Uri.file(file).with({ fragment: heading ? slugify(heading) : '' });
    await vscode.commands.executeCommand('markdown.showPreview', uri);
}

/**
 * Ancre d'un titre, même règle que l'aperçu Markdown de VS Code (« github
 * slugifier ») appliquée au texte RENDU du titre (backticks/emphase retirés).
 */
export function slugify(heading: string): string {
    return heading
        .replace(/[`*]/g, '')
        .trim()
        .toLowerCase()
        .replace(/\s+/g, '-')
        .replace(/[\]\[!/'"#$%&()*+,.:;<=>?@\\^{|}~`。，、；：？！…—·ˉ¨‘’“”々～‖∶＂＇｀｜〃〔〕〈〉《》「」『』．〖〗【】（）［］｛｝]/g, '')
        .replace(/^-+/, '')
        .replace(/-+$/, '');
}

/** Lien Markdown (survol) ouvrant `docs/<relPath>` en aperçu. */
export function docLink(label: string, relPath: string, heading?: string): string {
    const args = encodeURIComponent(JSON.stringify(heading ? [relPath, heading] : [relPath]));
    return `[${label}](command:${OPEN_DOC_COMMAND}?${args})`;
}

/**
 * Liens relatifs d'un extrait de `docs/builtins/<fichier>` (`[Regex](Regex.md)`,
 * `[EBNF](../EBNF.md#...)`) → liens `ocara.openDoc` vers la copie embarquée ;
 * les autres liens relatifs (README...) n'existent pas dans l'extension :
 * réduits à leur texte.
 */
export function rewriteDocLinks(markdown: string, fromDir: 'builtins' | ''): string {
    return markdown.replace(/\[([^\]]+)\]\((?!https?:|command:|#)([^)#\s]+)(#[^)\s]*)?\)/g, (_all, label: string, target: string) => {
        const rel = path.posix.normalize(path.posix.join(fromDir, target));
        const known = rel === 'EBNF.md' || /^builtins\/[^/]+\.md$/.test(rel);
        return known ? docLink(label, rel) : label;
    });
}
