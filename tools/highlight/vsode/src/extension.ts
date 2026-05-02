import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';

export function activate(context: vscode.ExtensionContext): void {
    const selector: vscode.DocumentSelector = { language: 'ocara', scheme: 'file' };
    context.subscriptions.push(
        vscode.languages.registerDefinitionProvider(selector, new OcaraDefinitionProvider())
    );
}

export function deactivate(): void {}

// ─── Structures internes ──────────────────────────────────────────────────────

interface ImportEntry {
    /** Chemin complet : "controllers.HomeController" */
    importPath: string;
    /** Alias déclaré (`as Alias`) ou undefined */
    alias: string | undefined;
    /** Dernier segment : "HomeController" */
    lastName: string;
    /** Numéro de ligne 0-indexé dans le document */
    line: number;
}

interface FileImportEntry {
    /** Symbole importé : "ClassName" ou "*" pour wildcard */
    symbol: string;
    /** Chemin du fichier : "file" ou "../file" */
    filePath: string;
    /** Alias déclaré (`as Alias`) ou undefined */
    alias: string | undefined;
    /** Numéro de ligne 0-indexé dans le document */
    line: number;
}

// ─── Provider ────────────────────────────────────────────────────────────────

class OcaraDefinitionProvider implements vscode.DefinitionProvider {

