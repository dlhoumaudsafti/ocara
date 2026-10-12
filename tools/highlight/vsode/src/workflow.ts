import * as vscode from 'vscode';
import * as path from 'path';
import { languageClient } from './lspclient';

// ─────────────────────────────────────────────────────────────────────────────
// Encart « Workflow » (barre latérale Ocara), en partant du script affiché :
// imports (récursifs, runtimes compris), structure (types, membres,
// variables typées), graphe d'appels et variantes `when`. Données de la
// requête `ocara/workflow` du serveur de langage (src/lsp/workflow.rs).
// ─────────────────────────────────────────────────────────────────────────────

interface WfNode {
    label: string;
    description: string;
    kind: string;
    file: string | null;
    line: number;
    children: WfNode[];
    /** Déclaré dans une variante `when` écartée pour la configuration active. */
    inactive?: boolean;
}

interface CallNode {
    label: string;
    file: string;
    line: number;
    callees: string[];
    inactive?: boolean;
    /** Type propriétaire (`runtime main` pour un bloc runtime), genre, signature. */
    owner?: string | null;
    kind?: string;
    name?: string;
    params?: string;
    ret?: string;
    static?: boolean;
    /** Contexte et couche hexagonaux du fichier (architecture du projet). */
    context?: string | null;
    layer?: string | null;
}

export interface Workflow {
    file: string;
    /** Place du script dans l'architecture hexagonale (« car · infrastructure (infra) »). */
    place?: string | null;
    imports: WfNode[];
    structure: WfNode[];
    calls: { roots: string[]; nodes: Record<string, CallNode> };
    variants: WfNode[];
}

type Item =
    | { type: 'script' }
    | { type: 'section'; id: string; label: string; icon: string }
    | { type: 'node'; node: WfNode }
    | { type: 'call'; key: string; ancestors: string[] };

const ICONS: Record<string, string> = {
    builtin: 'library', import: 'file-code', runtime: 'play-circle', missing: 'warning',
    class: 'symbol-class', struct: 'symbol-structure', interface: 'symbol-interface', module: 'symbol-module',
    enum: 'symbol-enum', 'enum-member': 'symbol-enum-member', constant: 'symbol-constant',
    property: 'symbol-property', method: 'symbol-method', 'static-method': 'symbol-method',
    function: 'symbol-function', constructor: 'symbol-constructor', variable: 'symbol-variable',
    active: 'pass', inactive: 'circle-slash',
};

/** Libellés estompés : nœuds des variantes `when` écartées (décoration). */
const INACTIVE = vscode.Uri.parse('ocara-workflow:/inactive');

class InactiveDecorations implements vscode.FileDecorationProvider {
    provideFileDecoration(uri: vscode.Uri): vscode.FileDecoration | undefined {
        if (uri.scheme !== INACTIVE.scheme) { return undefined; }
        return { color: new vscode.ThemeColor('disabledForeground'), tooltip: 'Variante when écartée pour la configuration active' };
    }
}

export class WorkflowProvider implements vscode.TreeDataProvider<Item> {
    private readonly changed = new vscode.EventEmitter<void>();
    readonly onDidChangeTreeData = this.changed.event;
    private readonly loaded = new vscode.EventEmitter<Workflow>();
    /** Données reçues du serveur (graphe d'appels). */
    readonly onData = this.loaded.event;
    private data: Workflow | undefined;
    private timer: NodeJS.Timeout | undefined;

    constructor(private readonly extensionUri: vscode.Uri) {}

