import * as vscode from 'vscode';
import { BuiltinMethod, getBuiltinClass } from './builtins';
import { findPrimitiveType, instanceMethodsFor } from './primitives';
import { runtimeContext } from './runtimecontext';
import { docLink, ebnfHeading, rewriteDocLinks, trustedMarkdown } from './docs';
import { CONTEXTUAL_KEYWORDS, KEYWORDS } from './keywords';
import { maskSource } from './declarations';
import {
    CallableSignature,
    findBuiltinAncestor,
    findEnclosingClassName,
    findFunctionSignature,
    findMethodSignature,
    findStructFields,
    findTypeDeclaration,
    findVariableType,
    leadingComment,
} from './resolver';

// ─────────────────────────────────────────────────────────────────────────────
// Documentation au survol :
//   - méthode builtin, statique (`String::trim`) ou d'instance (`server.route`,
//     classe utilisateur héritant d'un builtin comprise) — doc extraite de
//     docs/builtins/*.md ;
//   - sucre d'instance (`s.trim()` ≡ `String::trim(s)`, `s.toInt()` ≡
//     `Convert::strToInt(s)`) — doc de la méthode réellement appelée ;
//   - fonction, méthode, classe, struct... utilisateur : signature + les
//     commentaires `//` placés juste au-dessus de la déclaration.
// ─────────────────────────────────────────────────────────────────────────────

export class OcaraHoverProvider implements vscode.HoverProvider {

