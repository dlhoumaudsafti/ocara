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
        addInstanceSugar();
    } catch (err) {
        console.error('Ocara: impossible de charger data/builtins-data.json', err);
        builtins = [];
        builtinsByName = new Map();
    }
}

/** Classes dont les méthodes statiques à récepteur s'appellent en sucre
 * d'instance (`req.path()` ≡ `HTTPServerRequest::path(req)`), comme
 * `allows_instance_sugar` dans src/sema/typecheck.rs. */
const INSTANCE_SUGAR_CLASSES = new Set(['HTTPRequest', 'HTTPResponse', 'HTTPServerRequest', 'HTTPServerSession']);

/** Ajoute à la classe du récepteur une copie d'instance (récepteur retiré)
 * de chaque méthode statique dont le premier paramètre est ce récepteur. */
function addInstanceSugar(): void {
    for (const cls of [...builtins]) {
        for (const m of cls.methods) {
            const receiver = m.static ? m.params[0]?.type : undefined;
            if (!receiver || !INSTANCE_SUGAR_CLASSES.has(receiver)) { continue; }
            let target = builtinsByName.get(receiver);
            if (!target) {
                target = { name: receiver, methods: [], consts: [] };
                builtins.push(target);
                builtinsByName.set(receiver, target);
            }
            if (!target.methods.some(x => !x.static && x.name === m.name)) {
                target.methods.push({ ...m, static: false, params: m.params.slice(1) });
            }
        }
    }
}

export function builtinClasses(): BuiltinClass[] {
    return builtins;
}

export function getBuiltinClass(name: string): BuiltinClass | undefined {
    return builtinsByName.get(name);
}
