"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const completion_1 = require("./completion");
const builtins_1 = require("./builtins");
const signature_1 = require("./signature");
const codelens_1 = require("./codelens");
const lint_1 = require("./lint");
const compile_1 = require("./compile");
const fix_1 = require("./fix");
const hover_1 = require("./hover");
const docs_1 = require("./docs");
const lspclient_1 = require("./lspclient");
function activate(context) {
    const selector = { language: 'ocara', scheme: 'file' };
    // Serveur de langage du compilateur : diagnostics en direct, définition,
    // survol des noms, symboles du document.
    void (0, lspclient_1.startLanguageServer)(context);
    // Autocomplétion : méthodes/constantes des classes builtin (ocara.*) et
    // des classes utilisateur, déclenchée après `.` et `:` (pour `::`).
    (0, builtins_1.loadBuiltins)(context.extensionPath);
    context.subscriptions.push(vscode.languages.registerCompletionItemProvider(selector, new completion_1.OcaraCompletionProvider(), '.', ':'));
    // Signature help (paramètre actif résolu par nom pour un argument nommé).
    context.subscriptions.push(vscode.languages.registerSignatureHelpProvider(selector, new signature_1.OcaraSignatureHelpProvider(), '(', ','));
    // CodeLens implémentations/overrides/références, sur l'index du workspace.
    const index = new codelens_1.WorkspaceIndex();
    index.watch(context);
    void index.build();
    context.subscriptions.push(vscode.languages.registerCodeLensProvider(selector, new codelens_1.OcaraCodeLensProvider(index)));
    // Documentation embarquée (copie de docs/, ouverte en aperçu depuis le survol).
    (0, docs_1.registerDocs)(context);
    // Documentation des mots-clés au survol (le reste vient du serveur).
    context.subscriptions.push(vscode.languages.registerHoverProvider(selector, new hover_1.OcaraKeywordHoverProvider()));
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