    provideDefinition(
        document: vscode.TextDocument,
        position: vscode.Position,
        _token: vscode.CancellationToken
    ): vscode.ProviderResult<vscode.Definition> {

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
        if (!wordRange) { return undefined; }
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

    private async resolveStaticMember(
        document: vscode.TextDocument,
        className: string,
        memberName: string
    ): Promise<vscode.Location | undefined> {
        // Cherche le fichier de la classe via les imports from d'abord
        const fileImports = this.parseFileImports(document);
        for (const imp of fileImports) {
            const match = imp.alias === className || (!imp.alias && imp.symbol === className) || imp.symbol === '*';
            if (match) {
                const targetUri = await this.resolveFileImportUri(document, imp.filePath);
                if (targetUri) {
                    const memberLoc = this.findMemberInFile(targetUri, memberName);
                    if (memberLoc) { return memberLoc; }
                    // Si pas trouvé, ouvre au moins le fichier
                    return new vscode.Location(targetUri, new vscode.Position(0, 0));
                }
            }
        }

        // Cherche ensuite via les imports namespace
        const imports = this.parseImports(document);
        let targetFile: string | undefined;

        for (const imp of imports) {
            const match = imp.alias === className || (!imp.alias && imp.lastName === className);
            if (match) {
                const loc = this.resolveImportPath(document, imp.importPath);
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
        
        // Trouve le type de cette variable dans le document
        const typeName = this.findVariableType(document, varName);
        if (!typeName) { return undefined; }
        
        // Cherche le fichier de cette classe via les imports from
        const fileImports = this.parseFileImports(document);
        for (const imp of fileImports) {
            const match = imp.alias === typeName || (!imp.alias && imp.symbol === typeName) || imp.symbol === '*';
            if (match) {
                const targetUri = await this.resolveFileImportUri(document, imp.filePath);
                if (targetUri) {
                    const memberLoc = this.findMemberInFile(targetUri, methodName);
                    if (memberLoc) { return memberLoc; }
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
                    if (memberLoc) { return memberLoc; }
                }
            }
        }
        
        // Cherche dans le fichier courant (classe locale)
        const memberLoc = this.findMemberInFile(document.uri, methodName);
        if (memberLoc) { return memberLoc; }
        
        return undefined;
    }

    // ─── Trouve le type d'une variable/propriété ──────────────────────────────

    private findVariableType(document: vscode.TextDocument, varName: string): string | undefined {
        // Cherche les déclarations de propriétés : private/public/property name:Type ou name:Generic<T>
        const propertyRe = new RegExp(`\\b(?:private|public)?\\s*property\\s+(${esc(varName)})\\s*:\\s*([A-Z]\\w*)(?:<[^>]+>)?`);
        
        // Cherche les déclarations de variables : var name:Type ou var name:Generic<T>
        const varRe = new RegExp(`\\b(?:var|scoped|const)\\s+(${esc(varName)})\\s*:\\s*([A-Z]\\w*)(?:<[^>]+>)?`);
        
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            
            // Propriété
            let m = text.match(propertyRe);
            if (m && m[2]) {
                return m[2]; // Retourne le type de base (sans les arguments génériques)
            }
            
            // Variable
            m = text.match(varRe);
            if (m && m[2]) {
                return m[2]; // Retourne le type de base (sans les arguments génériques)
            }
        }
        
        return undefined;
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

    // ─── Résout l'URI d'un fichier importé avec from ──────────────────────────

    private async resolveFileImportUri(document: vscode.TextDocument, filePath: string): Promise<vscode.Uri | undefined> {
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
        if (!ws) { return undefined; }
        
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
    private resolveImportPath(
        document: vscode.TextDocument,
        importPath: string
    ): vscode.Location | undefined {
        if (importPath.startsWith('ocara.')) { return undefined; }

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
        let searchRoot: string;
        if (currentNamespace) {
            const dirName = path.basename(docDir);
            if (dirName === currentNamespace) {
                // On est dans un dossier nommé comme le namespace, remonter d'un niveau
                searchRoot = path.dirname(docDir);
            } else {
                searchRoot = docDir;
            }
        } else {
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

    private findSymbolInFile(absolutePath: string, symbol: string): vscode.Location | undefined {
        const targetUri = vscode.Uri.file(absolutePath);
        
        // Si wildcard (*), ouvre au début du fichier
        if (symbol === '*') {
            return new vscode.Location(targetUri, new vscode.Position(0, 0));
        }
        
        // Sinon, cherche la définition du symbole dans le fichier cible
        const content = fs.readFileSync(absolutePath, 'utf8');
        const lines = content.split('\n');
        
        // Cherche class, generic, interface, function, module, enum
        const symbolRe = new RegExp(`\\b(?:generic|class|interface|function|module|enum)\\s+(${esc(symbol)})\\b`);
        
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
        const imports = this.parseImports(document);
        const fileImports = this.parseFileImports(document);

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

        // Cherche ensuite une correspondance par alias dans les imports namespace
        for (const imp of imports) {
            if (imp.alias === name) {
                const loc = this.resolveImportPath(document, imp.importPath);
                if (loc) { return [loc]; }
            }
        }

        // Cherche ensuite par nom de dernier segment (sans alias)
        for (const imp of imports) {
            if (!imp.alias && imp.lastName === name) {
                const loc = this.resolveImportPath(document, imp.importPath);
                if (loc) { return [loc]; }
            }
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
        const re = new RegExp(`\\b(?:generic|class|interface|module|enum)\\s+(${esc(name)})\\b`);
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

    // ─── Parse les imports du document ────────────────────────────────────────

    private parseImports(document: vscode.TextDocument): ImportEntry[] {
        const entries: ImportEntry[] = [];
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            const m    = text.match(/^\s*import\s+([\w.]+)(?:\s+as\s+(\w+))?\s*$/);
            if (!m) { continue; }
            const importPath = m[1];
            const alias      = m[2] as string | undefined;
            const segs       = importPath.split('.');
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

    private parseFileImports(document: vscode.TextDocument): FileImportEntry[] {
        const entries: FileImportEntry[] = [];
        for (let i = 0; i < document.lineCount; i++) {
            const text = document.lineAt(i).text;
            // Match: import Symbol from "file" [as Alias]
            const m = text.match(/^\s*import\s+([\w*]+)\s+from\s+"([^"]+)"(?:\s+as\s+(\w+))?\s*$/);
            if (!m) { continue; }
            const symbol = m[1];
            const filePath = m[2];
            const alias = m[3] as string | undefined;
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
    private parseNamespace(document: vscode.TextDocument): string | null {
        // Cherche la première ligne non-vide et non-commentaire
        for (let i = 0; i < Math.min(5, document.lineCount); i++) {
            const text = document.lineAt(i).text.trim();
            if (!text || text.startsWith('//')) { continue; }
            
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

// ─── Utilitaire ───────────────────────────────────────────────────────────────

/** Échappe les caractères spéciaux pour usage dans une RegExp. */
function esc(s: string): string {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
