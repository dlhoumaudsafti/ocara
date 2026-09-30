// ─────────────────────────────────────────────────────────────────────────────
// Déclarations d'un fichier Ocara (classes, generics, interfaces, modules,
// enums, fonctions libres, et méthodes de chacun) — base de l'index du
// workspace utilisé par les CodeLens (codelens.ts).
//
// Même approche heuristique que resolver.ts (regex + comptage d'accolades,
// pas de vrai parseur) : un en-tête de déclaration est supposé tenir sur une
// seule ligne, comme partout dans les exemples du dépôt.
// ─────────────────────────────────────────────────────────────────────────────

export type DeclKind = 'class' | 'generic' | 'interface' | 'module' | 'enum' | 'function';

export interface MethodDecl {
    name: string;
    line: number;
    col: number;
}

export interface Declaration {
    kind: DeclKind;
    name: string;
    line: number;
    col: number;
    extendsName?: string;
    implementsNames: string[];
    moduleNames: string[];
    methods: MethodDecl[];
}

/**
 * Masque le contenu des chaînes et des commentaires `//` par des espaces
 * (longueur et sauts de ligne conservés) — un template `` `...` `` peut
 * couvrir plusieurs lignes, pas une chaîne `'...'`/`"..."`.
 */
export function maskSource(text: string): string {
    let out = '';
    let quote: string | undefined;
    let inComment = false;
    for (let i = 0; i < text.length; i++) {
        const ch = text[i];
        if (ch === '\n') {
            out += ch;
            inComment = false;
            if (quote !== '`') { quote = undefined; }
            continue;
        }
        if (inComment) { out += ' '; continue; }
        if (quote) {
            if (ch === '\\' && text[i + 1] !== undefined && text[i + 1] !== '\n') {
                out += '  ';
                i++;
            } else if (ch === quote) {
                out += ch;
                quote = undefined;
            } else {
                out += ' ';
            }
            continue;
        }
        if (ch === '/' && text[i + 1] === '/') { inComment = true; out += ' '; continue; }
        if (ch === '"' || ch === "'" || ch === '`') { quote = ch; }
        out += ch;
    }
    return out;
}

const TYPE_HEADER_RE = /\b(class|generic|interface|module|enum)\s+([A-Za-z_]\w*)/;
const FUNCTION_RE = /\bfunction\s+([A-Za-z_]\w*)\s*\(/;
const METHOD_RE = /\bmethod\s+([A-Za-z_]\w*)\s*\(/;

function nameList(header: string, keyword: string): string[] {
    const m = header.match(new RegExp(`\\b${keyword}\\s+([\\w\\s,]+?)(?=\\s+(?:modules|implements)\\b|\\s*\\{|$)`));
    return m ? m[1].split(',').map(n => n.trim()).filter(n => n.length > 0) : [];
}

export function parseDeclarations(text: string): Declaration[] {
    const lines = maskSource(text).split('\n');
    const decls: Declaration[] = [];
    const open: { decl: Declaration; depth: number }[] = [];
    let depth = 0;

    lines.forEach((line, lineNo) => {
        const lineDepth = depth;
        const owner = open.length > 0 ? open[open.length - 1] : undefined;

        const header = line.match(TYPE_HEADER_RE);
        if (header && header.index !== undefined && line.includes('{')) {
            const rest = line.substring(header.index + header[0].length);
            const decl: Declaration = {
                kind: header[1] as DeclKind,
                name: header[2],
                line: lineNo,
                col: header.index + header[0].length - header[2].length,
                extendsName: rest.match(/\bextends\s+([A-Za-z_]\w*)/)?.[1],
                implementsNames: nameList(rest, 'implements'),
                moduleNames: nameList(rest, 'modules'),
                methods: [],
            };
            decls.push(decl);
            open.push({ decl, depth: lineDepth });
        } else if (owner && lineDepth === owner.depth + 1) {
            const method = line.match(METHOD_RE);
            if (method && method.index !== undefined) {
                owner.decl.methods.push({ name: method[1], line: lineNo, col: line.indexOf(method[1], method.index + 6) });
            }
        } else if (!owner) {
            const fn = line.match(FUNCTION_RE);
            if (fn && fn.index !== undefined) {
                decls.push({
                    kind: 'function',
                    name: fn[1],
                    line: lineNo,
                    col: line.indexOf(fn[1], fn.index + 8),
                    implementsNames: [],
                    moduleNames: [],
                    methods: [],
                });
            }
        }

        for (const ch of line) {
            if (ch === '{') { depth++; }
            else if (ch === '}') {
                depth--;
                if (open.length > 0 && depth === open[open.length - 1].depth) { open.pop(); }
            }
        }
    });
    return decls;
}
