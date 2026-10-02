import * as vscode from 'vscode';
import { Declaration, MethodDecl, maskSource, parseDeclarations } from './declarations';
import { esc } from './resolver';

// ─────────────────────────────────────────────────────────────────────────────
// CodeLens au-dessus de chaque déclaration, calculés sur tout le workspace
// (index construit à l'activation, tenu à jour à chaque modification) :
//
//   classe/generic : implémentations = sous-classes (extends, transitif),
//                    overrides = méthodes redéfinies par ces sous-classes ;
//   struct         : implémentations = structs dérivés (aucune méthode,
//                    donc aucun override) ;
//   interface      : implémentations = classes qui l'implémentent (héritage
//                    compris), overrides = méthodes de l'interface qu'elles
//                    définissent ;
//   module         : implémentations = classes qui l'utilisent (`modules`),
//                    overrides = méthodes du module qu'elles redéfinissent ;
//   méthode        : d'interface → implémentations ; de module →
//                    implémentations + overrides ; de classe → overrides ;
//   fonction/enum  : nombre de références ;
//   + références sur chaque classe/struct/generic/interface/module (usages
//   de son nom hors imports : types, `use X(`, `X::`, `extends`, `wiring`)
//   et sur chaque méthode (`Classe::m` — appel OU référence sans appel,
//   ex. `server.route("/x", "GET", CarController::show)` —, `self::m`/
//   `parent::m` dans son propre fichier, et `.m(` pour une méthode
//   d'instance, par nom).
//
// Résolution PAR NOM, sans suivre les imports (heuristique, comme le reste
// de l'extension) : deux classes homonymes de contextes différents sont
// confondues.
// ─────────────────────────────────────────────────────────────────────────────

const OC_GLOB = '**/*.oc';
const EXCLUDED_GLOB = '{**/node_modules/**,**/out/**,**/target/**}';

interface IndexedFile {
    uri: vscode.Uri;
    /** Texte masqué (chaînes/commentaires effacés), pour compter les références. */
    masked: string;
    decls: Declaration[];
}

interface RefPattern {
    re: RegExp;
    /** Ne chercher que dans ce fichier (ex. `self::m`, propre au fichier de la classe). */
    onlyIn?: vscode.Uri;
}

interface Located<T> {
    uri: vscode.Uri;
    item: T;
}

export class WorkspaceIndex {
    private files = new Map<string, IndexedFile>();
    private readonly changed = new vscode.EventEmitter<void>();
    readonly onDidChange = this.changed.event;

    async build(): Promise<void> {
        const uris = await vscode.workspace.findFiles(OC_GLOB, EXCLUDED_GLOB);
        await Promise.all(uris.map(uri => this.updateFromDisk(uri, false)));
        this.changed.fire();
    }

    watch(context: vscode.ExtensionContext): void {
        const watcher = vscode.workspace.createFileSystemWatcher(OC_GLOB);
        watcher.onDidCreate(uri => this.updateFromDisk(uri, true));
        watcher.onDidChange(uri => this.updateFromDisk(uri, true));
        watcher.onDidDelete(uri => { this.files.delete(uri.toString()); this.changed.fire(); });
        context.subscriptions.push(
            watcher,
            vscode.workspace.onDidChangeTextDocument(e => {
                if (e.document.languageId === 'ocara') { this.update(e.document.uri, e.document.getText(), true); }
            })
        );
    }

    private async updateFromDisk(uri: vscode.Uri, notify: boolean): Promise<void> {
        try {
            const bytes = await vscode.workspace.fs.readFile(uri);
            this.update(uri, Buffer.from(bytes).toString('utf8'), notify);
        } catch {
            this.files.delete(uri.toString());
        }
    }

    private update(uri: vscode.Uri, text: string, notify: boolean): void {
        this.files.set(uri.toString(), { uri, masked: maskSource(text), decls: parseDeclarations(text) });
        if (notify) { this.changed.fire(); }
    }

    typeDecls(): Located<Declaration>[] {
        const out: Located<Declaration>[] = [];
        for (const file of this.files.values()) {
            for (const decl of file.decls) {
                if (decl.kind !== 'function') { out.push({ uri: file.uri, item: decl }); }
            }
        }
        return out;
    }

