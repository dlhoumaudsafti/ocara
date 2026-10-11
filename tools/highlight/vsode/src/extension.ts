import * as vscode from 'vscode';
import { OcaracsLinter } from './lint';
import { OcaraCompiler } from './compile';
import { registerFix } from './fix';
import { registerDocs } from './docs';
import { startLanguageServer, stopLanguageServer } from './lspclient';
import { WorkflowProvider } from './workflow';
import { BenchmarkProvider } from './benchmark';
import { ControlsProvider } from './controls';
import { GraphProvider } from './graph';

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

    // Barre latérale Ocara : configuration et commandes, workflow, graphe
    // d'appels et benchmark du script affiché.
    new ControlsProvider().register(context);
    const workflow = new WorkflowProvider(context.extensionUri);
    workflow.register(context);
    new GraphProvider(context.extensionUri).register(context, workflow.onData);
    new BenchmarkProvider(context.workspaceState).register(context);
}

export function deactivate(): Thenable<void> | undefined {
    return stopLanguageServer();
}
