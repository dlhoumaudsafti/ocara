import * as vscode from 'vscode';
import { BuiltinMethod, getBuiltinClass } from './builtins';
import { esc } from './resolver';

// ─────────────────────────────────────────────────────────────────────────────
// Méthodes d'instance des types primitifs (`string`, `int`, `float`, `bool`,
// `array<T>`, `map<K,V>`) — même table que le compilateur
// (`src/sema/convert_sugar.rs`) : conversions `Convert` sous un nom sans
// préfixe de type source (`s.toInt()` ≡ `Convert::strToInt(s)`), plus, pour
// string/array/map, les méthodes des classes `String`/`Array`/`Map`
// utilisables en instance (`s.trim()` ≡ `String::trim(s)`).
// ─────────────────────────────────────────────────────────────────────────────

export type PrimitiveKind = 'string' | 'int' | 'float' | 'bool' | 'array' | 'map';

const CONVERSIONS: Record<PrimitiveKind, [string, string][]> = {
    string: [['toInt', 'strToInt'], ['toFloat', 'strToFloat'], ['toBool', 'strToBool'], ['toArray', 'strToArray'], ['toMap', 'strToMap']],
    int:    [['toStr', 'intToStr'], ['toFloat', 'intToFloat'], ['toBool', 'intToBool']],
    float:  [['toStr', 'floatToStr'], ['toInt', 'floatToInt'], ['toBool', 'floatToBool']],
    bool:   [['toStr', 'boolToStr'], ['toInt', 'boolToInt'], ['toFloat', 'boolToFloat']],
    array:  [['toStr', 'arrayToStr'], ['toMap', 'arrayToMap']],
    map:    [['toStr', 'mapToStr']],
};

const SUGAR_CLASS: Partial<Record<PrimitiveKind, string>> = { string: 'String', array: 'Array', map: 'Map' };

export interface InstanceMethod {
    name: string;
    /** Paramètres sans le receveur. */
    params: BuiltinMethod['params'];
    returns: string;
    /** Méthode statique réellement appelée, ex. "Convert::strToInt". */
    target: string;
}

/** Type primitif déclaré d'une variable/propriété/paramètre, s'il en est un. */
export function findPrimitiveType(document: vscode.TextDocument, varName: string): PrimitiveKind | undefined {
    const declRe = new RegExp(`\\b(?:var|scoped|consumed|const|property)\\s+${esc(varName)}\\s*:\\s*(string|int|float|bool|array|map)\\b`);
    const paramRe = new RegExp(`[(,]\\s*${esc(varName)}\\s*:\\s*(string|int|float|bool|array|map)\\b`);
    for (let i = 0; i < document.lineCount; i++) {
        const text = document.lineAt(i).text;
        const m = text.match(declRe) ?? (/\b(?:function|method|init|nameless)\b/.test(text) ? text.match(paramRe) : null);
        if (m) { return m[1] as PrimitiveKind; }
    }
    return undefined;
}

export function instanceMethodsFor(kind: PrimitiveKind): InstanceMethod[] {
    const methods: InstanceMethod[] = [];
    const convert = getBuiltinClass('Convert');
    for (const [name, staticName] of CONVERSIONS[kind]) {
        const m = convert?.methods.find(x => x.name === staticName);
        if (m) { methods.push({ name, params: m.params.slice(1), returns: m.returns, target: `Convert::${staticName}` }); }
    }
    const sugarClass = SUGAR_CLASS[kind];
    const sugar = sugarClass ? getBuiltinClass(sugarClass) : undefined;
    for (const m of sugar?.methods ?? []) {
        methods.push({ name: m.name, params: m.params.slice(1), returns: m.returns, target: `${sugarClass}::${m.name}` });
    }
    return methods;
}
