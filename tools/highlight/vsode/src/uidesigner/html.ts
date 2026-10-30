import * as vscode from 'vscode';

/** Page du designer : trois colonnes (palette et structure, écran, propriétés). */
export function html(webview: vscode.Webview, media: vscode.Uri): string {
    const n = Array.from({ length: 24 }, () => Math.floor(Math.random() * 36).toString(36)).join('');
    const uri = (file: string) => webview.asWebviewUri(vscode.Uri.joinPath(media, file));
    const csp = `default-src 'none'; img-src ${webview.cspSource} https: data:; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${n}' ${webview.cspSource}; font-src ${webview.cspSource};`;
    return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<link rel="stylesheet" href="${uri('engine.css')}">
<link rel="stylesheet" href="${uri('designer.css')}">
</head><body>
  <header class="d-toolbar">
    <select id="screen" title="Écran affiché"></select>
    <button id="screenAdd" title="Nouvel écran">＋ Écran</button>
    <span class="d-sep"></span>
    <div class="d-seg" id="sizes" title="Largeur de l'aperçu (classes de taille)"></div>
    <span class="d-sep"></span>
    <div class="d-seg" id="platform">
      <button data-platform="tauri" class="on" title="Aperçu desktop (Tauri)">Desktop</button><button data-platform="android" title="Aperçu Android (Compose)">Android</button>
    </div>
    <select id="state" title="Aperçu d'un état : tables states des nœuds appliquées (le survol réel marche aussi)">
      <option value="">État : normal</option><option value="hover">survol</option><option value="focus">focus</option><option value="active">appui</option><option value="disabled">désactivé</option><option value="invalid">invalide</option><option value="checked">coché</option><option value="visited">visité</option>
    </select>
    <button id="replay" title="Rejouer les animations de l'aperçu">▶ Animations</button>
    <div class="d-seg" id="theme">
      <button data-theme="auto" title="Thème auto : window.theme du fichier, sinon celui de VS Code (le système, dans l'application)">Auto</button><button data-theme="light" title="Forcer le thème clair">☀</button><button data-theme="dark" title="Forcer le thème sombre">☾</button>
    </div>
    <span class="d-grow"></span>
    <div class="d-seg" id="variants" title="Variantes de taille du fichier"></div>
    <button id="check" title="Enregistrer et vérifier avec le compilateur">✓ Vérifier</button>
    <button id="source" title="Afficher le source YAML">{ } Source</button>
  </header>
  <main class="d-main">
    <aside class="d-left">
      <section class="d-panel" id="palettePanel"><h2>Composants</h2><input id="paletteFilter" placeholder="Filtrer…"><div id="palette"></div></section>
      <section class="d-panel d-fill"><h2>Structure</h2><div id="tree"></div></section>
    </aside>
    <section class="d-center" id="center">
      <div class="d-stage" id="stage">
        <div class="d-frame" id="frame"><div class="oc-canvas" id="canvas"></div></div>
        <div class="d-hover" id="hoverBox" hidden></div><div class="d-select" id="selectBox" hidden><span id="selectTag"></span></div><div class="d-drop" id="dropLine" hidden></div>
      </div>
      <footer class="d-problems" id="problems"></footer>
    </section>
    <aside class="d-right">
      <nav class="d-tabs" id="tabs">
        <button data-tab="props" class="on">Propriétés</button><button data-tab="window">Fenêtre</button><button data-tab="styles">Styles</button><button data-tab="animations">Animations</button><button data-tab="menu">Menu</button>
      </nav>
      <div class="d-inspector" id="inspector"></div>
    </aside>
  </main>
  <div class="d-banner" id="banner" hidden></div>
  <datalist id="actionList"></datalist>
  <script nonce="${n}" src="${uri('effects.js')}"></script>
  <script nonce="${n}" type="module" src="${uri('main.js')}"></script>
</body></html>`;
}
