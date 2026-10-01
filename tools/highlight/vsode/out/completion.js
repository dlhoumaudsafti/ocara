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
exports.OcaraCompletionProvider = void 0;
const vscode = __importStar(require("vscode"));
const builtins_1 = require("./builtins");
const resolver_1 = require("./resolver");
const callsite_1 = require("./callsite");
const primitives_1 = require("./primitives");
const runtimecontext_1 = require("./runtimecontext");
const hover_1 = require("./hover");
const resolver_2 = require("./resolver");
// ─────────────────────────────────────────────────────────────────────────────
// Autocomplétion : méthodes/constantes des classes builtin `ocara.*` (données
// générées depuis src/builtins/*.rs, voir tools/highlight/vsode/data/) et des
// classes utilisateur (résolues via les imports du document, resolver.ts).
// ─────────────────────────────────────────────────────────────────────────────
/** Propriétés communes à toutes les exceptions builtin (voir src/builtins/exception.rs). */
const EXCEPTION_PROPERTIES = [
    { name: 'message', type: 'string' },
    { name: 'code', type: 'int' },
    { name: 'source', type: 'string' },
];
class OcaraCompletionProvider {
    async provideCompletionItems(document, position, _token, _context) {
        const linePrefix = document.lineAt(position).text.substring(0, position.character);
        // ── ClassName::member  (et self::member / parent::member) ─────────────
        const staticMatch = linePrefix.match(/([A-Za-z_]\w*)::\w*$/);
        if (staticMatch) {
            let className = staticMatch[1];
            if (className === 'self' || className === 'parent') {
                className = (0, resolver_1.findEnclosingClassName)(document, position) ?? className;
            }
            return this.completeStatic(document, className);
        }
        // ── variable.member  (et self.member) ──────────────────────────────────
        const instanceMatch = linePrefix.match(/(?:^|[^.\w])([a-zA-Z_]\w*)\.\w*$/);
        if (instanceMatch) {
            let varName = instanceMatch[1];
            let className;
            if (varName === 'self' || varName === 'parent') {
                className = (0, resolver_1.findEnclosingClassName)(document, position);
            }
            else {
                className = (0, resolver_1.findVariableType)(document, varName);
            }
            // Fichier runtime : variable déclarée dans un autre fichier du
            // même programme (voir runtimecontext.ts).
            const context = className ? [] : await (0, runtimecontext_1.runtimeContext)(document);
            for (const doc of context) {
                className = (0, resolver_1.findVariableType)(doc, varName);
                if (className) {
                    break;
                }
            }
            if (!className) {
                // Type primitif : conversions (`s.toInt()`) et sucre String/Array/Map.
                const primitive = (0, primitives_1.findPrimitiveType)(document, varName);
                return primitive ? (0, primitives_1.instanceMethodsFor)(primitive).map(m => this.primitiveMethodItem(varName, m)) : undefined;
            }
            return this.completeInstance(document, className);
        }
        // ── use ClassName(  — complétion du nom de classe instanciable ────────
        const useMatch = linePrefix.match(/\buse\s+\w*$/);
        if (useMatch) {
            return this.completeClassNames(document);
        }
        // ── f(nom: ...) — noms des paramètres de la cible de l'appel ────────
        if (/(?:^|[(,])\s*\w*$/.test(linePrefix)) {
            const named = await this.completeArgumentNames(document, position);
            if (named) {
                return named;
            }
        }
        // ── Identifiant nu : fonctions libres du programme ──────────────────
        if (/(?:^|[^.:\w])[a-z_]\w*$/.test(linePrefix)) {
            return this.completeFunctions(document);
        }
        return undefined;
    }
    // ─── fonctions libres ───────────────────────────────────────────────────
    /** Fonctions du document, de son contexte runtime et des fichiers importés. */
    async completeFunctions(document) {
        const items = [];
        const seen = new Set();
        for (const f of await collectFunctions(document)) {
            if (seen.has(f.name)) {
                continue;
            }
            seen.add(f.name);
            const item = new vscode.CompletionItem(f.name, vscode.CompletionItemKind.Function);
            item.detail = `function ${f.name}(${f.params}): ${f.returnType}`;
            item.insertText = callSnippet(f.name, paramNames(f.params));
            item.documentation = new vscode.MarkdownString('```ocara\n' + item.detail + '\n```' + (f.comment ? `\n\n${f.comment}` : ''));
            items.push(item);
        }
        return items;
    }
    // ─── f(nom: valeur) ─────────────────────────────────────────────────────
    /**
     * Un appel est soit 100 % positionnel, soit 100 % nommé : rien à proposer
     * dès qu'un argument précédent est positionnel. Variadic et noms déjà
     * fournis exclus (voir docs/roadmap.d/langage-named-arguments.md).
     */
    async completeArgumentNames(document, position) {
        const site = (0, callsite_1.findCallSite)(document, position);
        if (!site || site.positionalCount > 0 || site.currentName !== undefined) {
            return undefined;
        }
        const call = await (0, callsite_1.resolveCall)(document, position, site);
        if (!call) {
            return undefined;
        }
        return (0, callsite_1.remainingNamedParams)(call, site).map((p, i) => {
            const item = new vscode.CompletionItem(`${p.name}:`, vscode.CompletionItemKind.Property);
            item.insertText = `${p.name}: `;
            item.filterText = p.name;
            item.sortText = String(i).padStart(3, '0');
            item.detail = (0, callsite_1.paramLabel)(p);
            item.documentation = new vscode.MarkdownString(`Argument nommé de \`${call.owner}\`${p.defaultValue !== undefined ? ' — optionnel' : ''}`);
            return item;
        });
    }
    // ─── ClassName::membre ──────────────────────────────────────────────────
    async completeStatic(document, className) {
        const builtin = (0, builtins_1.getBuiltinClass)(className);
        if (builtin) {
            const items = [];
            for (const c of builtin.consts) {
                items.push(this.builtinConstItem(className, c, true));
            }
            for (const m of builtin.methods.filter(x => x.static)) {
                items.push(this.builtinMethodItem(className, m, true));
            }
            return items;
        }
        const members = await (0, resolver_1.findClassMembers)(document, className);
        return members.filter(m => m.isStatic).map(m => this.memberItem(className, m));
    }
    // ─── variable.membre ────────────────────────────────────────────────────
    async completeInstance(document, className) {
        // Exceptions builtin : message/code/source, jamais de méthode.
        if (className === 'Exception' || className.endsWith('Exception')) {
            return EXCEPTION_PROPERTIES.map(p => this.exceptionPropertyItem(className, p));
        }
        const builtin = (0, builtins_1.getBuiltinClass)(className);
        if (builtin) {
            return builtin.methods.filter(m => !m.static).map(m => this.builtinMethodItem(className, m, false));
        }
        let members = await (0, resolver_1.findClassMembers)(document, className);
        // Classe importée par le programme dont ce fichier est un runtime.
        for (const doc of members.length === 0 ? await (0, runtimecontext_1.runtimeContext)(document) : []) {
            members = await (0, resolver_1.findClassMembers)(doc, className);
            if (members.length > 0) {
                break;
            }
        }
        const items = members.filter(m => !m.isStatic).map(m => this.memberItem(className, m));
        // Méthodes héritées d'un parent builtin (`class Server extends HTTPServer`).
        const ancestor = await (0, resolver_2.findBuiltinAncestor)(document, className, name => (0, builtins_1.getBuiltinClass)(name) !== undefined);
        const own = new Set(members.map(m => m.name));
        for (const m of ancestor ? (0, builtins_1.getBuiltinClass)(ancestor).methods : []) {
            if (!m.static && !own.has(m.name)) {
                items.push(this.builtinMethodItem(ancestor, m, false));
            }
        }
        return items;
    }
    // ─── use ClassName(...) ─────────────────────────────────────────────────
    completeClassNames(document) {
        const items = [];
        const seen = new Set();
        for (const c of (0, builtins_1.builtinClasses)()) {
            const hasInstanceApi = c.methods.some(m => !m.static);
            if (!hasInstanceApi) {
                continue;
            } // classes purement statiques : jamais `use X()`
            const item = new vscode.CompletionItem(c.name, vscode.CompletionItemKind.Class);
            item.detail = `ocara.${c.name}`;
            item.insertText = new vscode.SnippetString(`${c.name}($1)`);
            items.push(item);
            seen.add(c.name);
        }
        for (const name of (0, resolver_1.collectKnownClassNames)(document)) {
            if (seen.has(name)) {
                continue;
            }
            // Builtin purement statique (ex: IO, Math, Array) importé par son nom
            // (`import ocara.IO`) : jamais instanciable via `use`, on l'exclut.
            const builtin = (0, builtins_1.getBuiltinClass)(name);
            if (builtin && !builtin.methods.some(m => !m.static)) {
                continue;
            }
            seen.add(name);
            const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Class);
            item.insertText = new vscode.SnippetString(`${name}($1)`);
            items.push(item);
        }
        return items;
    }
    // ─── Construction des CompletionItem ───────────────────────────────────
    builtinMethodItem(className, m, isStatic) {
        const item = new vscode.CompletionItem(m.name, vscode.CompletionItemKind.Method);
        const paramsStr = m.params.map(p => `${p.name}:${p.type}`).join(', ');
        const sep = isStatic ? '::' : '.';
        item.detail = `${className}${sep}${m.name}(${paramsStr}): ${m.returns}`;
        item.insertText = callSnippet(m.name, m.params.map(p => p.name));
        item.documentation = new vscode.MarkdownString((0, hover_1.builtinDoc)(className, m, sep));
        return item;
    }
    primitiveMethodItem(receiver, m) {
        const item = new vscode.CompletionItem(m.name, vscode.CompletionItemKind.Method);
        const paramsStr = m.params.map(p => `${p.name}:${p.type}`).join(', ');
        item.detail = `${receiver}.${m.name}(${paramsStr}): ${m.returns}`;
        item.insertText = callSnippet(m.name, m.params.map(p => p.name));
        const [cls, method] = m.target.split('::');
        const target = (0, builtins_1.getBuiltinClass)(cls)?.methods.find(x => x.name === method);
        item.documentation = new vscode.MarkdownString(`\`${item.detail}\`\n\nÉquivalent de \`${m.target}(${receiver}${m.params.length > 0 ? ', ' + m.params.map(p => p.name).join(', ') : ''})\`` +
            (target?.doc ? `\n\n${target.doc}` : ''));
        return item;
    }
    builtinConstItem(className, c, isStatic) {
        const item = new vscode.CompletionItem(c.name, vscode.CompletionItemKind.Constant);
        item.detail = `${className}${isStatic ? '::' : '.'}${c.name}: ${c.type}`;
        item.documentation = new vscode.MarkdownString(`Constante builtin — \`ocara.${className}\``);
        return item;
    }
    exceptionPropertyItem(className, p) {
        const item = new vscode.CompletionItem(p.name, vscode.CompletionItemKind.Field);
        item.detail = `${className}.${p.name}: ${p.type}`;
        item.documentation = new vscode.MarkdownString('Propriété commune à toutes les exceptions Ocara.');
        return item;
    }
    memberItem(className, m) {
        const sep = m.isStatic ? '::' : '.';
        if (m.kind === 'method') {
            const item = new vscode.CompletionItem(m.name, vscode.CompletionItemKind.Method);
            item.detail = `${className}${sep}${m.name}(${m.params}): ${m.returnType || 'void'}`;
            item.insertText = callSnippet(m.name, paramNames(m.params));
            item.documentation = new vscode.MarkdownString(`\`${item.detail}\`\n\n${m.visibility}${m.isStatic ? ' static' : ''} method — classe \`${className}\``);
            return item;
        }
        const kind = m.kind === 'const' ? vscode.CompletionItemKind.Constant : vscode.CompletionItemKind.Field;
        const item = new vscode.CompletionItem(m.name, kind);
        item.detail = `${className}${sep}${m.name}: ${m.returnType}`;
        item.documentation = new vscode.MarkdownString(`${m.visibility} ${m.kind} — classe \`${className}\``);
        return item;
    }
}
exports.OcaraCompletionProvider = OcaraCompletionProvider;
/** Appel avec un champ à remplir par paramètre, nommé comme le paramètre. */
function callSnippet(name, params) {
    const escape = (t) => t.replace(/[$}\\]/g, '\\$&');
    const fields = params.map((p, i) => `\${${i + 1}:${escape(p)}}`);
    return new vscode.SnippetString(`${name}(${fields.join(', ')})`);
}
/** Noms des paramètres d'une liste brute (`a:int, b:string = "x"`), variadic compris. */
function paramNames(params) {
    return params.split(',')
        .map(p => p.trim().match(/^([A-Za-z_]\w*)\s*:/)?.[1])
        .filter((n) => !!n);
}
const FUNCTION_DECL_RE = /\bfunction\s+([A-Za-z_]\w*)\s*\(([^)]*)\)\s*:\s*([^{]+)\{/g;
async function collectFunctions(document) {
    const texts = [document.getText()];
    for (const doc of await (0, runtimecontext_1.runtimeContext)(document)) {
        texts.push(doc.getText());
    }
    for (const imp of (0, resolver_2.parseFileImports)(document)) {
        const uri = await (0, resolver_2.resolveFileImportUri)(document, imp.filePath);
        if (uri) {
            texts.push((await vscode.workspace.openTextDocument(uri)).getText());
        }
    }
    for (const imp of (0, resolver_2.parseImports)(document)) {
        const loc = (0, resolver_2.resolveImportPath)(document, imp.importPath);
        if (loc) {
            texts.push((await vscode.workspace.openTextDocument(loc.uri)).getText());
        }
    }
    const functions = [];
    for (const text of texts) {
        for (const m of text.matchAll(FUNCTION_DECL_RE)) {
            functions.push({ name: m[1], params: m[2].replace(/\s+/g, ' ').trim(), returnType: m[3].trim(), comment: (0, resolver_2.leadingComment)(text, m.index) });
        }
    }
    return functions;
}
//# sourceMappingURL=completion.js.map