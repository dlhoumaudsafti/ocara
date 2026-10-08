import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';
import { resolveCommand } from './toolrunner';
import { docLink } from './docs';

// ─────────────────────────────────────────────────────────────────────────────
// Client du serveur de langage `ocara --lsp` : diagnostics en direct,
// définition, survol et symboles du document viennent du compilateur
// lui-même (voir docs/roadmap.d/tooling-language-server.md).
// ─────────────────────────────────────────────────────────────────────────────

let client: LanguageClient | undefined;

export async function startLanguageServer(context: vscode.ExtensionContext): Promise<void> {
    const [command, ...args] = resolveCommand('compilerPath', 'ocara');
    const serverOptions: ServerOptions = { command, args: [...args, '--lsp'] };
    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ language: 'ocara', scheme: 'file' }],
        middleware: {
            provideHover: async (document, position, token, next) => {
                const hover = await next(document, position, token);
                return hover ? new vscode.Hover(hover.contents.map(trustDocLinks), hover.range) : hover;
            },
        },
    };
    client = new LanguageClient('ocara', 'Ocara (serveur de langage)', serverOptions, clientOptions);
    context.subscriptions.push(client);
    try {
        await client.start();
    } catch (err) {
        vscode.window.showErrorMessage(`Ocara : impossible de démarrer le serveur de langage (${err}) — réglage « ocara.compilerPath ».`);
    }
}

export function stopLanguageServer(): Thenable<void> | undefined {
    return client?.stop();
}

/** Liens `ocara-doc:<chemin>#<titre>` du serveur → ouverture de la documentation embarquée. */
function trustDocLinks(content: vscode.MarkdownString | vscode.MarkedString): vscode.MarkdownString {
    const text = typeof content === 'string' ? content : 'value' in content && !('language' in content) ? content.value : '```' + content.language + '\n' + content.value + '\n```';
    const rewritten = text.replace(/\[([^\]]+)\]\(ocara-doc:([^)#]+)(?:#([^)]*))?\)/g,
        (_all, label: string, file: string, heading?: string) => docLink(label, file, heading ? decodeURIComponent(heading) : undefined));
    const md = new vscode.MarkdownString(rewritten);
    md.isTrusted = { enabledCommands: ['ocara.openDoc'] };
    return md;
}
