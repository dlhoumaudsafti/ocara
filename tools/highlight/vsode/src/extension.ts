import * as vscode from 'vscode';
import { OcaracsLinter } from './lint';
import { OcaraCompiler } from './compile';
import { registerFix } from './fix';
import { registerDocs } from './docs';
import { startLanguageServer, stopLanguageServer } from './lspclient';

export function activate(context: vscode.ExtensionContext): void {
    // Serveur de langage du compilateur : diagnostics en direct, définition,
    // survol des noms, symboles du document, complétion, aide à la signature,
    // références et CodeLens.
    void startLanguageServer(context);

    // Documentation embarquée (copie de docs/, ouverte en aperçu depuis le survol).
    registerDocs(context);

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
