// Copie la documentation du langage dans l'extension, à chaque compilation
// (`npm run compile`, donc aussi `vsce package` via `vscode:prepublish`) :
// `docs/EBNF.md` et `docs/builtins/*.md` du dépôt → `docs/` de l'extension,
// après suppression de la copie précédente. Le survol ouvre ces fichiers en
// aperçu : ils doivent voyager avec l'extension, pas dépendre des sources
// d'Ocara. Copie = artefact de build (ignoré par git), jamais édité ici.
const fs = require('fs');
const path = require('path');

const extDir = path.resolve(__dirname, '..');
const repoDocs = path.resolve(extDir, '../../../docs');
const target = path.join(extDir, 'docs');

fs.rmSync(target, { recursive: true, force: true });
fs.mkdirSync(path.join(target, 'builtins'), { recursive: true });

fs.copyFileSync(path.join(repoDocs, 'EBNF.md'), path.join(target, 'EBNF.md'));
let count = 1;
for (const file of fs.readdirSync(path.join(repoDocs, 'builtins'))) {
    if (!file.endsWith('.md')) { continue; }
    fs.copyFileSync(path.join(repoDocs, 'builtins', file), path.join(target, 'builtins', file));
    count++;
}
console.log(`copy-docs : ${count} fichier(s) copié(s) dans ${path.relative(process.cwd(), target) || target}`);
