import * as vscode from 'vscode';
import { BuiltinConst, BuiltinMethod, builtinClasses, getBuiltinClass } from './builtins';
import {
    findVariableType,
    findEnclosingClassName,
    findClassMembers,
    collectKnownClassNames,
    ClassMember,
} from './resolver';
import { findCallSite, resolveCall, remainingNamedParams, paramLabel } from './callsite';

// ─────────────────────────────────────────────────────────────────────────────
// Autocomplétion : méthodes/constantes des classes builtin `ocara.*` (données
// générées depuis src/builtins/*.rs, voir tools/highlight/vsode/data/) et des
// classes utilisateur (résolues via les imports du document, resolver.ts).
// ─────────────────────────────────────────────────────────────────────────────

/** Propriétés communes à toutes les exceptions builtin (voir src/builtins/exception.rs). */
const EXCEPTION_PROPERTIES: { name: string; type: string }[] = [
    { name: 'message', type: 'string' },
    { name: 'code', type: 'int' },
    { name: 'source', type: 'string' },
];

export class OcaraCompletionProvider implements vscode.CompletionItemProvider {

    async provideCompletionItems(
        document: vscode.TextDocument,
        position: vscode.Position,
        _token: vscode.CancellationToken,
        _context: vscode.CompletionContext
    ): Promise<vscode.CompletionItem[] | undefined> {
        const linePrefix = document.lineAt(position).text.substring(0, position.character);

        // ── ClassName::member  (et self::member / parent::member) ─────────────
        const staticMatch = linePrefix.match(/([A-Za-z_]\w*)::\w*$/);
        if (staticMatch) {
            let className = staticMatch[1];
            if (className === 'self' || className === 'parent') {
                className = findEnclosingClassName(document, position) ?? className;
            }
            return this.completeStatic(document, className);
        }

        // ── variable.member  (et self.member) ──────────────────────────────────
        const instanceMatch = linePrefix.match(/(?:^|[^.\w])([a-zA-Z_]\w*)\.\w*$/);
        if (instanceMatch) {
            let varName = instanceMatch[1];
            let className: string | undefined;
            if (varName === 'self' || varName === 'parent') {
                className = findEnclosingClassName(document, position);
            } else {
                className = findVariableType(document, varName);
            }
            if (!className) { return undefined; }
            return this.completeInstance(document, className);
        }

        // ── use ClassName(  — complétion du nom de classe instanciable ────────
        const useMatch = linePrefix.match(/\buse\s+\w*$/);
        if (useMatch) {
            return this.completeClassNames(document);
        }

        // ── f(nom: ...) — noms des paramètres de la cible de l'appel ────────
        if (/(?:^|[(,])\s*\w*$/.test(linePrefix)) {
            return this.completeArgumentNames(document, position);
        }

        return undefined;
    }

    // ─── f(nom: valeur) ─────────────────────────────────────────────────────

    /**
     * Un appel est soit 100 % positionnel, soit 100 % nommé : rien à proposer
     * dès qu'un argument précédent est positionnel. Variadic et noms déjà
     * fournis exclus (voir docs/roadmap.d/langage-named-arguments.md).
     */
    private async completeArgumentNames(document: vscode.TextDocument, position: vscode.Position): Promise<vscode.CompletionItem[] | undefined> {
        const site = findCallSite(document, position);
        if (!site || site.positionalCount > 0 || site.currentName !== undefined) { return undefined; }
        const call = await resolveCall(document, position, site);
        if (!call) { return undefined; }
        return remainingNamedParams(call, site).map((p, i) => {
            const item = new vscode.CompletionItem(`${p.name}:`, vscode.CompletionItemKind.Property);
            item.insertText = `${p.name}: `;
            item.filterText = p.name;
            item.sortText = String(i).padStart(3, '0');
            item.detail = paramLabel(p);
            item.documentation = new vscode.MarkdownString(
                `Argument nommé de \`${call.owner}\`${p.defaultValue !== undefined ? ' — optionnel' : ''}`
            );
            return item;
        });
    }

    // ─── ClassName::membre ──────────────────────────────────────────────────

    private async completeStatic(document: vscode.TextDocument, className: string): Promise<vscode.CompletionItem[]> {
        const builtin = getBuiltinClass(className);
        if (builtin) {
            const items: vscode.CompletionItem[] = [];
            for (const c of builtin.consts) {
                items.push(this.builtinConstItem(className, c, true));
            }
            for (const m of builtin.methods.filter(x => x.static)) {
                items.push(this.builtinMethodItem(className, m, true));
            }
            return items;
        }

        const members = await findClassMembers(document, className);
        return members.filter(m => m.isStatic).map(m => this.memberItem(className, m));
    }

    // ─── variable.membre ────────────────────────────────────────────────────

    private async completeInstance(document: vscode.TextDocument, className: string): Promise<vscode.CompletionItem[]> {
        // Exceptions builtin : message/code/source, jamais de méthode.
        if (className === 'Exception' || className.endsWith('Exception')) {
            return EXCEPTION_PROPERTIES.map(p => this.exceptionPropertyItem(className, p));
        }

        const builtin = getBuiltinClass(className);
        if (builtin) {
            return builtin.methods.filter(m => !m.static).map(m => this.builtinMethodItem(className, m, false));
        }

        const members = await findClassMembers(document, className);
        return members.filter(m => !m.isStatic).map(m => this.memberItem(className, m));
    }

    // ─── use ClassName(...) ─────────────────────────────────────────────────

    private completeClassNames(document: vscode.TextDocument): vscode.CompletionItem[] {
        const items: vscode.CompletionItem[] = [];
        const seen = new Set<string>();

        for (const c of builtinClasses()) {
            const hasInstanceApi = c.methods.some(m => !m.static);
            if (!hasInstanceApi) { continue; } // classes purement statiques : jamais `use X()`
            const item = new vscode.CompletionItem(c.name, vscode.CompletionItemKind.Class);
            item.detail = `ocara.${c.name}`;
            item.insertText = new vscode.SnippetString(`${c.name}($1)`);
            items.push(item);
            seen.add(c.name);
        }

        for (const name of collectKnownClassNames(document)) {
            if (seen.has(name)) { continue; }
            // Builtin purement statique (ex: IO, Math, Array) importé par son nom
            // (`import ocara.IO`) : jamais instanciable via `use`, on l'exclut.
            const builtin = getBuiltinClass(name);
            if (builtin && !builtin.methods.some(m => !m.static)) { continue; }
            seen.add(name);
            const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Class);
            item.insertText = new vscode.SnippetString(`${name}($1)`);
            items.push(item);
        }

        return items;
    }

