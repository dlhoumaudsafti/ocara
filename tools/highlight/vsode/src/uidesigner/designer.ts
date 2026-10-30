import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { parseToolMessages, runTool } from '../toolrunner';
import { Catalog, loadCatalog } from './catalog';
import { applyOps, minimalEdit, Op, parse, pathAt } from './yamlops';
import { html } from './html';

// ─────────────────────────────────────────────────────────────────────────────
// Designer des fichiers `.oc.ui` : éditeur personnalisé sur le même document
// texte que l'éditeur YAML (bouton en haut à droite pour basculer, comme
// l'aperçu Markdown). La webview (media/uidesigner/) affiche l'écran avec
// l'apparence du moteur desktop, la palette des composants, l'arbre et les
// propriétés ; chaque modification revient ici en opérations (yamlops.ts)
// appliquées au texte, donc annulables (Ctrl+Z) et enregistrées comme une
// frappe. La vérification du compilateur (`ocara x.oc.ui`) tourne à
// l'ouverture et à chaque enregistrement ; ses erreurs sont posées sur les
// nœuds concernés.
// ─────────────────────────────────────────────────────────────────────────────

export const VIEW_TYPE = 'ocara.uiDesigner';

/** Classes de taille (docs/oc-ui.md, « Tailles d'écran ») : variantes `x.<classe>.oc.ui`. */
const SIZE_CLASSES = ['xxs', 'xs', 'sm', 'md', 'lg', 'xl', 'xxl'];

type Message =
    | { type: 'ready' }
    | { type: 'edit'; ops: Op[] }
    | { type: 'open'; file: string; designer: boolean }
    | { type: 'source' }
    | { type: 'variant'; size: string }
    | { type: 'check' }
    | { type: 'reveal'; path: (string | number)[] };

interface Problem { severity: string; message: string; line: number; file: string; path?: (string | number)[] }

export class UiDesigner implements vscode.CustomTextEditorProvider {
    private catalog: Catalog | undefined;

    constructor(private readonly extensionUri: vscode.Uri) {}