    async provideHover(document: vscode.TextDocument, position: vscode.Position): Promise<vscode.Hover | undefined> {
        const range = document.getWordRangeAtPosition(position, /[A-Za-z_]\w*/);
        if (!range) { return undefined; }
        const word = document.getText(range);
        const line = document.lineAt(position.line).text;
        const before = line.substring(0, range.start.character);
        const after = line.substring(range.end.character);
        const isCall = /^\s*\(/.test(after);

        const keyword = keywordDoc(document, position, word, before, after);
        if (keyword) { return this.hoverFor(range, keyword); }

        const staticMatch = before.match(/([A-Za-z_]\w*)::$/);
        if (staticMatch) {
            const owner = staticMatch[1] === 'self' || staticMatch[1] === 'parent'
                ? findEnclosingClassName(document, position) ?? staticMatch[1]
                : staticMatch[1];
            return this.hoverFor(range, await this.methodDoc(document, owner, word, '::'));
        }

        const instanceMatch = before.match(/([A-Za-z_]\w*)\.$/);
        if (instanceMatch) {
            return this.hoverFor(range, await this.instanceDoc(document, position, instanceMatch[1], word));
        }

        if (/\buse\s+$/.test(before) || /^[A-Z]/.test(word)) {
            return this.hoverFor(range, await this.typeDoc(document, word));
        }
        if (isCall) {
            const sig = await findFunctionSignature(document, word);
            return this.hoverFor(range, sig ? userCallableDoc(sig, `function ${word}`) : undefined);
        }
        return undefined;
    }

    private hoverFor(range: vscode.Range, markdown: string | undefined): vscode.Hover | undefined {
        if (!markdown) { return undefined; }
        return new vscode.Hover(trustedMarkdown(markdown), range);
    }

    /** `receiver.word` : variable typée, `self`/`parent`, primitif (sucre). */
    private async instanceDoc(document: vscode.TextDocument, position: vscode.Position, receiver: string, word: string): Promise<string | undefined> {
        let className = receiver === 'self' || receiver === 'parent'
            ? findEnclosingClassName(document, position)
            : findVariableType(document, receiver);
        if (!className) {
            const primitive = findPrimitiveType(document, receiver);
            if (primitive) {
                const sugar = instanceMethodsFor(primitive).find(m => m.name === word);
                if (!sugar) { return undefined; }
                const [cls, method] = sugar.target.split('::');
                const target = getBuiltinClass(cls)?.methods.find(m => m.name === method);
                const params = sugar.params.map(p => `${p.name}:${p.type}`).join(', ');
                const head = codeBlock(`${receiver}.${word}(${params}): ${sugar.returns}`);
                const note = `_Équivalent de_ \`${sugar.target}(${receiver}${sugar.params.length ? ', ' + sugar.params.map(p => p.name).join(', ') : ''})\``;
                return [head, note, target?.doc].filter(Boolean).join('\n\n');
            }
            for (const doc of await runtimeContext(document)) {
                className = findVariableType(doc, receiver);
                if (className) { return this.methodDoc(doc, className, word, '.'); }
            }
            return undefined;
        }
        return this.methodDoc(document, className, word, '.');
    }

    /** Méthode `word` de la classe `owner` : builtin, utilisateur, ou héritée d'un builtin. */
    private async methodDoc(document: vscode.TextDocument, owner: string, word: string, sep: string): Promise<string | undefined> {
        const builtin = getBuiltinClass(owner);
        if (builtin) {
            const m = builtin.methods.find(x => x.name === word);
            return m ? builtinDoc(owner, m, sep) : undefined;
        }
        const sig = await findMethodSignature(document, owner, word);
        if (sig) { return userCallableDoc(sig, `${owner}${sep}${word}`); }
        const ancestor = await findBuiltinAncestor(document, owner, name => getBuiltinClass(name) !== undefined);
        const inherited = ancestor ? getBuiltinClass(ancestor)?.methods.find(x => x.name === word) : undefined;
        return inherited ? builtinDoc(ancestor!, inherited, sep) + `\n\n_Hérité de \`${ancestor}\` par \`${owner}\`._` : undefined;
    }

    /** Classe/struct/interface... : en-tête, commentaire, et constructeur généré d'un struct. */
    private async typeDoc(document: vscode.TextDocument, name: string): Promise<string | undefined> {
        const builtin = getBuiltinClass(name);
        if (builtin) {
            return codeBlock(`ocara.${name}`) + `\n\nClasse builtin — ${builtin.methods.length} méthode(s).\n\n` +
                docLink(`📖 docs/builtins/${name}.md`, `builtins/${name}.md`);
        }
        const decl = await findTypeDeclaration(document, name);
        if (!decl) { return undefined; }
        const parts = [codeBlock(decl.header)];
        if (decl.kind === 'struct') {
            const fields = await findStructFields(document, name);
            if (fields) {
                const params = fields.map(f => `${f.name}:${f.type}${f.defaultValue !== undefined ? ' = ' + f.defaultValue : ''}`);
                parts.push('Constructeur généré :\n' + codeBlock(`use ${name}(${params.join(', ')})`));
            }
        } else {
            const init = await findMethodSignature(document, name, 'init');
            if (init) { parts.push('Constructeur :\n' + codeBlock(`use ${name}(${normalizeParams(init.params)})`)); }
        }
        if (decl.comment) { parts.push(decl.comment); }
        return parts.join('\n\n');
    }
}

export function builtinDoc(owner: string, m: BuiltinMethod, sep: string): string {
    const params = m.params.map(p => `${p.name}:${p.type}`).join(', ');
    const head = codeBlock(`${owner}${sep}${m.name}(${params}): ${m.returns}`);
    const file = m.docFile ?? `${owner}.md`;
    const link = docLink(`📖 docs/builtins/${file}`, `builtins/${file}`, m.docHeading);
    const body = m.doc ? rewriteDocLinks(m.doc, 'builtins') : `_Méthode builtin \`ocara.${owner}\` — pas encore de description dans la documentation._`;
    return [head, body, link].join('\n\n');
}

/**
 * Mot-clé sous le curseur (hors chaîne/commentaire) : résumé + lien vers sa
 * section de l'EBNF. Un mot-clé aussi utilisable comme identifiant
 * (`result`, `message`, `init`...) n'est documenté qu'en position de
 * mot-clé : début d'instruction ou après un modificateur, `map`/`array`
 * suivis de `<`, `default` suivi de `=>`.
 */
function keywordDoc(document: vscode.TextDocument, position: vscode.Position, word: string, before: string, after: string): string | undefined {
    const kw = Object.prototype.hasOwnProperty.call(KEYWORDS, word) ? KEYWORDS[word] : undefined;
    if (!kw || /[.:]$/.test(before) || /^\s*:(?!:)/.test(after)) { return undefined; }
    if (CONTEXTUAL_KEYWORDS.has(word)) {
        const asKeyword =
            (/^\s*$/.test(before) || /\b(?:public|private|protected|static|async|is)\s+$/.test(before)) && !/^\s*[.=:]/.test(after)
            || ((word === 'map' || word === 'array') && /^\s*</.test(after))
            || (word === 'default' && /^\s*=>/.test(after));
        if (!asKeyword) { return undefined; }
    }
    const offset = document.offsetAt(position);
    const masked = maskSource(document.getText());
    if (masked[offset] === ' ' && document.getText()[offset] !== ' ') { return undefined; }
    const heading = ebnfHeading(kw.section);
    const link = heading ? docLink(`📖 EBNF §${heading.replace(/`/g, '')}`, 'EBNF.md', heading) : '';
    return [codeBlock(word), kw.summary, link].filter(Boolean).join('\n\n');
}

function userCallableDoc(sig: CallableSignature, label: string): string {
    const head = codeBlock(`${label}(${normalizeParams(sig.params)})${sig.returnType ? ': ' + sig.returnType : ''}`);
    const comment = leadingComment(sig.fileText, sig.paramsOffset);
    return comment ? `${head}\n\n${comment}` : head;
}

function normalizeParams(params: string): string {
    return params.replace(/\s+/g, ' ').trim();
}

function codeBlock(code: string): string {
    return '```ocara\n' + code + '\n```';
}
