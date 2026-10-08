"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const lint_1 = require("./lint");
const compile_1 = require("./compile");
const fix_1 = require("./fix");
const docs_1 = require("./docs");
const lspclient_1 = require("./lspclient");
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
}
function deactivate() {
    return (0, lspclient_1.stopLanguageServer)();
}
//# sourceMappingURL=extension.js.map