    register(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
            vscode.window.registerCustomEditorProvider(VIEW_TYPE, this, {
                webviewOptions: { retainContextWhenHidden: true },
                supportsMultipleEditorsPerDocument: true,
            }),
            vscode.commands.registerCommand('ocara.ui.openDesigner', (uri?: vscode.Uri) => this.reopen(uri, VIEW_TYPE)),
            vscode.commands.registerCommand('ocara.ui.openDesignerToSide', (uri?: vscode.Uri) => this.reopen(uri, VIEW_TYPE, true)),
            vscode.commands.registerCommand('ocara.ui.openSource', (uri?: vscode.Uri) => this.reopen(uri, 'default')),
        );
    }

    /** Rouvre le fichier affiché avec l'éditeur `viewType` (designer ou texte). */
    private async reopen(uri: vscode.Uri | undefined, viewType: string, side = false): Promise<void> {
        const target = uri ?? activeUri();
        if (!target) { return; }
        await vscode.commands.executeCommand('vscode.openWith', target, viewType, side ? vscode.ViewColumn.Beside : undefined);
    }

    resolveCustomTextEditor(document: vscode.TextDocument, panel: vscode.WebviewPanel): void {
        this.catalog ??= loadCatalog(vscode.Uri.joinPath(this.extensionUri, 'schemas', 'oc-ui.schema.json').fsPath);
        const media = vscode.Uri.joinPath(this.extensionUri, 'media', 'uidesigner');
        const folder = vscode.Uri.file(path.dirname(document.uri.fsPath));
        const roots = [media, folder, ...(vscode.workspace.workspaceFolders ?? []).map(f => f.uri)];
        panel.webview.options = { enableScripts: true, localResourceRoots: roots };
        panel.webview.html = html(panel.webview, media);
        let problems: Problem[] = [];
        let root: RootContext | undefined;

        const post = () => {
            const { doc } = parse(document.getText());
            void panel.webview.postMessage({
                type: 'model',
                file: path.basename(document.uri.fsPath),
                error: doc.errors[0] ? `${doc.errors[0].message}` : undefined,
                model: doc.errors.length ? undefined : doc.toJS() ?? {},
                includes: this.includes(document, doc.toJS()),
                variants: this.variants(document),
                problems,
                context: root && { root: path.relative(folder.fsPath, root.file), styles: allStyles(root.file, document.uri.fsPath), window: root.model?.window, menu: root.model?.menu },
                // Styles des autres fichiers de l'interface (racine et inclus), utilisables ici.
                shared: allStyles(root?.file ?? document.uri.fsPath, document.uri.fsPath),
            });
        };
        const check = async () => {
            root = await findRoot(document.uri.fsPath);
            problems = await this.check(document, root?.file);
            post();
        };
        const subscriptions = [
            vscode.workspace.onDidChangeTextDocument(e => { if (e.document === document && e.contentChanges.length) { post(); } }),
            vscode.workspace.onDidSaveTextDocument(d => { if (d === document) { void check(); } }),
        ];
        panel.onDidDispose(() => subscriptions.forEach(s => s.dispose()));
        panel.webview.onDidReceiveMessage(async (msg: Message) => {
            switch (msg.type) {
                case 'ready':
                    void panel.webview.postMessage({
                        type: 'init',
                        catalog: this.catalog,
                        icons: this.icons(),
                        base: panel.webview.asWebviewUri(folder).toString(),
                    });
                    post();
                    void check();
                    break;
                case 'edit':
                    await this.edit(document, msg.ops);
                    break;
                case 'open': {
                    const uri = vscode.Uri.file(path.resolve(folder.fsPath, msg.file));
                    await vscode.commands.executeCommand('vscode.openWith', uri, msg.designer ? VIEW_TYPE : 'default');
                    break;
                }
                case 'source':
                    await vscode.commands.executeCommand('vscode.openWith', document.uri, 'default');
                    break;
                case 'variant':
                    await this.variant(document, msg.size);
                    break;
                case 'check':
                    if (document.isDirty) { await document.save(); } else { await check(); }
                    break;
                case 'reveal':
                    await this.reveal(document, msg.path);
                    break;
            }
        });
    }

    private async edit(document: vscode.TextDocument, ops: Op[]): Promise<void> {
        const text = document.getText();
        const result = applyOps(text, ops);
        if ('error' in result) {
            vscode.window.showErrorMessage(`Ocara UI : YAML invalide, modification impossible — ${result.error}`);
            return;
        }
        const change = minimalEdit(text, result.text);
        if (change.start === change.end && change.text === '') { return; }
        const edit = new vscode.WorkspaceEdit();
        edit.replace(document.uri, new vscode.Range(document.positionAt(change.start), document.positionAt(change.end)), change.text);
        await vscode.workspace.applyEdit(edit);
    }

    /** Écrans des fichiers inclus (`screens.includes`), en lecture seule ici. */
    private includes(document: vscode.TextDocument, model: any): { file: string; screens: string[] }[] {
        const list = model?.screens?.includes;
        if (!Array.isArray(list)) { return []; }
        const dir = path.dirname(document.uri.fsPath);
        return list.filter(f => typeof f === 'string').map(file => {
            try {
                const included = parse(fs.readFileSync(path.resolve(dir, file), 'utf8')).doc.toJS();
                return { file, screens: Object.keys(included?.screens ?? {}).filter(s => s !== 'includes') };
            } catch {
                return { file, screens: [] };
            }
        });
    }

    /** Variantes de taille présentes à côté du fichier (`x.md.oc.ui`…). */
    private variants(document: vscode.TextDocument): { size: string; file: string; current: boolean }[] {
        const name = path.basename(document.uri.fsPath);
        const base = name.replace(new RegExp(`(\\.(${SIZE_CLASSES.join('|')}))?\\.oc\\.ui$`), '');
        const dir = path.dirname(document.uri.fsPath);
        return ['', ...SIZE_CLASSES].map(size => {
            const file = `${base}${size ? '.' + size : ''}.oc.ui`;
            return { size, file, current: file === name, exists: fs.existsSync(path.join(dir, file)) };
        }).filter(v => v.exists).map(({ size, file, current }) => ({ size, file, current }));
    }

    /** Crée la variante `size` (copie du fichier par défaut) et l'ouvre. */
    private async variant(document: vscode.TextDocument, size: string): Promise<void> {
        const dir = path.dirname(document.uri.fsPath);
        const defaults = this.variants(document).find(v => v.size === '');
        const name = path.basename(document.uri.fsPath).replace(/(\.[a-z]+)?\.oc\.ui$/, '');
        const target = path.join(dir, `${(defaults?.file ?? name).replace(/\.oc\.ui$/, '')}.${size}.oc.ui`);
        if (!fs.existsSync(target)) {
            const source = defaults ? fs.readFileSync(path.join(dir, defaults.file), 'utf8') : document.getText();
            fs.writeFileSync(target, source);
        }
        await vscode.commands.executeCommand('vscode.openWith', vscode.Uri.file(target), VIEW_TYPE);
    }

    /** Erreurs du compilateur sur le fichier enregistré, rattachées aux nœuds.
     *  Fichier inclus (`rootFile`) : c'est le fichier racine qui est vérifié
     *  (styles, `id` et actions partagés), et seules les erreurs de ce
     *  fichier-ci sont gardées. */
    private async check(document: vscode.TextDocument, rootFile?: string): Promise<Problem[]> {
        const dir = path.dirname(document.uri.fsPath);
        const target = rootFile ?? document.uri.fsPath;
        const result = await runTool('compilerPath', 'ocara', [target], path.dirname(target), document.uri);
        if (result.launchError) { return [{ severity: 'warning', message: `vérification indisponible (${result.launchError})`, line: 0, file: '' }]; }
        const { doc } = parse(document.getText());
        const model = doc.toJS();
        /** Nœud (table qui a un `type`) qui contient la position. */
        const nodeAt = (offset: number) => {
            const p = pathAt(doc, offset);
            while (p.length && typeof p.reduce((o: any, k) => o?.[k], model)?.type !== 'string') { p.pop(); }
            return p.length ? p : undefined;
        };
        const messages = parseToolMessages(result.stdout + result.stderr, path.dirname(target))
            .filter(m => !rootFile || path.resolve(m.file) === path.resolve(document.uri.fsPath));
        return messages.map(m => {
            const own = path.resolve(m.file) === path.resolve(document.uri.fsPath);
            const offset = own && m.line < document.lineCount ? document.offsetAt(new vscode.Position(m.line, m.col)) : -1;
            return {
                severity: m.severity, message: m.message, line: m.line + 1,
                file: own ? '' : path.relative(dir, m.file),
                path: offset >= 0 ? nodeAt(offset) : undefined,
            };
        });
    }

    /** Montre le nœud `nodePath` dans l'éditeur texte, à côté. */
    private async reveal(document: vscode.TextDocument, nodePath: (string | number)[]): Promise<void> {
        const { doc } = parse(document.getText());
        const node = doc.getIn(nodePath, true) as { range?: [number, number, number] } | undefined;
        const editor = await vscode.window.showTextDocument(document, { viewColumn: vscode.ViewColumn.Beside, preserveFocus: false });
        if (node?.range) {
            const range = new vscode.Range(document.positionAt(node.range[0]), document.positionAt(node.range[1]));
            editor.selection = new vscode.Selection(range.start, range.end);
            editor.revealRange(range, vscode.TextEditorRevealType.InCenter);
        }
    }

    /** Tracés SVG des icônes (media/uidesigner/icons.json, tiré de icons.rs au build). */
    private icons(): Record<string, string> {
        try {
            return JSON.parse(fs.readFileSync(vscode.Uri.joinPath(this.extensionUri, 'media', 'uidesigner', 'icons.json').fsPath, 'utf8'));
        } catch {
            return {};
        }
    }
}

