import * as path from 'path';
import * as fs from 'fs';

// ─────────────────────────────────────────────────────────────────────────────
// Catalogue des classes builtin `ocara.*` (généré depuis src/builtins/*.rs,
// voir tools/highlight/vsode/data/ et scripts/generate-builtins-data.py).
// ─────────────────────────────────────────────────────────────────────────────

export interface BuiltinParam { name: string; type: string; }
export interface BuiltinMethod { name: string; params: BuiltinParam[]; returns: string; static: boolean; /** Documentation markdown extraite de docs/builtins/*.md (voir scripts/generate-builtins-data.py). */ doc?: string; /** Fichier de docs/builtins/ et titre de section d'où vient `doc`. */ docFile?: string; docHeading?: string; }
export interface BuiltinConst { name: string; type: string; }
export interface BuiltinClass { name: string; methods: BuiltinMethod[]; consts: BuiltinConst[]; }

let builtins: BuiltinClass[] = [];
let builtinsByName: Map<string, BuiltinClass> = new Map();

/** Charge le catalogue des builtins (appelé une fois à l'activation). */
export function loadBuiltins(extensionPath: string): void {
    try {
        const dataPath = path.join(extensionPath, 'data', 'builtins-data.json');
        const raw = fs.readFileSync(dataPath, 'utf8');
        builtins = JSON.parse(raw) as BuiltinClass[];
        builtinsByName = new Map(builtins.map(c => [c.name, c]));
    } catch (err) {
        console.error('Ocara: impossible de charger data/builtins-data.json', err);
        builtins = [];
        builtinsByName = new Map();
    }
}

export function builtinClasses(): BuiltinClass[] {
    return builtins;
}

export function getBuiltinClass(name: string): BuiltinClass | undefined {
    return builtinsByName.get(name);
}
