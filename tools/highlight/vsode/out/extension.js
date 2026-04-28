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
const path = __importStar(require("path"));
const fs = __importStar(require("fs"));
function activate(context) {
    const selector = { language: 'ocara', scheme: 'file' };
    context.subscriptions.push(vscode.languages.registerDefinitionProvider(selector, new OcaraDefinitionProvider()));
}
function deactivate() { }
// ─── Provider ────────────────────────────────────────────────────────────────
class OcaraDefinitionProvider {
    provideDefinition(document, position, _token) {
        const lineText = document.lineAt(position.line).text;
        // ── 1. Ligne d'import : Ctrl+Click n'importe où sur la ligne ──────────
        const importLineMatch = lineText.match(/^\s*import\s+([\w.]+)(?:\s+as\s+(\w+))?\s*$/);
        if (importLineMatch) {
            const loc = this.resolveImportPath(document, importLineMatch[1]);
            return loc ? [loc] : undefined;
        }
        // Récupère l'identifiant sous le curseur
        const wordRange = document.getWordRangeAtPosition(position, /[A-Za-z_][\w]*/);
        if (!wordRange) {
            return undefined;
        }
        const word = document.getText(wordRange);
        // ── 2. PascalCase → classe ou import ──────────────────────────────────
        if (/^[A-Z]/.test(word)) {
            return this.resolveTypeName(document, word, position);
        }
        // ── 3. ClassName::member — membre statique d'une autre classe ────────
        const staticCallMatch = lineText.match(/\b([A-Z]\w*)::(\w+)\b/g);
        if (staticCallMatch) {
            for (const chunk of staticCallMatch) {
                const parts = chunk.split('::');
                if (parts[1] === word) {
                    const className = parts[0];
                    const loc = this.resolveStaticMember(document, className, word);
                    if (loc) {
                        return [loc];
                    }
                }
            }
        }
        // ── 4. snake_case / camelCase → déclaration de variable ou fonction ──
        return this.resolveIdentifier(document, word, position);
    }
    // ─── Résolution d'un membre statique ClassName::member ───────────────────
    resolveStaticMember(document, className, memberName) {
        // Cherche le fichier de la classe via les imports
        const imports = this.parseImports(document);
        let targetFile;
        for (const imp of imports) {
            const match = imp.alias === className || (!imp.alias && imp.lastName === className);
            if (match) {
                const loc = this.resolveImportPath(document, imp.importPath);
                if (loc) {
                    targetFile = loc.uri.fsPath;
                    break;
                }
            }
        }
        // Recherche dans le fichier cible (ou le fichier courant si pas d'import)
        const searchUri = targetFile
            ? vscode.Uri.file(targetFile)
            : document.uri;
        const content = fs.readFileSync(searchUri.fsPath, 'utf8');
        const lines = content.split('\n');
        const re = new RegExp(`\\b(?:method|function)\\s+(${esc(memberName)})\\s*\\(`);
        for (let i = 0; i < lines.length; i++) {
            const m = lines[i].match(re);
            if (m && m.index !== undefined) {
                const col = lines[i].indexOf(memberName, m.index);
                if (col >= 0) {
                    return new vscode.Location(searchUri, new vscode.Position(i, col));
                }
            }
        }
        // Rien trouvé : ouvre au moins le fichier de la classe
        if (targetFile) {
            return new vscode.Location(vscode.Uri.file(targetFile), new vscode.Position(0, 0));
        }
        return undefined;
    }
    // ─── Navigation vers un fichier importé ───────────────────────────────────
    /**
     * Convertit un chemin d'import ("foo.bar.Baz") vers le fichier .oc correspondant.
     * Les imports `ocara.*` sont des builtins sans fichier navigable.
     */
    resolveImportPath(document, importPath) {
        if (importPath.startsWith('ocara.')) {
            return undefined;
        }
        const segments = importPath.split('.');
        const relFile = path.join(...segments) + '.oc';
        // Cherche depuis la racine du workspace en premier
        const ws = vscode.workspace.getWorkspaceFolder(document.uri);
        if (ws) {
            const candidate = path.join(ws.uri.fsPath, relFile);
            if (fs.existsSync(candidate)) {
                return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
            }
        }
        // Fallback : cherche depuis le répertoire du fichier courant
        const docDir = path.dirname(document.uri.fsPath);
        const candidate = path.join(docDir, relFile);
        if (fs.existsSync(candidate)) {
            return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
        }
        return undefined;
    }
    // ─── Résolution d'un nom de type / classe ─────────────────────────────────
    resolveTypeName(document, name, position) {
        const imports = this.parseImports(document);
        // Cherche d'abord une correspondance par alias (priorité sur le nom simple)
        for (const imp of imports) {
            if (imp.alias === name) {
                const loc = this.resolveImportPath(document, imp.importPath);
                if (loc) {
                    return [loc];
                }
            }
        }
        // Cherche ensuite par nom de dernier segment (sans alias)
        for (const imp of imports) {
            if (!imp.alias && imp.lastName === name) {
                const loc = this.resolveImportPath(document, imp.importPath);
                if (loc) {
                    return [loc];
                }
            }
        }
        // Fallback : déclaration locale (class / interface / module / enum)
        return this.findTypeDeclaration(document, name, position);
    }
    // ─── Résolution d'un identifiant minuscule ────────────────────────────────
    resolveIdentifier(document, name, position) {
        // Patterns de déclaration de variable / propriété / constante
        const varDeclRe = new RegExp(`\\b(?:var|scoped|const|property)\\s+(${esc(name)})\\s*:`);
        // Patterns de déclaration de fonction / méthode
        const funcDeclRe = new RegExp(`\\b(?:function|method)\\s+(${esc(name)})\\s*\\(`);
        // Pattern de paramètre sur une ligne de déclaration de fonction
        const paramRe = new RegExp(`\\b(${esc(name)})\\s*:`);
        // 1. Recherche vers le haut depuis la position courante (var / scoped)
        for (let i = position.line; i >= 0; i--) {
            const text = document.lineAt(i).text;
            const m = text.match(varDeclRe);
            if (m && m.index !== undefined) {
                const col = this.findWordCol(text, name, m.index);
                if (col >= 0 && !this.isSamePosition(i, col, position)) {
                    return new vscode.Location(document.uri, new vscode.Position(i, col));
                }
            }
        }
        // 2. Recherche dans tout le fichier pour function / method / property / const
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            const fm = text.match(funcDeclRe);
            if (fm && fm.index !== undefined) {
                const col = this.findWordCol(text, name, fm.index);
                if (col >= 0 && !this.isSamePosition(i, col, position)) {
                    return new vscode.Location(document.uri, new vscode.Position(i, col));
                }
            }
        }
        // 3. Paramètres : cherche le mot sur les lignes de déclaration de fonction
        //    (function / method / init / nameless) remontant depuis le curseur
        for (let i = position.line; i >= 0; i--) {
            const text = document.lineAt(i).text;
            if (!/\b(?:function|method|init|nameless)\b/.test(text)) {
                continue;
            }
            const pm = text.match(paramRe);
            if (pm && pm.index !== undefined) {
                const col = this.findWordCol(text, name, pm.index);
                if (col >= 0 && !this.isSamePosition(i, col, position)) {
                    return new vscode.Location(document.uri, new vscode.Position(i, col));
                }
            }
        }
        return undefined;
    }
    // ─── Déclaration locale d'un type (class / interface / module / enum) ──────
    findTypeDeclaration(document, name, position) {
        const re = new RegExp(`\\b(?:class|interface|module|enum)\\s+(${esc(name)})\\b`);
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            const m = text.match(re);
            if (m && m.index !== undefined) {
                const col = this.findWordCol(text, name, m.index);
                if (col >= 0 && !this.isSamePosition(i, col, position)) {
                    return new vscode.Location(document.uri, new vscode.Position(i, col));
                }
            }
        }
        return undefined;
    }
    // ─── Parse les imports du document ────────────────────────────────────────
    parseImports(document) {
        const entries = [];
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            const m = text.match(/^\s*import\s+([\w.]+)(?:\s+as\s+(\w+))?\s*$/);
            if (!m) {
                continue;
            }
            const importPath = m[1];
            const alias = m[2];
            const segs = importPath.split('.');
            entries.push({
                importPath,
                alias,
                lastName: segs[segs.length - 1],
                line: i,
            });
        }
        return entries;
    }
    // ─── Helpers ──────────────────────────────────────────────────────────────
    /** Trouve la colonne d'un mot dans `text` en partant de `fromIdx`. */
    findWordCol(text, word, fromIdx) {
        const idx = text.indexOf(word, fromIdx);
        if (idx < 0) {
            return -1;
        }
        // Vérifie qu'il s'agit d'un mot entier (pas partie d'un autre identifiant)
        const before = text[idx - 1];
        const after = text[idx + word.length];
        const isWordChar = (c) => c !== undefined && /\w/.test(c);
        if (isWordChar(before) || isWordChar(after)) {
            return -1;
        }
        return idx;
    }
    /** Retourne true si la position (line, col) correspond au curseur. */
    isSamePosition(line, col, position) {
        return line === position.line && col === position.character;
    }
}
// ─── Utilitaire ───────────────────────────────────────────────────────────────
/** Échappe les caractères spéciaux pour usage dans une RegExp. */
function esc(s) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
//# sourceMappingURL=extension.js.map