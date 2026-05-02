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
        // ── 1a. Ligne d'import avec from : import ... from "file" ─────────────
        const importFromMatch = lineText.match(/^\s*import\s+([\w*]+)\s+from\s+"([^"]+)"(?:\s+as\s+(\w+))?\s*$/);
        if (importFromMatch) {
            const symbol = importFromMatch[1];
            const filePath = importFromMatch[2];
            return this.resolveFileImport(document, symbol, filePath);
        }
        // ── 1b. Ligne d'import namespace : import foo.bar.Baz ─────────────────
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
        // ── 2. obj.method() — appel de méthode d'instance ─────────────────────
        // Cherche tous les patterns obj.method() dans la ligne
        const methodCallRe = /(\w+(?:\.\w+)*)\.([\w]+)\s*\(/g;
        let match;
        while ((match = methodCallRe.exec(lineText)) !== null) {
            const objectPath = match[1]; // "self.circle" ou "circle"
            const methodName = match[2]; // "area"
            const methodStart = match.index + match[1].length + 1; // Position du nom de méthode
            const methodEnd = methodStart + methodName.length;
            // Vérifie si le curseur est sur le nom de la méthode
            if (position.character >= methodStart && position.character <= methodEnd) {
                return this.resolveInstanceMethod(document, objectPath, methodName).then(loc => loc ? [loc] : undefined);
            }
        }
        // ── 3. PascalCase → classe ou import ──────────────────────────────────
        if (/^[A-Z]/.test(word)) {
            return this.resolveTypeName(document, word, position);
        }
        // ── 4. ClassName::member — membre statique d'une autre classe ────────
        const staticCallMatch = lineText.match(/\b([A-Z]\w*)::(\w+)\b/g);
        if (staticCallMatch) {
            for (const chunk of staticCallMatch) {
                const parts = chunk.split('::');
                if (parts[1] === word) {
                    const className = parts[0];
                    return this.resolveStaticMember(document, className, word).then(loc => loc ? [loc] : undefined);
                }
            }
        }
        // ── 4. snake_case / camelCase → déclaration de variable ou fonction ──
        return this.resolveIdentifier(document, word, position);
    }
    // ─── Résolution d'un membre statique ClassName::member ───────────────────
    async resolveStaticMember(document, className, memberName) {
        // Cherche le fichier de la classe via les imports from d'abord
        const fileImports = this.parseFileImports(document);
        for (const imp of fileImports) {
            const match = imp.alias === className || (!imp.alias && imp.symbol === className) || imp.symbol === '*';
            if (match) {
                const targetUri = await this.resolveFileImportUri(document, imp.filePath);
                if (targetUri) {
                    const memberLoc = this.findMemberInFile(targetUri, memberName);
                    if (memberLoc) {
                        return memberLoc;
                    }
                    // Si pas trouvé, ouvre au moins le fichier
                    return new vscode.Location(targetUri, new vscode.Position(0, 0));
                }
            }
        }
        // Cherche ensuite via les imports namespace
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
        const memberLoc = this.findMemberInFile(searchUri, memberName);
        if (memberLoc) {
            return memberLoc;
        }
        // Rien trouvé : ouvre au moins le fichier de la classe
        if (targetFile) {
            return new vscode.Location(vscode.Uri.file(targetFile), new vscode.Position(0, 0));
        }
        return undefined;
    }
    // ─── Résolution d'un appel de méthode d'instance obj.method() ─────────────
    async resolveInstanceMethod(document, objectPath, methodName) {
        // Extrait le nom de la variable/propriété (dernier segment)
        const segments = objectPath.split('.');
        const varName = segments[segments.length - 1];
        // Trouve le type de cette variable dans le document
        const typeName = this.findVariableType(document, varName);
        if (!typeName) {
            return undefined;
        }
        // Cherche le fichier de cette classe via les imports from
        const fileImports = this.parseFileImports(document);
        for (const imp of fileImports) {
            const match = imp.alias === typeName || (!imp.alias && imp.symbol === typeName) || imp.symbol === '*';
            if (match) {
                const targetUri = await this.resolveFileImportUri(document, imp.filePath);
                if (targetUri) {
                    const memberLoc = this.findMemberInFile(targetUri, methodName);
                    if (memberLoc) {
                        return memberLoc;
                    }
                }
            }
        }
        // Cherche ensuite via les imports namespace
        const imports = this.parseImports(document);
        for (const imp of imports) {
            const match = imp.alias === typeName || (!imp.alias && imp.lastName === typeName);
            if (match) {
                const loc = this.resolveImportPath(document, imp.importPath);
                if (loc) {
                    const memberLoc = this.findMemberInFile(loc.uri, methodName);
                    if (memberLoc) {
                        return memberLoc;
                    }
                }
            }
        }
        // Cherche dans le fichier courant (classe locale)
        const memberLoc = this.findMemberInFile(document.uri, methodName);
        if (memberLoc) {
            return memberLoc;
        }
        return undefined;
    }
    // ─── Trouve le type d'une variable/propriété ──────────────────────────────
    findVariableType(document, varName) {
        // Cherche les déclarations de propriétés : private/public/property name:Type
        const propertyRe = new RegExp(`\\b(?:private|public)?\\s*property\\s+(${esc(varName)})\\s*:\\s*([A-Z]\\w*)`);
        // Cherche les déclarations de variables : var name:Type
        const varRe = new RegExp(`\\b(?:var|scoped|const)\\s+(${esc(varName)})\\s*:\\s*([A-Z]\\w*)`);
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            // Propriété
            let m = text.match(propertyRe);
            if (m && m[2]) {
                return m[2]; // Retourne le type
            }
            // Variable
            m = text.match(varRe);
            if (m && m[2]) {
                return m[2]; // Retourne le type
            }
        }
        return undefined;
    }
    // ─── Trouve un membre (méthode/fonction) dans un fichier ──────────────────
    findMemberInFile(uri, memberName) {
        if (!fs.existsSync(uri.fsPath)) {
            return undefined;
        }
        const content = fs.readFileSync(uri.fsPath, 'utf8');
        const lines = content.split('\n');
        const re = new RegExp(`\\b(?:method|function)\\s+(${esc(memberName)})\\s*\\(`);
        for (let i = 0; i < lines.length; i++) {
            const m = lines[i].match(re);
            if (m && m.index !== undefined) {
                const col = lines[i].indexOf(memberName, m.index);
                if (col >= 0) {
                    return new vscode.Location(uri, new vscode.Position(i, col));
                }
            }
        }
        return undefined;
    }
    // ─── Résout l'URI d'un fichier importé avec from ──────────────────────────
    async resolveFileImportUri(document, filePath) {
        const docDir = path.dirname(document.uri.fsPath);
        let targetPath = filePath.endsWith('.oc') ? filePath : filePath + '.oc';
        // Si le chemin est relatif explicite (../, ./), on résout directement
        if (targetPath.startsWith('../') || targetPath.startsWith('./')) {
            const absolutePath = path.resolve(docDir, targetPath);
            if (fs.existsSync(absolutePath)) {
                return vscode.Uri.file(absolutePath);
            }
            return undefined;
        }
        // Sinon, scanne le workspace pour trouver le fichier
        const ws = vscode.workspace.getWorkspaceFolder(document.uri);
        if (!ws) {
            return undefined;
        }
        // Cherche tous les fichiers .oc dans le workspace
        const pattern = new vscode.RelativePattern(ws, `**/${path.basename(targetPath)}`);
        const files = await vscode.workspace.findFiles(pattern, '**/node_modules/**');
        // Retourne le premier fichier trouvé
        if (files.length > 0) {
            return files[0];
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
        const currentNamespace = this.parseNamespace(document);
        const docDir = path.dirname(document.uri.fsPath);
        // Si l'import a un seul segment et qu'on est dans un namespace,
        // chercher d'abord dans le namespace courant (même dossier)
        if (segments.length === 1 && currentNamespace) {
            const namespacedPath = path.join(docDir, segments[0] + '.oc');
            if (fs.existsSync(namespacedPath)) {
                return new vscode.Location(vscode.Uri.file(namespacedPath), new vscode.Position(0, 0));
            }
        }
        // Détermine le root de recherche pour les imports multi-segments
        let searchRoot;
        if (currentNamespace) {
            const dirName = path.basename(docDir);
            if (dirName === currentNamespace) {
                // On est dans un dossier nommé comme le namespace, remonter d'un niveau
                searchRoot = path.dirname(docDir);
            }
            else {
                searchRoot = docDir;
            }
        }
        else {
            // Pas de namespace, chercher depuis docDir
            searchRoot = docDir;
        }
        const relFile = path.join(...segments) + '.oc';
        // Chercher depuis searchRoot (parent du namespace ou docDir)
        let candidate = path.join(searchRoot, relFile);
        if (fs.existsSync(candidate)) {
            return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
        }
        // Fallback : cherche depuis la racine du workspace
        const ws = vscode.workspace.getWorkspaceFolder(document.uri);
        if (ws) {
            candidate = path.join(ws.uri.fsPath, relFile);
            if (fs.existsSync(candidate)) {
                return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
            }
        }
        // Fallback : cherche depuis le répertoire du fichier courant
        candidate = path.join(docDir, relFile);
        if (fs.existsSync(candidate)) {
            return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
        }
        return undefined;
    }
    // ─── Navigation vers un fichier importé avec from ─────────────────────────
    /**
     * Résout un import avec syntaxe from : import Symbol from "file"
     * Gère les chemins relatifs (../, ../../) et scanne le workspace.
     */
    async resolveFileImport(document, symbol, filePath) {
        const docDir = path.dirname(document.uri.fsPath);
        // Ajoute .oc si pas d'extension
        let targetPath = filePath.endsWith('.oc') ? filePath : filePath + '.oc';
        // Si le chemin est relatif explicite (../, ./), on résout directement
        if (targetPath.startsWith('../') || targetPath.startsWith('./')) {
            const absolutePath = path.resolve(docDir, targetPath);
            if (fs.existsSync(absolutePath)) {
                return this.findSymbolInFile(absolutePath, symbol);
            }
            return undefined;
        }
        // Sinon, scanne le workspace pour trouver le fichier
        const ws = vscode.workspace.getWorkspaceFolder(document.uri);
        if (!ws) {
            return undefined;
        }
        // Cherche tous les fichiers .oc dans le workspace
        const pattern = new vscode.RelativePattern(ws, `**/${path.basename(targetPath)}`);
        const files = await vscode.workspace.findFiles(pattern, '**/node_modules/**');
        // Retourne le premier fichier trouvé
        if (files.length > 0) {
            return this.findSymbolInFile(files[0].fsPath, symbol);
        }
        return undefined;
    }
    // ─── Trouve un symbole dans un fichier ────────────────────────────────────
    findSymbolInFile(absolutePath, symbol) {
        const targetUri = vscode.Uri.file(absolutePath);
        // Si wildcard (*), ouvre au début du fichier
        if (symbol === '*') {
            return new vscode.Location(targetUri, new vscode.Position(0, 0));
        }
        // Sinon, cherche la définition du symbole dans le fichier cible
        const content = fs.readFileSync(absolutePath, 'utf8');
        const lines = content.split('\n');
        // Cherche class, interface, function, module, enum
        const symbolRe = new RegExp(`\\b(?:class|interface|function|module|enum)\\s+(${esc(symbol)})\\b`);
        for (let i = 0; i < lines.length; i++) {
            const m = lines[i].match(symbolRe);
            if (m && m.index !== undefined) {
                const col = lines[i].indexOf(symbol, m.index);
                if (col >= 0) {
                    return new vscode.Location(targetUri, new vscode.Position(i, col));
                }
            }
        }
        // Si pas trouvé, ouvre au début du fichier
        return new vscode.Location(targetUri, new vscode.Position(0, 0));
    }
    // ─── Résolution d'un nom de type / classe ─────────────────────────────────
    async resolveTypeName(document, name, position) {
        const imports = this.parseImports(document);
        const fileImports = this.parseFileImports(document);
        // Cherche d'abord dans les imports from (priorité car plus explicite)
        for (const imp of fileImports) {
            // Correspondance par alias
            if (imp.alias === name) {
                const loc = await this.resolveFileImport(document, imp.symbol, imp.filePath);
                if (loc) {
                    return [loc];
                }
            }
            // Correspondance par symbole (si pas d'alias)
            if (!imp.alias && imp.symbol === name) {
                const loc = await this.resolveFileImport(document, imp.symbol, imp.filePath);
                if (loc) {
                    return [loc];
                }
            }
            // Wildcard : tous les symboles sont disponibles
            if (imp.symbol === '*') {
                const loc = await this.resolveFileImport(document, name, imp.filePath);
                if (loc) {
                    return [loc];
                }
            }
        }
        // Cherche ensuite une correspondance par alias dans les imports namespace
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
    // ─── Parse les imports from du document ───────────────────────────────────
    parseFileImports(document) {
        const entries = [];
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            // Match: import Symbol from "file" [as Alias]
            const m = text.match(/^\s*import\s+([\w*]+)\s+from\s+"([^"]+)"(?:\s+as\s+(\w+))?\s*$/);
            if (!m) {
                continue;
            }
            const symbol = m[1];
            const filePath = m[2];
            const alias = m[3];
            entries.push({
                symbol,
                filePath,
                alias,
                line: i,
            });
        }
        return entries;
    }
    // ─── Parse le namespace du document ──────────────────────────────────────
    /**
     * Extrait le namespace déclaré dans le document.
     * Retourne null pour namespace root (namespace .) ou pas de namespace,
     * retourne le nom du namespace sinon (ex: "classes").
     */
    parseNamespace(document) {
        // Cherche la première ligne non-vide et non-commentaire
        for (let i = 0; i < Math.min(5, document.lineCount); i++) {
            const text = document.lineAt(i).text.trim();
            if (!text || text.startsWith('//')) {
                continue;
            }
            // Match: namespace .
            if (/^namespace\s+\.\s*$/.test(text)) {
                return null; // root namespace
            }
            // Match: namespace identifier
            const m = text.match(/^namespace\s+([\w]+)\s*$/);
            if (m) {
                return m[1];
            }
            // Si on trouve autre chose qu'un namespace, on arrête
            break;
        }
        return null; // pas de namespace déclaré = root
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