import * as vscode from 'vscode';
import { getBuiltinClass } from './builtins';
import {
    findVariableType,
    findEnclosingClassName,
    findMethodSignature,
    findFunctionSignature,
    CallableSignature,
} from './resolver';

// ─────────────────────────────────────────────────────────────────────────────
// Site d'appel autour du curseur et paramètres de sa cible — partagé par la
// complétion des noms d'arguments, le signature help et la navigation vers
// le paramètre d'un argument nommé (`f(name: ...)`, voir
// docs/roadmap.d/langage-named-arguments.md).
//
// Même approche heuristique que resolver.ts (regex + comptage de
// parenthèses, pas de vrai parseur) : un appel via `Function<...>` ou dont le
// receveur n'a pas de type déclaré reste simplement sans aide.
// ─────────────────────────────────────────────────────────────────────────────

/** Nombre maximal de lignes remontées pour trouver la `(` ouvrante. */
const MAX_LINES_BACK = 50;

export type CallTargetKind = 'function' | 'instance' | 'static' | 'new';

export interface CallSite {
    kind: CallTargetKind;
    /** Fonction ou méthode appelée (`init` pour `use Classe(...)`). */
    name: string;
    /** Receveur : variable/`self`/`parent` (instance) ou classe (static/new). */
    receiver?: string;
    /** Index de l'argument sous le curseur (0 = premier). */
    argIndex: number;
    /** Noms déjà fournis dans les arguments précédents (`name:` en tête). */
    usedNames: string[];
    /** Nombre d'arguments précédents passés positionnellement. */
    positionalCount: number;
    /** Nom de l'argument sous le curseur, s'il est nommé. */
    currentName?: string;
}

export interface ParamInfo {
    name: string;
    type: string;
    defaultValue?: string;
    variadic: boolean;
    /** Position (dans `ResolvedCall.source.fileText`) du nom du paramètre. */
    offset: number;
}

/** Cible résolue d'un site d'appel. `source` absent pour un builtin. */
export interface ResolvedCall {
    owner: string;
    params: ParamInfo[];
    returnType: string;
    source?: CallableSignature;
}

/** Masque le contenu des chaînes et commentaires (longueur conservée). */
function maskLine(text: string): string {
    let out = '';
    let quote: string | undefined;
    for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        if (quote) {
            if (ch === '\\') {
                out += text[i + 1] === undefined ? ' ' : '  ';
                i++;
            } else if (ch === quote) {
                out += ch;
                quote = undefined;
            } else {
                out += ' ';
            }
            continue;
        }
        if (ch === '/' && text[i + 1] === '/') { return out + ' '.repeat(text.length - i); }
        if (ch === '"' || ch === "'" || ch === '`') { quote = ch; }
        out += ch;
    }
    return out;
}

/**
 * Découpe `text` sur les virgules de niveau 0. `withGenerics` compte aussi
 * `<`/`>` comme imbrication (types `map<K, V>` d'une liste de paramètres —
 * jamais dans des arguments, où `->`/`=>` n'ouvrent rien).
 */
function splitTopLevel(text: string, withGenerics: boolean): string[] {
    const open = withGenerics ? '([{<' : '([{';
    const close = withGenerics ? ')]}>' : ')]}';
    const parts: string[] = [];
    let depth = 0;
    let current = '';
    for (const ch of text) {
        if (open.includes(ch)) { depth++; }
        else if (close.includes(ch)) { depth--; }
        if (ch === ',' && depth === 0) {
            parts.push(current);
            current = '';
        } else {
            current += ch;
        }
    }
    parts.push(current);
    return parts;
}

const NAMED_PREFIX_RE = /^\s*([A-Za-z_]\w*)\s*:(?!:)/;

/** Retrouve l'appel dont les parenthèses entourent `position`. */
export function findCallSite(document: vscode.TextDocument, position: vscode.Position): CallSite | undefined {
    let depth = 0;
    let argsText = '';
    const firstLine = Math.max(0, position.line - MAX_LINES_BACK);
    for (let line = position.line; line >= firstLine; line--) {
        const raw = document.lineAt(line).text;
        const text = maskLine(line === position.line ? raw.substring(0, position.character) : raw);
        for (let i = text.length - 1; i >= 0; i--) {
            const ch = text[i];
            if (ch === ')' || ch === ']' || ch === '}') { depth++; }
            else if (ch === '[' || ch === '{') {
                if (depth === 0) { return undefined; }
                depth--;
            } else if (ch === '(') {
                if (depth > 0) { depth--; }
                else { return buildCallSite(document, position, text.substring(0, i), argsText); }
            }
            argsText = ch + argsText;
        }
        argsText = '\n' + argsText;
    }
    return undefined;
}

