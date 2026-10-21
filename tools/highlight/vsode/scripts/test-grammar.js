// Vérifie les scopes de la grammaire TextMate sur des lignes types :
// `npm run test:grammar`. Chaque cas : une ligne, un fragment et le scope
// attendu (ou absent) sur ce fragment.
const fs = require('fs');
const path = require('path');
const vt = require('vscode-textmate');
const onig = require('vscode-oniguruma');

const GRAMMAR = path.join(__dirname, '..', 'syntaxes', 'ocara.tmLanguage.json');
const WASM = require.resolve('vscode-oniguruma/release/onig.wasm');

const CASES = [
    ['db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY)")', 'CREATE', 'keyword.other.sql'],
    ['db.execute("CREATE TABLE t (id INTEGER PRIMARY KEY)")', 'INTEGER', 'storage.type.sql'],
    ['db.query("select count(*) from t where a = :a", m)', 'select', 'keyword.other.sql'],
    ['db.query("select count(*) from t where a = :a", m)', 'count', 'support.function.sql'],
    ['db.query("select count(*) from t where a = :a", m)', ':a', 'variable.parameter.sql'],
    ['db.query("SELECT 1 -- d\'ici", m)', ' d\'ici', 'comment.line.double-dash.sql'],
    ['db.queryOne("SELECT * FROM t WHERE n = \'x\'")', 'x', 'string.quoted.single.sql'],
    ['db.prepare(`SELECT ${col} FROM t`)', 'col', 'source.ocara.embedded'],
    ['db.prepare(`SELECT ${col} FROM t`)', 'FROM', 'keyword.other.sql'],
    ['var q:string = \'UPDATE t SET a = "b"\'', 'b', 'string.quoted.double.sql'],
    ['var q:string = "DELETE FROM t"', 'DELETE', 'keyword.other.sql'],
    ['var q:string = /* sql */ "update t set a = 1"', 'update', 'keyword.other.sql'],
    ['IO::writeln("Select a file")', 'Select a file', '!meta.embedded.sql'],
    ['http.method("DELETE")', 'DELETE', '!meta.embedded.sql'],
    ['var h:string = `<div>${x}</div>`', 'div', 'entity.name.tag.html'],
    ['var h:string = `<div>${x}</div>`', 'div', '!meta.embedded.sql'],
];

async function main() {
    await onig.loadWASM(fs.readFileSync(WASM).buffer);
    const registry = new vt.Registry({
        onigLib: Promise.resolve({
            createOnigScanner: s => new onig.OnigScanner(s),
            createOnigString: s => new onig.OnigString(s),
        }),
        loadGrammar: async () => vt.parseRawGrammar(fs.readFileSync(GRAMMAR, 'utf8'), GRAMMAR),
    });
    const grammar = await registry.loadGrammar('source.ocara');
    let failures = 0;
    for (const [line, fragment, expected] of CASES) {
        const tokens = grammar.tokenizeLine(line, vt.INITIAL).tokens;
        const token = tokens.find(t => line.substring(t.startIndex, t.endIndex) === fragment);
        const scopes = token ? token.scopes : [];
        const wanted = expected.replace(/^!/, '');
        const present = scopes.some(s => s.split(' ').includes(wanted));
        if (!token || present === expected.startsWith('!')) {
            failures++;
            console.log(`FAIL ${line}\n     « ${fragment} » attendu ${expected}, obtenu ${token ? scopes.join(' ') : 'aucun jeton'}`);
        }
    }
    console.log(`${CASES.length - failures}/${CASES.length} cas de grammaire OK`);
    process.exit(failures ? 1 : 0);
}

main();
