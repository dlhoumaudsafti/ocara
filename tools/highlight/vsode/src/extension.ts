import * as vscode from 'vscode';
import { OcaraCompletionProvider } from './completion';
import { loadBuiltins } from './builtins';
import { OcaraSignatureHelpProvider } from './signature';
import { OcaraCodeLensProvider, WorkspaceIndex } from './codelens';
import { OcaracsLinter } from './lint';
import { OcaraCompiler } from './compile';
import { registerFix } from './fix';
import { OcaraKeywordHoverProvider } from './hover';
import { registerDocs } from './docs';
import { startLanguageServer, stopLanguageServer } from './lspclient';

export function activate(context: vscode.ExtensionContext): void {
    const selector: vscode.DocumentSelector = { language: 'ocara', scheme: 'file' };

    // Serveur de langage du compilateur : diagnostics en direct, définition,
    // survol des noms, symboles du document.
    void startLanguageServer(context);

    // Autocomplétion : méthodes/constantes des classes builtin (ocara.*) et
    // des classes utilisateur, déclenchée après `.` et `:` (pour `::`).
    loadBuiltins(context.extensionPath);
    context.subscriptions.push(
        vscode.languages.registerCompletionItemProvider(selector, new OcaraCompletionProvider(), '.', ':')
    );

    // Signature help (paramètre actif résolu par nom pour un argument nommé).
    context.subscriptions.push(
        vscode.languages.registerSignatureHelpProvider(selector, new OcaraSignatureHelpProvider(), '(', ',')
    );

    // CodeLens implémentations/overrides/références, sur l'index du workspace.
    const index = new WorkspaceIndex();
    index.watch(context);
    void index.build();
    context.subscriptions.push(
        vscode.languages.registerCodeLensProvider(selector, new OcaraCodeLensProvider(index))
    );

    // Documentation embarquée (copie de docs/, ouverte en aperçu depuis le survol).
    registerDocs(context);

    // Documentation des mots-clés au survol (le reste vient du serveur).
    context.subscriptions.push(vscode.languages.registerHoverProvider(selector, new OcaraKeywordHoverProvider()));

    // Analyse ocaracs automatique + commandes Compiler / Compiler et lancer /
    // Afficher le dump / Fixer la mise en forme.
    const linter = new OcaracsLinter();
    context.subscriptions.push(linter);
    new OcaraCompiler().register(context);
    registerFix(context, linter);
}

export function deactivate(): Thenable<void> | undefined {
    return stopLanguageServer();
}