interface RootContext { file: string; model: any }

/** Fichier racine d'un `.oc.ui` inclus (`screens.includes`, inclusions en
 *  chaîne comprises), cherché dans l'espace de travail ; `undefined` pour un
 *  fichier qui n'est inclus nulle part. */
async function findRoot(file: string): Promise<RootContext | undefined> {
    const parents = new Map<string, { file: string; model: any }>();
    for (const uri of await vscode.workspace.findFiles('**/*.oc.ui', '**/{node_modules,target}/**', 2000)) {
        let model: any;
        try { model = parse(fs.readFileSync(uri.fsPath, 'utf8')).doc.toJS(); } catch { continue; }
        const includes = model?.screens?.includes;
        if (!Array.isArray(includes)) { continue; }
        for (const inc of includes) {
            if (typeof inc !== 'string') { continue; }
            const child = path.resolve(path.dirname(uri.fsPath), inc);
            // Un défaut plutôt qu'une variante de taille (`x.md.oc.ui`) quand les deux incluent le fichier.
            const known = parents.get(child);
            if (!known || /\.[a-z]+\.oc\.ui$/.test(path.basename(known.file))) { parents.set(child, { file: uri.fsPath, model }); }
        }
    }
    let current = parents.get(path.resolve(file));
    const seen = new Set<string>();
    while (current && parents.has(current.file) && !seen.has(current.file)) {
        seen.add(current.file);
        current = parents.get(current.file);
    }
    return current;
}

/** Styles de l'interface de racine `rootFile` : ceux de la racine et des
 *  fichiers qu'elle inclut (un seul espace de noms), sauf ceux de `except`
 *  (le fichier affiché, dont les styles sont dans son propre modèle). */
function allStyles(rootFile: string, except: string, seen = new Set<string>()): Record<string, unknown> {
    const file = path.resolve(rootFile);
    if (seen.has(file)) { return {}; }
    seen.add(file);
    let model: any;
    try { model = parse(fs.readFileSync(file, 'utf8')).doc.toJS(); } catch { return {}; }
    const out: Record<string, unknown> = file === path.resolve(except) ? {} : { ...(model?.styles ?? {}) };
    for (const inc of Array.isArray(model?.screens?.includes) ? model.screens.includes : []) {
        if (typeof inc === 'string') { Object.assign(out, allStyles(path.resolve(path.dirname(file), inc), except, seen)); }
    }
    return out;
}

function activeUri(): vscode.Uri | undefined {
    const tab = vscode.window.tabGroups.activeTabGroup.activeTab?.input as { uri?: vscode.Uri } | undefined;
    return tab?.uri ?? vscode.window.activeTextEditor?.document.uri;
}
