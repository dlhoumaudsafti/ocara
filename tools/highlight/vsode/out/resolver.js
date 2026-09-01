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
exports.esc = esc;
exports.parseNamespace = parseNamespace;
exports.parseImports = parseImports;
exports.parseFileImports = parseFileImports;
exports.resolveImportPath = resolveImportPath;
exports.resolveFileImportUri = resolveFileImportUri;
exports.findVariableType = findVariableType;
exports.findEnclosingClassName = findEnclosingClassName;
exports.resolveClassUri = resolveClassUri;
exports.findClassMembers = findClassMembers;
exports.collectKnownClassNames = collectKnownClassNames;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const fs = __importStar(require("fs"));
/** Échappe les caractères spéciaux pour usage dans une RegExp. */
function esc(s) {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
/**
 * Extrait le namespace déclaré dans le document.
 * Retourne null pour namespace root (namespace .) ou pas de namespace,
 * retourne le nom du namespace sinon (ex: "classes").
 */
function parseNamespace(document) {
    for (let i = 0; i < Math.min(5, document.lineCount); i++) {
        const text = document.lineAt(i).text.trim();
        if (!text || text.startsWith('//')) {
            continue;
        }
        if (/^namespace\s+\.\s*$/.test(text)) {
            return null;
        }
        const m = text.match(/^namespace\s+([\w]+)\s*$/);
        if (m) {
            return m[1];
        }
        break;
    }
    return null;
}
function parseImports(document) {
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
        entries.push({ importPath, alias, lastName: segs[segs.length - 1], line: i });
    }
    return entries;
}
function parseFileImports(document) {
    const entries = [];
    for (let i = 0; i < document.lineCount; i++) {
        const text = document.lineAt(i).text;
        const m = text.match(/^\s*import\s+([\w*]+)\s+from\s+"([^"]+)"(?:\s+as\s+(\w+))?\s*$/);
        if (!m) {
            continue;
        }
        entries.push({ symbol: m[1], filePath: m[2], alias: m[3], line: i });
    }
    return entries;
}
/**
 * Convertit un chemin d'import ("foo.bar.Baz") vers le fichier .oc correspondant.
 * Les imports `ocara.*` sont des builtins sans fichier navigable (undefined).
 */
