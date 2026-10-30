import * as vscode from 'vscode';
import * as path from 'path';
import * as fs from 'fs';
import { isMap, isScalar, LineCounter, parseDocument } from 'yaml';
import { findRoot } from './designer';

// ─────────────────────────────────────────────────────────────────────────────
// Aide à l'édition des `.oc.ui` dans l'éditeur texte :
//   - couleurs (`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`) : pastille
//     et sélecteur, comme en CSS ;
//   - Ctrl+clic sur un chemin d'`includes` (ou le `src` local d'une image) :
//     ouvre le fichier ;
//   - Ctrl+clic (définition) sur la valeur de `start` (écran), de `style`
//     (style) ou de `type` (composant) : sa déclaration, dans ce fichier ou
//     dans un autre fichier de l'interface (racine, fichiers inclus).
// ─────────────────────────────────────────────────────────────────────────────

const SELECTOR: vscode.DocumentSelector = { pattern: '**/*.oc.ui' };

const COLOR = /#(?:[0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})(?![0-9a-zA-Z_])|rgba?\(\s*\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\s*(?:,\s*[\d.]+\s*)?\)/g;

export function registerUiLanguage(context: vscode.ExtensionContext): void {
    context.subscriptions.push(
        vscode.languages.registerColorProvider(SELECTOR, { provideDocumentColors, provideColorPresentations }),
        vscode.languages.registerDocumentLinkProvider(SELECTOR, { provideDocumentLinks }),
        vscode.languages.registerDefinitionProvider(SELECTOR, { provideDefinition }),
    );
}

// ── couleurs ───────────────────────────────────────────────────────────────

function parseColor(text: string): vscode.Color | undefined {
    if (text.startsWith('#')) {
        let hex = text.slice(1);
        if (hex.length <= 4) { hex = [...hex].map(c => c + c).join(''); }
        const n = (i: number) => parseInt(hex.slice(i, i + 2), 16) / 255;
        return new vscode.Color(n(0), n(2), n(4), hex.length === 8 ? n(6) : 1);
    }
    const parts = text.replace(/^rgba?\(|\)$/g, '').split(',').map(p => Number(p.trim()));
    if (parts.slice(0, 3).some(p => !(p >= 0 && p <= 255)) || (parts[3] !== undefined && !(parts[3] >= 0 && parts[3] <= 1))) { return undefined; }
    return new vscode.Color(parts[0] / 255, parts[1] / 255, parts[2] / 255, parts[3] ?? 1);
}

function provideDocumentColors(document: vscode.TextDocument): vscode.ColorInformation[] {
    const out: vscode.ColorInformation[] = [];
    document.getText().split('\n').forEach((line, row) => {
        // Commentaire YAML (`# texte`) : pas de couleur.
        const comment = line.search(/(^|\s)#\s/);
        for (const m of line.matchAll(COLOR)) {
            if (comment >= 0 && m.index! > comment) { continue; }
            const color = parseColor(m[0]);
            if (color) { out.push(new vscode.ColorInformation(new vscode.Range(row, m.index!, row, m.index! + m[0].length), color)); }
        }
    });
    return out;
}

function provideColorPresentations(color: vscode.Color, ctx: { document: vscode.TextDocument; range: vscode.Range }): vscode.ColorPresentation[] {
    const byte = (v: number) => Math.round(v * 255);
    const hex = (v: number) => byte(v).toString(16).padStart(2, '0');
    const opaque = color.alpha >= 1;
    const hexText = `#${hex(color.red)}${hex(color.green)}${hex(color.blue)}${opaque ? '' : hex(color.alpha)}`;
    const alpha = Math.round(color.alpha * 100) / 100;
    const rgbText = opaque
        ? `rgb(${byte(color.red)}, ${byte(color.green)}, ${byte(color.blue)})`
        : `rgba(${byte(color.red)}, ${byte(color.green)}, ${byte(color.blue)}, ${alpha})`;
    // La notation d'origine d'abord : le sélecteur la garde.
    const original = ctx.document.getText(ctx.range);
    const list = original.startsWith('rgb') ? [rgbText, hexText] : [hexText, rgbText];
    return list.map(label => new vscode.ColorPresentation(label));
}

// ── liens : includes, images locales ───────────────────────────────────────

function provideDocumentLinks(document: vscode.TextDocument): vscode.DocumentLink[] {
    const dir = path.dirname(document.uri.fsPath);
    const out: vscode.DocumentLink[] = [];
    document.getText().split('\n').forEach((line, row) => {
        if (/^\s*#/.test(line)) { return; }
        const patterns = [/[\w./-]+\.oc\.ui/g, /(?<=\bsrc:\s*["']?)[\w./-]+\.(?:png|jpe?g|gif|webp|svg)/g];
        for (const re of patterns) {
            for (const m of line.matchAll(re)) {
                const target = path.resolve(dir, m[0]);
                if (!fs.existsSync(target)) { continue; }
                const link = new vscode.DocumentLink(new vscode.Range(row, m.index!, row, m.index! + m[0].length), vscode.Uri.file(target));
                link.tooltip = `Ouvrir ${m[0]}`;
                out.push(link);
            }
        }
    });
    return out;
}

// ── définitions : start, style, type ───────────────────────────────────────

/** Section où chercher la déclaration d'une valeur, selon sa clé. */
const SECTIONS: Record<string, string> = { start: 'screens', style: 'styles', type: 'components', navigate: 'screens' };

/** Fichiers de l'interface : racine (si celui-ci est inclus) et fichiers inclus, récursivement. */
async function interfaceFiles(file: string): Promise<string[]> {
    const root = (await findRoot(file))?.file ?? file;
    const out: string[] = [];
    const visit = (f: string) => {
        if (out.includes(f) || !fs.existsSync(f)) { return; }
        out.push(f);
        let model: any;
        try { model = parseDocument(fs.readFileSync(f, 'utf8')).toJS(); } catch { return; }
        for (const section of ['screens', 'components']) {
            const includes = model?.[section]?.includes;
            if (Array.isArray(includes)) { includes.filter(i => typeof i === 'string').forEach(i => visit(path.resolve(path.dirname(f), i))); }
        }
    };
    visit(root);
    if (!out.includes(file)) { out.unshift(file); }
    return out;
}

async function provideDefinition(document: vscode.TextDocument, position: vscode.Position): Promise<vscode.Location[] | undefined> {
    const range = document.getWordRangeAtPosition(position, /[A-Za-z_][A-Za-z0-9_.]*/);
    if (!range) { return undefined; }
    const name = document.getText(range);
    const before = document.lineAt(position.line).text.slice(0, range.start.character);
    const key = before.match(/([A-Za-z_]+)\s*:\s*["']?$/)?.[1];
    const section = key && SECTIONS[key];
    if (!section) { return undefined; }
    const out: vscode.Location[] = [];
    for (const file of await interfaceFiles(document.uri.fsPath)) {
        const text = file === document.uri.fsPath ? document.getText() : fs.readFileSync(file, 'utf8');
        const lines = new LineCounter();
        const doc = parseDocument(text, { lineCounter: lines });
        const map = doc.get(section, true);
        if (!isMap(map)) { continue; }
        for (const pair of map.items) {
            if (isScalar(pair.key) && pair.key.value === name && pair.key.range) {
                const at = lines.linePos(pair.key.range[0]);
                const pos = new vscode.Position(at.line - 1, at.col - 1);
                out.push(new vscode.Location(vscode.Uri.file(file), new vscode.Range(pos, pos.translate(0, name.length))));
            }
        }
    }
    return out;
}
