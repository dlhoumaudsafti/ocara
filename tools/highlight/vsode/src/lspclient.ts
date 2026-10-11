import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';
import { resolveCommand } from './toolrunner';
import { docLink, rewriteDocLinks } from './docs';

// ─────────────────────────────────────────────────────────────────────────────
// Client du serveur de langage `ocara --lsp` : diagnostics en direct,
// définition, survol, symboles, complétion, signature, références et CodeLens
// viennent du compilateur lui-même (voir docs/roadmap.d/tooling-language-server.md).
// ─────────────────────────────────────────────────────────────────────────────

let client: LanguageClient | undefined;

export async function startLanguageServer(context: vscode.ExtensionContext): Promise<void> {
    const [command, ...args] = resolveCommand('compilerPath', 'ocara');
    const serverOptions: ServerOptions = { command, args: [...args, '--lsp'] };
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ language: 'ocara', scheme: 'file' }],
        initializationOptions: buildSettings(),
        // Fichiers `.oc` créés, modifiés ou supprimés hors de l'éditeur.
        synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher('**/*.oc') },
        middleware: {
            provideHover: async (document, position, token, next) => {
                const hover = await next(document, position, token);
                return hover ? new vscode.Hover(hover.contents.map(trustDocLinks), hover.range) : hover;
            },
        },
    };
    client = new LanguageClient('ocara', 'Ocara (serveur de langage)', serverOptions, clientOptions);
    context.subscriptions.push(
        client,
        vscode.commands.registerCommand('ocara.showReferences', showReferences),
        vscode.workspace.onWillSaveTextDocument(e => {
            if (e.document.languageId === 'ocara' && vscode.workspace.getConfiguration('ocara').get<boolean>('autoImport.enable', true)) {
                e.waitUntil(missingImportEdits(e.document));
            }
        }),
        vscode.workspace.onDidChangeConfiguration(e => {
            if (client && ['ocara.os', 'ocara.osOther', 'ocara.arch', 'ocara.archOther', 'ocara.build'].some(s => e.affectsConfiguration(s))) {
                void client.sendNotification('workspace/didChangeConfiguration', { settings: buildSettings() });
            }
        }),
    );
    try {
        await client.start();
    } catch (err) {
        vscode.window.showErrorMessage(`Ocara : impossible de démarrer le serveur de langage (${err}) — réglage « ocara.compilerPath ».`);
    }
}

/** CodeLens du serveur : arguments LSP (uri, position, emplacements) → panneau de références. */
function showReferences(uri: string, position: unknown, locations: unknown[]): Thenable<unknown> | undefined {
    if (!client) { return undefined; }
    const p2c = client.protocol2CodeConverter;
    return vscode.commands.executeCommand('editor.action.showReferences',
        p2c.asUri(uri), p2c.asPosition(position as never), locations.map(l => p2c.asLocation(l as never)));
}

/** Imports manquants du document (action `source.addMissingImports` du serveur). */
async function missingImportEdits(document: vscode.TextDocument): Promise<vscode.TextEdit[]> {
    if (!client) { return []; }
    const all = new vscode.Range(0, 0, document.lineCount, 0);
    const actions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
        'vscode.executeCodeActionProvider', document.uri, all, 'source.addMissingImports');
    const edit = actions?.find(a => a.kind?.value === 'source.addMissingImports')?.edit;
    return edit?.get(document.uri) ?? [];
}

/** Configuration active de l'éditeur pour les clauses `when` : `system`
 *  (absent) = valeur de la machine, `other` = valeur personnalisée ; mode
 *  `debug` ou `release`. */
function buildSettings(): { os?: string; arch?: string; build: string } {
    const config = vscode.workspace.getConfiguration('ocara');
    const pick = (key: string): string | undefined => {
        const value = config.get<string>(key, 'system');
        if (value === 'system') { return undefined; }
        return value === 'other' ? (config.get<string>(`${key}Other`, '').trim() || undefined) : value;
    };
    return { os: pick('os'), arch: pick('arch'), build: config.get<string>('build', 'debug') };
}

export function stopLanguageServer(): Thenable<void> | undefined {
    return client?.stop();
}

/** Liens `ocara-doc:<chemin>#<titre>` du serveur → ouverture de la documentation embarquée. */
function trustDocLinks(content: vscode.MarkdownString | vscode.MarkedString): vscode.MarkdownString {
    const text = typeof content === 'string' ? content : 'value' in content && !('language' in content) ? content.value : '```' + content.language + '\n' + content.value + '\n```';
    const rewritten = text.replace(/\[([^\]]+)\]\(ocara-doc:([^)#]+)(?:#([^)]*))?\)/g,
        (_all, label: string, file: string, heading?: string) => docLink(label, file, heading ? decodeURIComponent(heading) : undefined));
    const md = new vscode.MarkdownString(rewriteDocLinks(rewritten, 'builtins'));
    md.isTrusted = { enabledCommands: ['ocara.openDoc'] };
    return md;
}