function resolveImportPath(document, importPath) {
    if (importPath.startsWith('ocara.')) {
        return undefined;
    }
    const segments = importPath.split('.');
    const currentNamespace = parseNamespace(document);
    const docDir = path.dirname(document.uri.fsPath);
    if (segments.length === 1 && currentNamespace) {
        const namespacedPath = path.join(docDir, segments[0] + '.oc');
        if (fs.existsSync(namespacedPath)) {
            return new vscode.Location(vscode.Uri.file(namespacedPath), new vscode.Position(0, 0));
        }
    }
    let searchRoot;
    if (currentNamespace) {
        const dirName = path.basename(docDir);
        searchRoot = dirName === currentNamespace ? path.dirname(docDir) : docDir;
    }
    else {
        searchRoot = docDir;
    }
    const relFile = path.join(...segments) + '.oc';
    let candidate = path.join(searchRoot, relFile);
    if (fs.existsSync(candidate)) {
        return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
    }
    const ws = vscode.workspace.getWorkspaceFolder(document.uri);
    if (ws) {
        candidate = path.join(ws.uri.fsPath, relFile);
        if (fs.existsSync(candidate)) {
            return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
        }
    }
    candidate = path.join(docDir, relFile);
    if (fs.existsSync(candidate)) {
        return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(0, 0));
    }
    return undefined;
}
/** Résout l'URI d'un fichier importé avec `from "..."`. */
async function resolveFileImportUri(document, filePath) {
    const docDir = path.dirname(document.uri.fsPath);
    const targetPath = filePath.endsWith('.oc') ? filePath : filePath + '.oc';
    if (targetPath.startsWith('../') || targetPath.startsWith('./')) {
        const absolutePath = path.resolve(docDir, targetPath);
        return fs.existsSync(absolutePath) ? vscode.Uri.file(absolutePath) : undefined;
    }
    const ws = vscode.workspace.getWorkspaceFolder(document.uri);
    if (!ws) {
        return undefined;
    }
    const pattern = new vscode.RelativePattern(ws, `**/${path.basename(targetPath)}`);
    const files = await vscode.workspace.findFiles(pattern, '**/node_modules/**');
    return files.length > 0 ? files[0] : undefined;
}
/** Trouve le type déclaré (`var`/`scoped`/`const`/`property`) d'une variable dans le document. */
function findVariableType(document, varName) {
    const propertyRe = new RegExp(`\\b(?:private|public|protected)?\\s*property\\s+(${esc(varName)})\\s*:\\s*([A-Z]\\w*)(?:<[^>]+>)?`);
    const varRe = new RegExp(`\\b(?:var|scoped|const)\\s+(${esc(varName)})\\s*:\\s*([A-Z]\\w*)(?:<[^>]+>)?`);
    const paramRe = new RegExp(`\\b(${esc(varName)})\\s*:\\s*([A-Z]\\w*)(?:<[^>]+>)?`);
    // `try { ... } on e is FileException { ... }` ou `on e { ... }` (catch générique)
    const catchTypedRe = new RegExp(`\\bon\\s+(${esc(varName)})\\s+is\\s+(\\w+)`);
    const catchUntypedRe = new RegExp(`\\bon\\s+(${esc(varName)})\\s*\\{`);
    for (let i = 0; i < document.lineCount; i++) {
        const text = document.lineAt(i).text;
        let m = text.match(propertyRe);
        if (m && m[2]) {
            return m[2];
        }
        m = text.match(varRe);
        if (m && m[2]) {
            return m[2];
        }
        m = text.match(catchTypedRe);
        if (m && m[2]) {
            return m[2];
        }
        if (catchUntypedRe.test(text)) {
            return 'Exception';
        }
    }
    // Fallback : paramètre de fonction/méthode (ex: function foo(self, x:MyClass))
    for (let i = 0; i < document.lineCount; i++) {
        const text = document.lineAt(i).text;
        if (!/\b(?:function|method|init|nameless)\b/.test(text)) {
            continue;
        }
        const m = text.match(paramRe);
        if (m && m[2]) {
            return m[2];
        }
    }
    return undefined;
}
/** Trouve le nom de la classe/generic englobant une position (scan ascendant, heuristique). */
function findEnclosingClassName(document, position) {
    const classRe = /\b(?:generic|class)\s+(\w+)/;
    for (let i = position.line; i >= 0; i--) {
        const m = document.lineAt(i).text.match(classRe);
        if (m) {
            return m[1];
        }
    }
    return undefined;
}
/** Résout l'URI du fichier définissant `className` (import from, import namespace, ou local). */
async function resolveClassUri(document, className) {
    for (const imp of parseFileImports(document)) {
        const match = imp.alias === className || (!imp.alias && imp.symbol === className) || imp.symbol === '*';
        if (match) {
            const uri = await resolveFileImportUri(document, imp.filePath);
            if (uri) {
                return uri;
            }
        }
    }
    for (const imp of parseImports(document)) {
        const match = imp.alias === className || (!imp.alias && imp.lastName === className);
        if (match) {
            const loc = resolveImportPath(document, imp.importPath);
            if (loc) {
                return loc.uri;
            }
        }
    }
    // Classe locale : présente dans le document courant lui-même ?
    const localRe = new RegExp(`\\b(?:generic|class|interface)\\s+(${esc(className)})\\b`);
    for (let i = 0; i < document.lineCount; i++) {
        if (localRe.test(document.lineAt(i).text)) {
            return document.uri;
        }
    }
    return undefined;
}
/**
 * Isole le corps d'une classe (`class Name ... { ... }`) dans le texte d'un
 * fichier, par comptage naïf d'accolades (ne tient pas compte des accolades
 * dans les chaînes/commentaires — limitation acceptée, comme le reste de
 * cette extension basée sur des regex plutôt qu'un vrai parseur).
 */
function findClassBody(fileText, className) {
    const declRe = new RegExp(`\\b(?:generic|class)\\s+${esc(className)}\\b([^{]*)\\{`);
    const m = declRe.exec(fileText);
    if (!m) {
        return undefined;
    }
    const header = m[1] || '';
    const extendsMatch = header.match(/\bextends\s+(\w+)/);
    let depth = 1;
    let i = m.index + m[0].length;
    const start = i;
    while (i < fileText.length && depth > 0) {
        const ch = fileText[i];
        if (ch === '{') {
            depth++;
        }
        else if (ch === '}') {
            depth--;
        }
        i++;
    }
    return {
        text: fileText.slice(start, depth === 0 ? i - 1 : i),
        extendsName: extendsMatch ? extendsMatch[1] : undefined,
    };
}
/**
 * Efface le contenu des corps de méthodes (`method X(...) : T { ... }` /
 * `init(...) { ... }`) d'un corps de classe, en gardant tout le reste
 * (properties, consts, signatures) intact. Nécessaire pour ne pas confondre
 * un `const` LOCAL déclaré dans une méthode (ex: `const db:SQLite = ...`)
 * avec une constante de classe (`public const NAME:T = ...`), les deux
 * utilisant le même mot-clé `const` en Ocara.
 */
