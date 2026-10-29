import * as fs from 'fs';

// ─────────────────────────────────────────────────────────────────────────────
// Catalogue des composants du designer `.oc.ui`, tiré du schéma JSON produit
// par le compilateur (`ocara --ui-schema` → schemas/oc-ui.schema.json, tenu à
// jour par src/core/tests_ui_schema.rs) : types de nœuds, propriétés de
// chacun et genre d'éditeur de chaque propriété. Un composant ajouté au
// compilateur apparaît donc dans la palette sans toucher au designer.
// ─────────────────────────────────────────────────────────────────────────────

/** Genre d'éditeur d'une propriété (media/uidesigner/editors.js). */
export type Kind =
    | 'text' | 'bool' | 'int' | 'number' | 'length' | 'spacing' | 'color' | 'enum' | 'id'
    | 'icon' | 'action' | 'style' | 'field' | 'node' | 'children' | 'strings' | 'options'
    | 'menu' | 'actionMap' | 'binds' | 'override';

export interface PropSpec { name: string; kind: Kind; values?: string[]; min?: number; max?: number }
export interface TypeSpec { type: string; own: PropSpec[]; required: string[] }

export interface Catalog {
    types: TypeSpec[];
    /** Propriétés de tout nœud, hors présentation. */
    common: PropSpec[];
    /** Propriétés de présentation (nœuds et `styles:`). */
    style: PropSpec[];
    window: PropSpec[];
    windowTauri: PropSpec[];
    windowAndroid: PropSpec[];
    screen: PropSpec[];
    menu: PropSpec[];
    icons: string[];
}

type Schema = { [key: string]: any };

function kindOf(name: string, s: Schema): PropSpec {
    const spec = (kind: Kind, extra: Partial<PropSpec> = {}): PropSpec => ({ name, kind, ...extra });
    const range = { min: s.minimum, max: s.maximum };
    switch (name) {
        case 'id': return spec('id');
        case 'style': return spec('style');
        case 'bind': return spec('field');
        case 'binds': return spec('binds');
        case 'children': return spec('children');
        case 'item': return s.$ref ? spec('node') : spec('menu');
        case 'items': return spec('menu');
        case 'tauri': case 'android': return spec('override');
        case 'contextMenu': case 'menu': return spec('menu');
        case 'options': return spec('options');
        case 'webHandlers': return spec('actionMap');
    }
    if (s.enum) { return spec(name === 'icon' || name === 'name' ? 'icon' : 'enum', { values: s.enum }); }
    if (typeof s.pattern === 'string' && s.pattern.startsWith('^#')) { return spec('color'); }
    if (s.anyOf?.some((a: Schema) => a.enum)) { return spec('length'); }
    if (s.anyOf?.some((a: Schema) => a.type === 'array')) { return spec('spacing'); }
    if (/^on[A-Z]/.test(name) || name === 'action') { return spec('action'); }
    if (s.type === 'boolean') { return spec('bool'); }
    if (s.type === 'integer') { return spec('int', range); }
    if (s.type === 'number') { return spec('number', range); }
    if (s.type === 'array') { return spec('strings'); }
    return spec('text');
}

function props(properties: Schema | undefined, skip: string[] = []): PropSpec[] {
    return Object.entries(properties ?? {}).filter(([n]) => !skip.includes(n)).map(([n, s]) => kindOf(n, s));
}

export function loadCatalog(schemaPath: string): Catalog {
    const schema: Schema = JSON.parse(fs.readFileSync(schemaPath, 'utf8'));
    const node = schema.definitions.node;
    const order: string[] = node.properties.type.enum;
    const rules = new Map<string, Schema>();
    for (const rule of node.allOf as Schema[]) {
        const type = rule.if?.properties?.type?.const;
        if (type) { rules.set(type, rule.then); }
    }
    const style = props(schema.properties.styles.additionalProperties.properties);
    const styleNames = new Set(style.map(p => p.name));
    // Communes : présentes sur tous les types, hors présentation.
    const all = order.map(t => Object.keys(rules.get(t)?.properties ?? {}));
    const commonNames = all[0].filter(n => n !== 'type' && !styleNames.has(n) && all.every(names => names.includes(n)));
    const firstProps = rules.get(order[0])?.properties ?? {};
    const common = commonNames.map(n => kindOf(n, firstProps[n]));
    const types = order.map(type => {
        const rule = rules.get(type) ?? {};
        const own = props(rule.properties, ['type', ...commonNames, ...styleNames]);
        return { type, own, required: rule.required ?? [] };
    });
    const win = schema.properties.window.properties;
    const icons = types.flatMap(t => t.own).find(p => p.kind === 'icon')?.values ?? [];
    return {
        types,
        common,
        style,
        window: props(win, ['tauri', 'android']),
        windowTauri: props(win.tauri.properties),
        windowAndroid: props(win.android.properties),
        screen: props(schema.properties.screens.additionalProperties.properties, ['layout']),
        menu: props(schema.definitions.menuItem.properties),
        icons,
    };
}