    // ─── Construction des CompletionItem ───────────────────────────────────

    private builtinMethodItem(className: string, m: BuiltinMethod, isStatic: boolean): vscode.CompletionItem {
        const item = new vscode.CompletionItem(m.name, vscode.CompletionItemKind.Method);
        const paramsStr = m.params.map(p => `${p.name}:${p.type}`).join(', ');
        const sep = isStatic ? '::' : '.';
        item.detail = `${className}${sep}${m.name}(${paramsStr}): ${m.returns}`;
        item.insertText = new vscode.SnippetString(m.params.length > 0 ? `${m.name}($1)` : `${m.name}()`);
        item.documentation = new vscode.MarkdownString(`\`${item.detail}\`\n\nMéthode builtin — \`ocara.${className}\``);
        return item;
    }

    private builtinConstItem(className: string, c: BuiltinConst, isStatic: boolean): vscode.CompletionItem {
        const item = new vscode.CompletionItem(c.name, vscode.CompletionItemKind.Constant);
        item.detail = `${className}${isStatic ? '::' : '.'}${c.name}: ${c.type}`;
        item.documentation = new vscode.MarkdownString(`Constante builtin — \`ocara.${className}\``);
        return item;
    }

    private exceptionPropertyItem(className: string, p: { name: string; type: string }): vscode.CompletionItem {
        const item = new vscode.CompletionItem(p.name, vscode.CompletionItemKind.Field);
        item.detail = `${className}.${p.name}: ${p.type}`;
        item.documentation = new vscode.MarkdownString('Propriété commune à toutes les exceptions Ocara.');
        return item;
    }

    private memberItem(className: string, m: ClassMember): vscode.CompletionItem {
        const sep = m.isStatic ? '::' : '.';
        if (m.kind === 'method') {
            const item = new vscode.CompletionItem(m.name, vscode.CompletionItemKind.Method);
            item.detail = `${className}${sep}${m.name}(${m.params}): ${m.returnType || 'void'}`;
            item.insertText = new vscode.SnippetString(m.params.trim().length > 0 ? `${m.name}($1)` : `${m.name}()`);
            item.documentation = new vscode.MarkdownString(
                `\`${item.detail}\`\n\n${m.visibility}${m.isStatic ? ' static' : ''} method — classe \`${className}\``
            );
            return item;
        }
        const kind = m.kind === 'const' ? vscode.CompletionItemKind.Constant : vscode.CompletionItemKind.Field;
        const item = new vscode.CompletionItem(m.name, kind);
        item.detail = `${className}${sep}${m.name}: ${m.returnType}`;
        item.documentation = new vscode.MarkdownString(`${m.visibility} ${m.kind} — classe \`${className}\``);
        return item;
    }
}