    register(context: vscode.ExtensionContext): void {
        const view = vscode.window.createTreeView('ocara.workflow', { treeDataProvider: this, showCollapseAll: true });
        const schedule = (): void => this.schedule();
        context.subscriptions.push(
            view,
            vscode.window.registerFileDecorationProvider(new InactiveDecorations()),
            vscode.commands.registerCommand('ocara.workflow.refresh', () => this.refresh()),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (['ocara.os', 'ocara.osOther', 'ocara.arch', 'ocara.archOther', 'ocara.build'].some(s => e.affectsConfiguration(s))) { schedule(); }
            }),
            vscode.window.onDidChangeActiveTextEditor(schedule),
            vscode.workspace.onDidSaveTextDocument(schedule),
            vscode.languages.onDidChangeDiagnostics(e => {
                const doc = vscode.window.activeTextEditor?.document;
                if (doc && e.uris.some(u => u.toString() === doc.uri.toString())) { schedule(); }
            }),
        );
        schedule();
    }

    /** Rafraîchissement regroupé (frappe, diagnostics, changement d'éditeur). */
    private schedule(): void {
        if (this.timer) { clearTimeout(this.timer); }
        this.timer = setTimeout(() => void this.refresh(), 400);
    }

    private async refresh(): Promise<void> {
        const doc = vscode.window.activeTextEditor?.document;
        const client = languageClient();
        if (!doc || doc.languageId !== 'ocara' || !client) { return; }
        try {
            this.data = await client.sendRequest<Workflow>('ocara/workflow', { textDocument: { uri: doc.uri.toString() } });
        } catch {
            return;
        }
        this.changed.fire();
        this.loaded.fire(this.data);
    }

    getChildren(item?: Item): Item[] {
        const data = this.data;
        if (!data) { return []; }
        if (!item) {
            return [
                { type: 'script' },
                { type: 'section', id: 'imports', label: 'Imports', icon: 'references' },
                { type: 'section', id: 'structure', label: 'Structure', icon: 'symbol-namespace' },
                { type: 'section', id: 'calls', label: 'Appels', icon: 'call-outgoing' },
                { type: 'section', id: 'variants', label: 'Variantes when', icon: 'git-compare' },
            ];
        }
        switch (item.type) {
            case 'script':
                return [];
            case 'section':
                if (item.id === 'calls') {
                    return data.calls.roots.map(key => ({ type: 'call', key, ancestors: [] }));
                }
                return (data[item.id as 'imports' | 'structure' | 'variants'] ?? []).map(node => ({ type: 'node', node }));
            case 'node':
                return item.node.children.map(node => ({ type: 'node', node }));
            case 'call': {
                // Un appel récursif n'est pas redéplié.
                if (item.ancestors.includes(item.key)) { return []; }
                const ancestors = [...item.ancestors, item.key];
                return (data.calls.nodes[item.key]?.callees ?? []).map(key => ({ type: 'call', key, ancestors }));
            }
        }
    }

    getTreeItem(item: Item): vscode.TreeItem {
        const data = this.data!;
        if (item.type === 'script') {
            // Script de départ, avec le logo Ocara du thème (clair / sombre).
            const t = new vscode.TreeItem(path.basename(data.file));
            t.description = data.place ?? vscode.workspace.asRelativePath(path.dirname(data.file));
            t.tooltip = `${data.file}${data.place ? ` — ${data.place}` : ''}`;
            t.iconPath = {
                light: vscode.Uri.joinPath(this.extensionUri, 'logo-min-black.png'),
                dark: vscode.Uri.joinPath(this.extensionUri, 'logo-min-white.png'),
            };
            t.command = openCommand(data.file, 1);
            return t;
        }
        if (item.type === 'section') {
            const t = new vscode.TreeItem(item.label, vscode.TreeItemCollapsibleState.Expanded);
            t.iconPath = new vscode.ThemeIcon(item.icon);
            return t;
        }
        if (item.type === 'call') {
            const node = data.calls.nodes[item.key];
            const recursive = item.ancestors.includes(item.key);
            const leaf = recursive || !node?.callees.length;
            const t = new vscode.TreeItem(node?.label ?? item.key, leaf ? vscode.TreeItemCollapsibleState.None : vscode.TreeItemCollapsibleState.Collapsed);
            t.iconPath = new vscode.ThemeIcon(recursive ? 'sync' : 'symbol-method', node?.inactive ? new vscode.ThemeColor('disabledForeground') : undefined);
            t.description = recursive ? 'récursif' : node ? `${path.basename(node.file)}:${node.line}` : '';
            if (node) { t.command = openCommand(node.file, node.line); }
            if (node?.inactive) { dim(t); }
            return t;
        }
        const node = item.node;
        const state = node.children.length ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None;
        const t = new vscode.TreeItem(node.label, state);
        t.description = node.description;
        t.tooltip = node.description ? `${node.label} — ${node.description}` : node.label;
        const color = node.kind === 'active' ? new vscode.ThemeColor('testing.iconPassed') : node.kind === 'missing' ? new vscode.ThemeColor('problemsWarningIcon.foreground') : undefined;
        t.iconPath = new vscode.ThemeIcon(ICONS[node.kind] ?? 'circle-small', color);
        if (node.file) { t.command = openCommand(node.file, node.line); }
        if (node.inactive || node.kind === 'inactive') { dim(t); }
        return t;
    }
}

/** Nœud d'une variante `when` écartée : libellé et icône estompés. */
function dim(t: vscode.TreeItem): void {
    t.resourceUri = INACTIVE;
    if (t.iconPath instanceof vscode.ThemeIcon) { t.iconPath = new vscode.ThemeIcon(t.iconPath.id, new vscode.ThemeColor('disabledForeground')); }
    t.tooltip = `${t.tooltip ?? t.label} — variante when écartée pour la configuration active`;
}

/** Ouverture enregistrée dans l'historique de navigation (navigation.ts). */
function openCommand(file: string, line: number): vscode.Command {
    return { command: 'ocara.nav.open', title: 'Ouvrir', arguments: [file, line] };
}