function stripMethodBodies(bodyText) {
    const sigRe = /(?:(?:public|private|protected|static)\s+)*(?:method\s+\w+|init)\s*\(.*?\)\s*(?::\s*[^{]+)?\{/g;
    let result = '';
    let i = 0;
    let m;
    sigRe.lastIndex = 0;
    while ((m = sigRe.exec(bodyText)) !== null) {
        if (m.index < i) {
            continue;
        } // chevauchement avec une méthode déjà traitée
        result += bodyText.slice(i, m.index); // texte avant la méthode : conservé
        const openIdx = m.index + m[0].length - 1; // position du '{' d'ouverture
        let depth = 1;
        let j = openIdx + 1;
        while (j < bodyText.length && depth > 0) {
            if (bodyText[j] === '{') {
                depth++;
            }
            else if (bodyText[j] === '}') {
                depth--;
            }
            j++;
        }
        i = j; // saute tout le corps de la méthode
        sigRe.lastIndex = i;
    }
    result += bodyText.slice(i);
    return result;
}
/** Extrait les méthodes/constantes/propriétés déclarées directement dans un corps de classe. */
function extractMembers(bodyText) {
    const members = [];
    const seen = new Set();
    const methodRe = /((?:(?:public|private|protected|static)\s+)*)method\s+(\w+)\s*\(([^)]*)\)\s*:\s*([^\{]+)\{/g;
    let m;
    while ((m = methodRe.exec(bodyText)) !== null) {
        const modifiers = m[1] || '';
        const key = 'method:' + m[2];
        if (seen.has(key)) {
            continue;
        }
        seen.add(key);
        members.push({
            name: m[2],
            kind: 'method',
            isStatic: /\bstatic\b/.test(modifiers),
            visibility: modifiers.match(/\b(public|private|protected)\b/)?.[1] || 'public',
            params: m[3].trim(),
            returnType: m[4].trim(),
        });
    }
    // Consts/properties : uniquement au niveau de la classe, jamais dans un
    // corps de méthode (un `const` local y utilise le même mot-clé Ocara).
    const topLevelText = stripMethodBodies(bodyText);
    const constRe = /((?:(?:public|private|protected|static)\s+)*)const\s+(\w+)\s*:\s*([^=\n]+)=/g;
    while ((m = constRe.exec(topLevelText)) !== null) {
        const modifiers = m[1] || '';
        const key = 'const:' + m[2];
        if (seen.has(key)) {
            continue;
        }
        seen.add(key);
        members.push({
            name: m[2],
            kind: 'const',
            isStatic: true, // les constantes de classe s'utilisent toujours via ClassName::NAME
            visibility: modifiers.match(/\b(public|private|protected)\b/)?.[1] || 'public',
            params: '',
            returnType: m[3].trim(),
        });
    }
    const propertyRe = /((?:(?:public|private|protected)\s+)*)property\s+(\w+)\s*:\s*([^\n=]+)/g;
    while ((m = propertyRe.exec(topLevelText)) !== null) {
        const modifiers = m[1] || '';
        const key = 'property:' + m[2];
        if (seen.has(key)) {
            continue;
        }
        seen.add(key);
        members.push({
            name: m[2],
            kind: 'property',
            isStatic: false,
            visibility: modifiers.match(/\b(public|private|protected)\b/)?.[1] || 'public',
            params: '',
            returnType: m[3].trim(),
        });
    }
    return members;
}
/**
 * Résout tous les membres (méthodes/constantes/propriétés) d'une classe,
 * en remontant la chaîne `extends` (jusqu'à 5 niveaux, garde-fou anti-cycle).
 */
async function findClassMembers(document, className, depth = 0) {
    if (depth > 5) {
        return [];
    }
    const uri = await resolveClassUri(document, className);
    if (!uri) {
        return [];
    }
    let fileText;
    try {
        fileText = fs.readFileSync(uri.fsPath, 'utf8');
    }
    catch {
        return [];
    }
    const body = findClassBody(fileText, className);
    if (!body) {
        return [];
    }
    const members = extractMembers(body.text);
    if (body.extendsName && body.extendsName !== className) {
        try {
            const parentDoc = uri.fsPath === document.uri.fsPath
                ? document
                : await vscode.workspace.openTextDocument(uri);
            const parentMembers = await findClassMembers(parentDoc, body.extendsName, depth + 1);
            const ownNames = new Set(members.map(mem => mem.kind + ':' + mem.name));
            for (const pm of parentMembers) {
                if (!ownNames.has(pm.kind + ':' + pm.name)) {
                    members.push(pm);
                }
            }
        }
        catch {
            // Classe parente non résolvable : on garde au moins les membres propres.
        }
    }
    return members;
}
/** Liste tous les noms de classes/génériques/interfaces connus du document (imports + locales). */
function collectKnownClassNames(document) {
    const names = new Set();
    for (const imp of parseImports(document)) {
        names.add(imp.alias || imp.lastName);
    }
    for (const imp of parseFileImports(document)) {
        if (imp.symbol !== '*') {
            names.add(imp.alias || imp.symbol);
        }
    }
    const localRe = /\b(?:generic|class|interface)\s+(\w+)/g;
    const text = document.getText();
    let m;
    while ((m = localRe.exec(text)) !== null) {
        names.add(m[1]);
    }
    return Array.from(names);
}
//# sourceMappingURL=resolver.js.map