    maskedFiles(): IndexedFile[] {
        return Array.from(this.files.values());
    }
}

export class OcaraCodeLensProvider implements vscode.CodeLensProvider {
    readonly onDidChangeCodeLenses: vscode.Event<void>;

    constructor(private readonly index: WorkspaceIndex) {
        this.onDidChangeCodeLenses = index.onDidChange;
    }

    provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
        const types = this.index.typeDecls();
        const lenses: vscode.CodeLens[] = [];
        for (const decl of parseDeclarations(document.getText())) {
            const at = new vscode.Position(decl.line, decl.col);
            switch (decl.kind) {
                case 'class':
                case 'generic':
                    lenses.push(...this.classLenses(document.uri, at, decl, types));
                    break;
                case 'struct':
                    lenses.push(lens(document.uri, at, 'implémentation', descendants(decl.name, types).map(declLocation)));
                    break;
                case 'interface':
                    lenses.push(...this.interfaceLenses(document.uri, at, decl, types));
                    break;
                case 'module':
                    lenses.push(...this.moduleLenses(document.uri, at, decl, types));
                    break;
                case 'enum':
                    lenses.push(this.nameReferences(document.uri, at, decl));
                    break;
                case 'function':
                    lenses.push(lens(document.uri, at, 'référence', this.references(decl.name, [{ re: new RegExp(`(?<![.:\\w])${esc(decl.name)}\\s*\\(`) }], document.uri, decl.line)));
                    break;
            }
            if (decl.kind !== 'enum' && decl.kind !== 'function') {
                lenses.push(this.nameReferences(document.uri, at, decl));
                for (const m of decl.methods) {
                    lenses.push(this.methodReferences(document.uri, decl, m));
                }
            }
        }
        return lenses;
    }

    /** Usages du nom d'un type (hors imports et déclaration). */
    private nameReferences(uri: vscode.Uri, at: vscode.Position, decl: Declaration): vscode.CodeLens {
        return lens(uri, at, 'référence', this.references(decl.name, [{ re: new RegExp(`\\b${esc(decl.name)}\\b`) }], uri, decl.line));
    }

    /** Appels et références d'une méthode (voir l'en-tête du fichier). */
    private methodReferences(uri: vscode.Uri, owner: Declaration, m: MethodDecl): vscode.CodeLens {
        const name = esc(m.name);
        const patterns: RefPattern[] = [
            { re: new RegExp(`\\b${esc(owner.name)}::${name}\\b`) },
            { re: new RegExp(`\\b(?:self|parent)::${name}\\b`), onlyIn: uri },
        ];
        if (!m.isStatic) {
            patterns.push({ re: new RegExp(`\\.${name}\\s*\\(`) });
        }
        return lens(uri, methodPosition(m), 'référence', this.references(m.name, patterns, uri, m.line));
    }

    private classLenses(uri: vscode.Uri, at: vscode.Position, decl: Declaration, types: Located<Declaration>[]): vscode.CodeLens[] {
        const subclasses = descendants(decl.name, types);
        const overrides = subclasses.flatMap(sub => decl.methods.flatMap(m => ownMethod(sub, m.name)));
        const lenses = [
            lens(uri, at, 'implémentation', subclasses.map(declLocation)),
            lens(uri, at, 'override', overrides),
        ];
        for (const m of decl.methods) {
            lenses.push(lens(uri, methodPosition(m), 'override', subclasses.flatMap(sub => ownMethod(sub, m.name))));
        }
        return lenses;
    }

    private interfaceLenses(uri: vscode.Uri, at: vscode.Position, decl: Declaration, types: Located<Declaration>[]): vscode.CodeLens[] {
        const implementers = withDescendants(types.filter(t => t.item.implementsNames.includes(decl.name)), types);
        const lenses = [
            lens(uri, at, 'implémentation', implementers.map(declLocation)),
            lens(uri, at, 'override', implementers.flatMap(c => decl.methods.flatMap(m => ownMethod(c, m.name)))),
        ];
        for (const m of decl.methods) {
            lenses.push(lens(uri, methodPosition(m), 'implémentation', implementers.flatMap(c => ownMethod(c, m.name))));
        }
        return lenses;
    }

    private moduleLenses(uri: vscode.Uri, at: vscode.Position, decl: Declaration, types: Located<Declaration>[]): vscode.CodeLens[] {
        const users = withDescendants(types.filter(t => t.item.moduleNames.includes(decl.name)), types);
        const lenses = [
            lens(uri, at, 'implémentation', users.map(declLocation)),
            lens(uri, at, 'override', users.flatMap(c => decl.methods.flatMap(m => ownMethod(c, m.name)))),
        ];
        for (const m of decl.methods) {
            const pos = methodPosition(m);
            lenses.push(
                lens(uri, pos, 'implémentation', users.map(declLocation)),
                lens(uri, pos, 'override', users.flatMap(c => ownMethod(c, m.name))),
            );
        }
        return lenses;
    }

    /**
     * Occurrences des `patterns` dans le workspace (chacun limité à `onlyIn`
     * s'il est précisé), hors déclaration elle-même et lignes `import` — une
     * position comptée une seule fois même si plusieurs motifs la couvrent.
     */
    private references(name: string, patterns: RefPattern[], declUri: vscode.Uri, declLine: number): vscode.Location[] {
        const locations: vscode.Location[] = [];
        for (const file of this.index.maskedFiles()) {
            if (!file.masked.includes(name)) { continue; }
            const active = patterns.filter(p => !p.onlyIn || p.onlyIn.toString() === file.uri.toString());
            if (active.length === 0) { continue; }
            file.masked.split('\n').forEach((line, lineNo) => {
                if (/^\s*import\b/.test(line)) { return; }
                if (lineNo === declLine && file.uri.toString() === declUri.toString()) { return; }
                const cols = new Set<number>();
                for (const p of active) {
                    for (const m of line.matchAll(new RegExp(p.re.source, 'g'))) {
                        cols.add(m.index! + m[0].lastIndexOf(name));
                    }
                }
                cols.forEach(col => locations.push(new vscode.Location(file.uri, new vscode.Position(lineNo, col))));
            });
        }
        return locations;
    }
}