function buildCallSite(
    document: vscode.TextDocument,
    position: vscode.Position,
    beforeParen: string,
    argsText: string
): CallSite | undefined {
    const args = splitTopLevel(argsText, false);
    const previous = args.slice(0, -1);
    const usedNames = previous.map(a => a.match(NAMED_PREFIX_RE)?.[1]).filter((n): n is string => !!n);
    const common = {
        argIndex: previous.length,
        usedNames,
        positionalCount: previous.length - usedNames.length,
        currentName: args[args.length - 1].match(NAMED_PREFIX_RE)?.[1],
    };

    let m = beforeParen.match(/\buse\s+([A-Z]\w*)\s*(?:<[^>]*>)?\s*$/);
    if (m) { return { kind: 'new', name: 'init', receiver: m[1], ...common }; }
    m = beforeParen.match(/\b([A-Za-z_]\w*)::([A-Za-z_]\w*)\s*$/);
    if (m) {
        const receiver = m[1] === 'self' || m[1] === 'parent' ? findEnclosingClassName(document, position) : m[1];
        return receiver ? { kind: 'static', name: m[2], receiver, ...common } : undefined;
    }
    m = beforeParen.match(/\b([A-Za-z_]\w*)\.([A-Za-z_]\w*)\s*$/);
    if (m) { return { kind: 'instance', name: m[2], receiver: m[1], ...common }; }
    m = beforeParen.match(/(?:^|[^.:\w])([a-z_]\w*)\s*$/);
    if (m && !/^(if|while|for|switch|match|function|method|init|nameless|return)$/.test(m[1])) {
        return { kind: 'function', name: m[1], ...common };
    }
    return undefined;
}

/** Cible d'un site d'appel : fonction/méthode/`init` utilisateur, ou méthode statique builtin. */
export async function resolveCall(
    document: vscode.TextDocument,
    position: vscode.Position,
    site: CallSite
): Promise<ResolvedCall | undefined> {
    const builtin = site.kind === 'static' && site.receiver ? getBuiltinClass(site.receiver) : undefined;
    if (builtin) {
        const method = builtin.methods.find(m => m.static && m.name === site.name);
        if (!method) { return undefined; }
        return {
            owner: `${builtin.name}::${method.name}`,
            params: method.params.map(p => ({ name: p.name, type: p.type, variadic: /^variadic\b/.test(p.type), offset: -1 })),
            returnType: method.returns,
        };
    }
    const source = await resolveUserSignature(document, position, site);
    return source ? { owner: source.owner, params: parseParams(source), returnType: source.returnType, source } : undefined;
}

async function resolveUserSignature(
    document: vscode.TextDocument,
    position: vscode.Position,
    site: CallSite
): Promise<CallableSignature | undefined> {
    switch (site.kind) {
        case 'function':
            return findFunctionSignature(document, site.name);
        case 'static':
        case 'new':
            return site.receiver ? findMethodSignature(document, site.receiver, site.name) : undefined;
        case 'instance': {
            const receiver = site.receiver!;
            const className = receiver === 'self' || receiver === 'parent'
                ? findEnclosingClassName(document, position)
                : findVariableType(document, receiver);
            return className ? findMethodSignature(document, className, site.name) : undefined;
        }
    }
}

/** Paramètres nommables encore disponibles (hors variadic, hors déjà fournis). */
export function remainingNamedParams(call: ResolvedCall, site: CallSite): ParamInfo[] {
    return call.params.filter(p => !p.variadic && !site.usedNames.includes(p.name));
}

/** Découpe une liste de paramètres brute (`a:int, b:string = "x"`). */
function parseParams(signature: CallableSignature): ParamInfo[] {
    const params: ParamInfo[] = [];
    let offset = signature.paramsOffset;
    for (const part of splitTopLevel(signature.params, true)) {
        const m = part.match(/^(\s*)([A-Za-z_]\w*)\s*:\s*([^=]+?)\s*(?:=\s*(.+?))?\s*$/s);
        if (m) {
            const type = m[3].trim();
            params.push({
                name: m[2],
                type,
                defaultValue: m[4]?.trim(),
                variadic: /^variadic\b/.test(type),
                offset: offset + m[1].length,
            });
        }
        offset += part.length + 1;
    }
    return params;
}

/** Libellé d'un paramètre tel qu'affiché (`name:type = défaut`). */
export function paramLabel(p: ParamInfo): string {
    return p.defaultValue !== undefined ? `${p.name}:${p.type} = ${p.defaultValue}` : `${p.name}:${p.type}`;
}
