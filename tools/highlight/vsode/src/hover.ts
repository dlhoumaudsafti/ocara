import * as vscode from 'vscode';
import { BuiltinMethod } from './builtins';
import { docLink, ebnfHeading, rewriteDocLinks, trustedMarkdown } from './docs';
import { CONTEXTUAL_KEYWORDS, KEYWORDS } from './keywords';
import { maskSource } from './declarations';

// ─────────────────────────────────────────────────────────────────────────────
// Documentation des mots-clés au survol (résumé + lien vers l'EBNF). Le survol
// des noms (variables, fonctions, méthodes, classes, builtins) vient du
// serveur de langage (`ocara --lsp`, voir lspclient.ts).
// ─────────────────────────────────────────────────────────────────────────────

export class OcaraKeywordHoverProvider implements vscode.HoverProvider {

    provideHover(document: vscode.TextDocument, position: vscode.Position): vscode.Hover | undefined {
        const range = document.getWordRangeAtPosition(position, /[A-Za-z_]\w*/);
        if (!range) { return undefined; }
        const word = document.getText(range);
        const line = document.lineAt(position.line).text;
        const keyword = keywordDoc(document, position, word, line.substring(0, range.start.character), line.substring(range.end.character));
        return keyword ? new vscode.Hover(trustedMarkdown(keyword), range) : undefined;
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

function codeBlock(code: string): string {
    return '```ocara\n' + code + '\n```';
}