function lens(uri: vscode.Uri, at: vscode.Position, noun: string, locations: vscode.Location[]): vscode.CodeLens {
    const n = locations.length;
    const title = `${n} ${noun}${n > 1 ? 's' : ''}`;
    const range = new vscode.Range(at, at);
    if (n === 0) {
        return new vscode.CodeLens(range, { title, command: '' });
    }
    return new vscode.CodeLens(range, {
        title,
        command: 'editor.action.showReferences',
        arguments: [uri, at, locations],
    });
}

function methodPosition(m: MethodDecl): vscode.Position {
    return new vscode.Position(m.line, m.col);
}

function declLocation(t: Located<Declaration>): vscode.Location {
    return new vscode.Location(t.uri, new vscode.Position(t.item.line, t.item.col));
}

function ownMethod(t: Located<Declaration>, name: string): vscode.Location[] {
    return t.item.methods
        .filter(m => m.name === name)
        .map(m => new vscode.Location(t.uri, methodPosition(m)));
}

/** Toutes les classes/generics descendant de `name` via `extends` (anti-cycle). */
function descendants(name: string, types: Located<Declaration>[]): Located<Declaration>[] {
    const result: Located<Declaration>[] = [];
    const seen = new Set<string>([name]);
    const queue = [name];
    while (queue.length > 0) {
        const current = queue.shift()!;
        for (const t of types) {
            if (t.item.extendsName === current && !seen.has(t.item.name)) {
                seen.add(t.item.name);
                result.push(t);
                queue.push(t.item.name);
            }
        }
    }
    return result;
}

function withDescendants(roots: Located<Declaration>[], types: Located<Declaration>[]): Located<Declaration>[] {
    const all = [...roots];
    for (const root of roots) {
        for (const d of descendants(root.item.name, types)) {
            if (!all.includes(d)) { all.push(d); }
        }
    }
    return all;
}
