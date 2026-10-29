"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const lint_1 = require("./lint");
const compile_1 = require("./compile");
const fix_1 = require("./fix");
const docs_1 = require("./docs");
const lspclient_1 = require("./lspclient");
const workflow_1 = require("./workflow");
const benchmark_1 = require("./benchmark");
const controls_1 = require("./controls");
const graph_1 = require("./graph");
const navigation_1 = require("./navigation");
const coverage_1 = require("./coverage");
const quality_1 = require("./quality");
const linterview_1 = require("./linterview");
const architecture_1 = require("./architecture");
const filestatus_1 = require("./filestatus");
const designer_1 = require("./uidesigner/designer");
function activate(context) {
    // Serveur de langage du compilateur : diagnostics en direct, définition,
    // survol des noms, symboles du document, complétion, aide à la signature,
    // références et CodeLens.
    void (0, lspclient_1.startLanguageServer)(context);
    // Documentation embarquée (copie de docs/, ouverte en aperçu depuis le survol).
    (0, docs_1.registerDocs)(context);
    // Analyse ocaracs automatique + commandes Compiler / Compiler et lancer /
    // Afficher le dump / Fixer la mise en forme.
    const linter = new lint_1.OcaracsLinter();
    context.subscriptions.push(linter);
    new compile_1.OcaraCompiler().register(context);
    (0, fix_1.registerFix)(context, linter);
    // Barre latérale Ocara : configuration et commandes, workflow, graphe
    // d'appels, benchmark du script affiché et couverture de l'espace de travail.
    const navigation = new navigation_1.Navigation();
    navigation.register(context);
    // Encarts Coverage (ocaraunit), Linter (ocaracs) et Architecture
    // (--check-architecture), voyants de qualité release.
    (0, filestatus_1.registerFileStatus)(context);
    const coverage = new coverage_1.CoverageProvider();
    coverage.register(context);
    const linterView = new linterview_1.LinterProvider();
    linterView.register(context);
    const architecture = new architecture_1.ArchitectureProvider();
    architecture.register(context);
    const quality = new quality_1.QualityMonitor(coverage, linterView, architecture);
    quality.register(context);
    new controls_1.ControlsProvider(navigation, quality).register(context);
    const workflow = new workflow_1.WorkflowProvider(context.extensionUri);
    workflow.register(context);
    new graph_1.GraphProvider(context.extensionUri).register(context, workflow.onData);
    new benchmark_1.BenchmarkProvider(context.workspaceState).register(context);
    // Designer des fichiers .oc.ui (bouton source ↔ designer en haut à droite).
    new designer_1.UiDesigner(context.extensionUri).register(context);
}
function deactivate() {
    return (0, lspclient_1.stopLanguageServer)();
}
//# sourceMappingURL=extension.js.map