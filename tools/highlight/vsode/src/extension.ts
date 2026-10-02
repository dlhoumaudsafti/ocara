import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { OcaraCompletionProvider } from './completion';
import { loadBuiltins } from './builtins';
import { OcaraSignatureHelpProvider } from './signature';
import { OcaraCodeLensProvider, WorkspaceIndex } from './codelens';
import { OcaracsLinter } from './lint';
import { OcaraCompiler } from './compile';
import { registerFix } from './fix';
import { runtimeContext } from './runtimecontext';
import { OcaraHoverProvider } from './hover';
import { registerDocs } from './docs';
import { findCallSite, resolveCall } from './callsite';
import {
    esc,
    findVariableType,
    parseFileImports,
    parseImports,
    resolveFileImportUri,
    resolveImportPath,
} from './resolver';

export function activate(context: vscode.ExtensionContext): void {
    const selector: vscode.DocumentSelector = { language: 'ocara', scheme: 'file' };
    context.subscriptions.push(
        vscode.languages.registerDefinitionProvider(selector, new OcaraDefinitionProvider())
    );

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

    // Documentation au survol (builtins, sucre d'instance, déclarations utilisateur).
    context.subscriptions.push(vscode.languages.registerHoverProvider(selector, new OcaraHoverProvider()));

    // Analyse ocaracs automatique + commandes Compiler / Compiler et lancer /
    // Afficher le dump / Fixer la mise en forme.
    const linter = new OcaracsLinter();
    context.subscriptions.push(linter);
    new OcaraCompiler().register(context);
    registerFix(context, linter);
}

export function deactivate(): void {}

// ─── Provider ────────────────────────────────────────────────────────────────

class OcaraDefinitionProvider implements vscode.DefinitionProvider {

    private async resolveNamedArgument(
        document: vscode.TextDocument,
        position: vscode.Position,
        name: string
    ): Promise<vscode.Location[] | undefined> {
        const site = findCallSite(document, position);
        if (!site) { return undefined; }
        const call = await resolveCall(document, position, site);
        const param = call?.params.find(p => p.name === name);
        return param?.location ? [param.location] : undefined;
    }

