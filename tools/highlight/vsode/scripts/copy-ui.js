// Copie dans le designer `.oc.ui` (media/uidesigner/) ce qu'il partage avec
// le compilateur, à chaque compilation de l'extension :
//   - l'apparence du moteur desktop (runtime_tauri/src/ui/engine.css), dont
//     les règles de page (`:root`, `body`) sont ramenées à la zone d'aperçu
//     `.oc-canvas` : l'aperçu a l'apparence de la vraie fenêtre ;
//   - les tracés des icônes (src/core/ui_schema.d/icons.rs) → icons.json.
// Copies = artefacts de build (ignorés par git), jamais éditées ici.
const fs = require('fs');
const path = require('path');

const extDir = path.resolve(__dirname, '..');
const repo = path.resolve(extDir, '../../..');
const target = path.join(extDir, 'media', 'uidesigner');

function replaceAll(text, from, to) {
    if (!text.includes(from)) {
        throw new Error(`copy-ui : « ${from} » introuvable dans engine.css (règle de page modifiée ?)`);
    }
    return text.split(from).join(to);
}

let css = fs.readFileSync(path.join(repo, 'runtime_tauri/src/ui/engine.css'), 'utf8');
css = replaceAll(css, 'html, body { margin: 0; height: 100%; }', '');
css = replaceAll(css, '\nbody {', '\n.oc-canvas {');
css = replaceAll(css, ':root', '.oc-canvas');
fs.writeFileSync(path.join(target, 'engine.css'), `/* Copie de runtime_tauri/src/ui/engine.css (scripts/copy-ui.js) : ne pas éditer. */\n${css}`);

const rust = fs.readFileSync(path.join(repo, 'src/core/ui_schema.d/icons.rs'), 'utf8');
const icons = {};
for (const m of rust.matchAll(/\("([a-z_]+)", "([^"]+)"\)/g)) { icons[m[1]] = m[2]; }
if (Object.keys(icons).length === 0) { throw new Error('copy-ui : aucune icône lue dans icons.rs'); }
fs.writeFileSync(path.join(target, 'icons.json'), JSON.stringify(icons));
console.log(`copy-ui : engine.css et ${Object.keys(icons).length} icône(s) copiés dans media/uidesigner`);