    provideDefinition(
        document: vscode.TextDocument,
        position: vscode.Position,
        _token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.Definition> {

        const lineText = document.lineAt(position.line).text;

        // ── 1. Runtime import : runtime X ou runtime X is Y ────────────────────
        const runtimeMatch = lineText.match(/^\s*runtime\s+([\w.]+)(?:\s+is\s+\w+)?\s*$/);
        if (runtimeMatch) {
            const runtimePath = runtimeMatch[1];
            return this.resolveRuntimeImport(document, runtimePath);
        }

        // ── 2a. Ligne d'import avec from : import ... from "file" ─────────────
        const importFromMatch = lineText.match(/^\s*import\s+([\w*]+)\s+from\s+"([^"]+)"(?:\s+as\s+(\w+))?\s*$/);
        if (importFromMatch) {
            const symbol = importFromMatch[1];
            const filePath = importFromMatch[2];
            return this.resolveFileImport(document, symbol, filePath);
        }

        // ── 2a-bis. `wiring chemin.vers.Classe` (interface) → la classe ciblée ─
        // Même résolution qu'un import namespace, puis positionnement sur la
        // déclaration ; classe déclarée dans le fichier courant sinon.
        const wiringMatch = lineText.match(/^\s*wiring\s+([\w.]+)\s*$/);
        if (wiringMatch) {
            const target = wiringMatch[1];
            const className = target.split('.').pop()!;
            const loc = resolveImportPath(document, target);
            if (loc) { return [this.findSymbolInFile(loc.uri.fsPath, className)]; }
            return this.findTypeDeclaration(document, className, position);
        }

        // ── 2b. Ligne d'import namespace : import foo.bar.Baz ─────────────────
        const importLineMatch = lineText.match(/^\s*import\s+([\w.]+)(?:\s+as\s+(\w+))?\s*$/);
        if (importLineMatch) {
            const loc = resolveImportPath(document, importLineMatch[1]);
            return loc ? [loc] : undefined;
        }

        // Récupère l'identifiant sous le curseur
        const wordRange = document.getWordRangeAtPosition(position, /[A-Za-z_][\w]*/);
        if (!wordRange) { return undefined; }
        const word = document.getText(wordRange);

        // ── 1b. f(nom: ...) — argument nommé → paramètre déclaré ───────────────
        if (/^\s*:(?!:)/.test(lineText.substring(wordRange.end.character)) && findCallSite(document, wordRange.start)) {
            return this.resolveNamedArgument(document, wordRange.start, word);
        }

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

    private async resolveStaticMember(
        document: vscode.TextDocument,
        className: string,
        memberName: string
    ): Promise<vscode.Location | undefined> {
        // Cherche le fichier de la classe via les imports from d'abord
        const fileImports = parseFileImports(document);
        for (const imp of fileImports) {
            const match = imp.alias === className || (!imp.alias && imp.symbol === className) || imp.symbol === '*';
            if (match) {
                const targetUri = await resolveFileImportUri(document, imp.filePath);
                if (targetUri) {
                    const memberLoc = this.findMemberInFile(targetUri, memberName);
                    if (memberLoc) { return memberLoc; }
                    // Si pas trouvé, ouvre au moins le fichier
                    return new vscode.Location(targetUri, new vscode.Position(0, 0));
                }
            }
        }

        // Cherche ensuite via les imports namespace
        const imports = parseImports(document);
        let targetFile: string | undefined;

        for (const imp of imports) {
            const match = imp.alias === className || (!imp.alias && imp.lastName === className);
            if (match) {
                const loc = resolveImportPath(document, imp.importPath);
                if (loc) { targetFile = loc.uri.fsPath; break; }
            }
        }

        // Recherche dans le fichier cible (ou le fichier courant si pas d'import)
        const searchUri = targetFile
            ? vscode.Uri.file(targetFile)
            : document.uri;

        const memberLoc = this.findMemberInFile(searchUri, memberName);
        if (memberLoc) { return memberLoc; }

        // Rien trouvé : ouvre au moins le fichier de la classe
        if (targetFile) {
            return new vscode.Location(vscode.Uri.file(targetFile), new vscode.Position(0, 0));
        }

        return undefined;
    }

    // ─── Résolution d'un appel de méthode d'instance obj.method() ─────────────

    private async resolveInstanceMethod(
        document: vscode.TextDocument,
        objectPath: string,
        methodName: string
    ): Promise<vscode.Location | undefined> {
        // Extrait le nom de la variable/propriété (dernier segment)
        const segments = objectPath.split('.');
        const varName = segments[segments.length - 1];

        // Fichier runtime : la variable et l'import de sa classe peuvent vivre
        // dans un autre fichier du même programme (voir runtimecontext.ts).
        const typeName = findVariableType(document, varName)
            ?? (await this.inRuntimeContext(document, doc => findVariableType(doc, varName)));
        if (!typeName) { return undefined; }

        return (await this.findMemberViaImports(document, typeName, methodName))
            ?? (await this.inRuntimeContext(document, doc => this.findMemberViaImports(doc, typeName, methodName)));
    }

    /** Premier résultat de `lookup` sur les documents du contexte runtime de `document`. */
    private async inRuntimeContext<T>(
        document: vscode.TextDocument,
        lookup: (doc: vscode.TextDocument) => T | undefined | Promise<T | undefined>
    ): Promise<T | undefined> {
        for (const doc of await runtimeContext(document)) {
            const found = await lookup(doc);
            if (found !== undefined) { return found; }
        }
        return undefined;
    }

    /** Méthode `methodName` de la classe `typeName`, résolue via les imports de `document` (ou localement). */
    private async findMemberViaImports(
        document: vscode.TextDocument,
        typeName: string,
        methodName: string
    ): Promise<vscode.Location | undefined> {
        for (const imp of parseFileImports(document)) {
            const match = imp.alias === typeName || (!imp.alias && imp.symbol === typeName) || imp.symbol === '*';
            if (match) {
                const targetUri = await resolveFileImportUri(document, imp.filePath);
                const memberLoc = targetUri ? this.findMemberInFile(targetUri, methodName) : undefined;
                if (memberLoc) { return memberLoc; }
            }
        }
        for (const imp of parseImports(document)) {
            const match = imp.alias === typeName || (!imp.alias && imp.lastName === typeName);
            if (match) {
                const loc = resolveImportPath(document, imp.importPath);
                const memberLoc = loc ? this.findMemberInFile(loc.uri, methodName) : undefined;
                if (memberLoc) { return memberLoc; }
            }
        }
        // Classe locale (déclarée dans ce document)
        return this.findMemberInFile(document.uri, methodName);
    }

    // ─── Trouve un membre (méthode/fonction) dans un fichier ──────────────────

    private findMemberInFile(uri: vscode.Uri, memberName: string): vscode.Location | undefined {
        if (!fs.existsSync(uri.fsPath)) { return undefined; }
        
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

    // ─── Résolution d'un import runtime ───────────────────────────────────────

    /**
     * Résout un import runtime vers le fichier correspondant.
     * Cherche dans l'ordre : .runtime.oc, .run.oc, .rt.oc, .oc
     * Supporte les chemins avec points (ex: config.prod → config/prod.runtime.oc)
     */
    private async resolveRuntimeImport(
        document: vscode.TextDocument,
        runtimePath: string
    ): Promise<vscode.Location[] | undefined> {
        const docDir = path.dirname(document.uri.fsPath);
        const segments = runtimePath.split('.');
        const relPath = segments.join('/');
        
        // Extensions à essayer dans l'ordre
        const extensions = ['.runtime.oc', '.run.oc', '.rt.oc', '.oc'];
        
        // Essaye chaque extension
        for (const ext of extensions) {
            const fullPath = path.resolve(docDir, relPath + ext);
            if (fs.existsSync(fullPath)) {
                return [new vscode.Location(vscode.Uri.file(fullPath), new vscode.Position(0, 0))];
            }
        }
        
        // Si pas trouvé localement, cherche dans le workspace
        const ws = vscode.workspace.getWorkspaceFolder(document.uri);
        if (!ws) { return undefined; }
        
        for (const ext of extensions) {
            const fileName = path.basename(relPath) + ext;
            const pattern = new vscode.RelativePattern(ws, `**/${fileName}`);
            const files = await vscode.workspace.findFiles(pattern, '**/node_modules/**');
            
            if (files.length > 0) {
                return [new vscode.Location(files[0], new vscode.Position(0, 0))];
            }
        }
        
        return undefined;
    }

    // ─── Navigation vers un fichier importé avec from ─────────────────────────

    /**
     * Résout un import avec syntaxe from : import Symbol from "file"
     * Gère les chemins relatifs (../, ../../) et scanne le workspace.
     */
    private async resolveFileImport(
        document: vscode.TextDocument,
        symbol: string,
        filePath: string
    ): Promise<vscode.Location | undefined> {
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
        if (!ws) { return undefined; }
        
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

    private findSymbolInFile(absolutePath: string, symbol: string): vscode.Location {
        const targetUri = vscode.Uri.file(absolutePath);
        
        // Si wildcard (*), ouvre au début du fichier
        if (symbol === '*') {
            return new vscode.Location(targetUri, new vscode.Position(0, 0));
        }
        
        // Sinon, cherche la définition du symbole dans le fichier cible
        const content = fs.readFileSync(absolutePath, 'utf8');
        const lines = content.split('\n');
        
        // Cherche class, generic, interface, function, module, enum
        const symbolRe = new RegExp(`\\b(?:generic|class|struct|interface|function|module|enum)\\s+(${esc(symbol)})\\b`);
        
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

    private async resolveTypeName(
        document: vscode.TextDocument,
        name: string,
        position: vscode.Position
    ): Promise<vscode.Definition | undefined> {
        const imports = parseImports(document);
        const fileImports = parseFileImports(document);

        // Cherche d'abord dans les imports from (priorité car plus explicite)
        for (const imp of fileImports) {
            // Correspondance par alias
            if (imp.alias === name) {
                const loc = await this.resolveFileImport(document, imp.symbol, imp.filePath);
                if (loc) { return [loc]; }
            }
            // Correspondance par symbole (si pas d'alias)
            if (!imp.alias && imp.symbol === name) {
                const loc = await this.resolveFileImport(document, imp.symbol, imp.filePath);
                if (loc) { return [loc]; }
            }
            // Wildcard : tous les symboles sont disponibles
            if (imp.symbol === '*') {
                const loc = await this.resolveFileImport(document, name, imp.filePath);
                if (loc) { return [loc]; }
            }
        }

        // Imports namespace : alias d'abord, puis dernier segment (sans alias)
        // — positionne sur la déclaration elle-même, pas en tête de fichier.
        const namespaceImport = imports.find(imp => imp.alias === name)
            ?? imports.find(imp => !imp.alias && imp.lastName === name);
        if (namespaceImport) {
            const loc = resolveImportPath(document, namespaceImport.importPath);
            if (loc) { return [this.findSymbolInFile(loc.uri.fsPath, namespaceImport.lastName)]; }
        }

        // Fallback : déclaration locale (class / interface / module / enum)
        return this.findTypeDeclaration(document, name, position);
    }

    // ─── Résolution d'un identifiant minuscule ────────────────────────────────

    private resolveIdentifier(
        document: vscode.TextDocument,
        name: string,
        position: vscode.Position
    ): vscode.Location | undefined {

        // Patterns de déclaration de variable / propriété / constante
        const varDeclRe   = new RegExp(`\\b(?:var|scoped|const|property)\\s+(${esc(name)})\\s*:`);
        // Patterns de déclaration de fonction / méthode
        const funcDeclRe  = new RegExp(`\\b(?:function|method)\\s+(${esc(name)})\\s*\\(`);
        // Pattern de paramètre sur une ligne de déclaration de fonction
        const paramRe     = new RegExp(`\\b(${esc(name)})\\s*:`);

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
            if (!/\b(?:function|method|init|nameless)\b/.test(text)) { continue; }
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

    private findTypeDeclaration(
        document: vscode.TextDocument,
        name: string,
        position: vscode.Position
    ): vscode.Location | undefined {
        const re = new RegExp(`\\b(?:generic|class|struct|interface|module|enum)\\s+(${esc(name)})\\b`);
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            const m    = text.match(re);
            if (m && m.index !== undefined) {
                const col = this.findWordCol(text, name, m.index);
                if (col >= 0 && !this.isSamePosition(i, col, position)) {
                    return new vscode.Location(document.uri, new vscode.Position(i, col));
                }
            }
        }
        return undefined;
    }

    // ─── Helpers ──────────────────────────────────────────────────────────────

    /** Trouve la colonne d'un mot dans `text` en partant de `fromIdx`. */
    private findWordCol(text: string, word: string, fromIdx: number): number {
        const idx = text.indexOf(word, fromIdx);
        if (idx < 0) { return -1; }
        // Vérifie qu'il s'agit d'un mot entier (pas partie d'un autre identifiant)
        const before = text[idx - 1];
        const after  = text[idx + word.length];
        const isWordChar = (c: string | undefined) => c !== undefined && /\w/.test(c);
        if (isWordChar(before) || isWordChar(after)) { return -1; }
        return idx;
    }

    /** Retourne true si la position (line, col) correspond au curseur. */
    private isSamePosition(line: number, col: number, position: vscode.Position): boolean {
        return line === position.line && col === position.character;
    }
}
