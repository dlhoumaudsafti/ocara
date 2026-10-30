import * as vscode from 'vscode';
import type { Navigation, NavState } from './navigation';
import type { QualityMonitor, QualityState } from './quality';

// ─────────────────────────────────────────────────────────────────────────────
// En-tête de la barre latérale Ocara : configuration `when` de l'éditeur
// (OS, ARCH, BUILD — réglages ocara.os/arch/build), script de démarrage
// (ocara.startupScript, lancé par « Lancer ») et boutons Compiler,
// Lancer, Fixer, Dump (mode Debug/Release suivant BUILD), accès aux réglages.
// Barre ← / → de l'historique de navigation, visible après une ouverture
// depuis le panneau (navigation.ts). En tête, la LED « prêt pour la release »
// et ses trois voyants (linter, couverture, architecture — quality.ts) ;
// tous au vert en BUILD release, Compiler et Lancer passent au vert.
// ─────────────────────────────────────────────────────────────────────────────

const OS = ['system', 'linux', 'windows', 'macos', 'android', 'other'];
const ARCH = ['system', 'x86_64', 'aarch64', 'arm', 'other'];
const BUILD = ['debug', 'release'];

type Message =
    | { type: 'set'; key: string; value: string }
    | { type: 'run'; action: 'compile' | 'run' | 'fix' | 'dump' | 'settings' | 'startup' }
    | { type: 'nav'; action: 'back' | 'forward' | 'clear' }
    | { type: 'nav-go'; index: number };

export class ControlsProvider implements vscode.WebviewViewProvider {
    private view: vscode.WebviewView | undefined;

    constructor(private readonly navigation: Navigation, private readonly quality: QualityMonitor) {}

    register(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
            this.navigation.onChange(state => this.postNav(state)),
            this.quality.onChange(state => this.postQuality(state)),
            vscode.window.registerWebviewViewProvider('ocara.controls', this),
            vscode.workspace.onDidChangeConfiguration(e => {
                if (e.affectsConfiguration('ocara')) { this.post(); }
            }),
        );
    }

    resolveWebviewView(view: vscode.WebviewView): void {
        this.view = view;
        view.webview.options = { enableScripts: true };
        view.webview.html = html(nonce());
        view.webview.onDidReceiveMessage((msg: Message) => void this.handle(msg));
        view.onDidChangeVisibility(() => { this.post(); this.postNav(this.navigation.state()); this.postQuality(this.quality.state()); });
        this.post();
        this.postNav(this.navigation.state());
        this.postQuality(this.quality.state());
    }

    private postQuality(quality: QualityState): void {
        void this.view?.webview.postMessage({ quality });
    }

    private postNav(nav: NavState): void {
        void this.view?.webview.postMessage({ nav });
    }

    /** Valeurs courantes des réglages → champs du formulaire. */
    private post(): void {
        const c = vscode.workspace.getConfiguration('ocara');
        void this.view?.webview.postMessage({ config: {
            os: c.get('os', 'system'), osOther: c.get('osOther', ''),
            arch: c.get('arch', 'system'), archOther: c.get('archOther', ''),
            build: c.get('build', 'debug'),
            startup: c.get('startupScript', ''),
        } });
    }

    private async handle(msg: Message): Promise<void> {
        if (msg.type === 'nav') {
            await vscode.commands.executeCommand(`ocara.nav.${msg.action}`);
            return;
        }
        if (msg.type === 'nav-go') {
            await vscode.commands.executeCommand('ocara.nav.go', msg.index);
            return;
        }
        if (msg.type === 'set') {
            const target = vscode.workspace.workspaceFolders ? vscode.ConfigurationTarget.Workspace : vscode.ConfigurationTarget.Global;
            await vscode.workspace.getConfiguration('ocara').update(msg.key, msg.value, target);
            return;
        }
        const release = vscode.workspace.getConfiguration('ocara').get<string>('build', 'debug') === 'release';
        const commands: Record<string, string> = {
            compile: release ? 'ocara.compileRelease' : 'ocara.compile',
            run: release ? 'ocara.compileAndRunRelease' : 'ocara.compileAndRun',
            fix: 'ocara.fixStyle',
            dump: 'ocara.dump',
            startup: 'ocara.chooseStartupScript',
        };
        if (msg.action === 'settings') {
            await vscode.commands.executeCommand('workbench.action.openSettings', '@ext:david-lhoumaud.ocara-language');
        } else {
            await vscode.commands.executeCommand(commands[msg.action]);
        }
    }
}

function nonce(): string {
    return Array.from({ length: 24 }, () => Math.floor(Math.random() * 36).toString(36)).join('');
}

function options(values: string[]): string {
    const labels: Record<string, string> = { system: 'système', other: 'autre…' };
    return values.map(v => `<option value="${v}">${labels[v] ?? v}</option>`).join('');
}

function html(n: string): string {
    return `<!DOCTYPE html>
<html lang="fr"><head><meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'nonce-${n}';">
<style>
  body { padding: 6px 8px; font-size: var(--vscode-font-size); color: var(--vscode-foreground); }
  .row { display: grid; grid-template-columns: 52px 1fr; align-items: center; gap: 4px; margin-bottom: 4px; }
  label { opacity: .8; font-weight: 600; }
  select, input { width: 100%; box-sizing: border-box; background: var(--vscode-dropdown-background); color: var(--vscode-dropdown-foreground);
    border: 1px solid var(--vscode-dropdown-border, transparent); padding: 2px 4px; font: inherit; }
  input { background: var(--vscode-input-background); color: var(--vscode-input-foreground); }
  input[hidden] { display: none; }
  .buttons { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; margin-top: 8px; }
  button { background: var(--vscode-button-secondaryBackground); color: var(--vscode-button-secondaryForeground); border: none;
    padding: 4px 2px; cursor: pointer; font: inherit; border-radius: 2px; }
  button:hover { background: var(--vscode-button-secondaryHoverBackground); }
  button.primary { background: var(--vscode-button-background); color: var(--vscode-button-foreground); }
  button.primary:hover { background: var(--vscode-button-hoverBackground); }
  .settings { width: 100%; margin-top: 4px; }
  .startup { text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; padding: 2px 6px; }
  .nav { display: grid; grid-template-columns: auto auto 1fr auto; gap: 4px; align-items: center; margin-bottom: 8px;
    padding-bottom: 6px; border-bottom: 1px solid var(--vscode-sideBarSectionHeader-border, var(--vscode-widget-border, transparent)); }
  .nav[hidden] { display: none; }
  .nav button { padding: 2px 8px; font-size: 13px; }
  .nav button:disabled { opacity: .4; cursor: default; }
  .quality { display: flex; align-items: center; gap: 6px; margin-bottom: 8px; padding-bottom: 6px; flex-wrap: wrap;
    border-bottom: 1px solid var(--vscode-sideBarSectionHeader-border, var(--vscode-widget-border, transparent)); }
  .led { width: 12px; height: 12px; border-radius: 50%; flex: none; background: #888;
    box-shadow: 0 0 6px 1px currentColor; color: #888; }
  .led.red { background: #e5484d; color: #e5484d; }
  .led.orange { background: #f5a524; color: #f5a524; }
  .led.blue { background: #3b82f6; color: #3b82f6; }
  .led.green { background: #30a46c; color: #30a46c; }
  .quality .title { font-weight: 600; opacity: .85; margin-right: auto; }
  .light { display: inline-flex; align-items: center; gap: 3px; opacity: .85; font-size: 11px; }
  .light::before { content: ''; width: 7px; height: 7px; border-radius: 50%; background: #888; }
  .light.ok::before { background: #30a46c; }
  .light.ko::before { background: #e5484d; }
  button.primary.ready { background: #2e8b57; color: #fff; }
  button.primary.ready:hover { background: #33a065; }
</style></head>
<body>
  <div class="quality" id="quality" title="Qualité requise pour une compilation release">
    <span class="led" id="led"></span><span class="title">Release</span>
    <span class="light" id="q-lint">Linter</span>
    <span class="light" id="q-coverage">Coverage</span>
    <span class="light" id="q-architecture">Architecture</span>
  </div>
  <div class="nav" id="nav" hidden>
    <button id="back" title="Script précédent">←</button>
    <button id="forward" title="Script suivant">→</button>
    <select id="history" title="Historique de navigation"></select>
    <button id="navClear" title="Fermer l'historique">✕</button>
  </div>
  <div class="row"><label for="os">OS</label><div><select id="os">${options(OS)}</select><input id="osOther" placeholder="valeur de System::OS" hidden></div></div>
  <div class="row"><label for="arch">ARCH</label><div><select id="arch">${options(ARCH)}</select><input id="archOther" placeholder="valeur de System::ARCH" hidden></div></div>
  <div class="row"><label for="build">BUILD</label><select id="build">${options(BUILD)}</select></div>
  <div class="row"><label for="startup">MAIN</label><button id="startup" class="startup" data-action="startup" title="Script de démarrage lancé par « Lancer » (réglage ocara.startupScript) — cliquer pour le choisir">script affiché</button></div>
  <div class="buttons">
    <button class="primary" data-action="compile" title="Compiler le script affiché (mode BUILD)">Compiler</button>
    <button class="primary" data-action="run" title="Compiler et lancer le script de démarrage, sinon le script affiché (mode BUILD)">Lancer</button>
    <button data-action="fix" title="Fixer la mise en forme (ocaracs --fix)">Fixer</button>
    <button data-action="dump" title="Afficher le dump (tokens, AST, IR)">Dump</button>
  </div>
  <button class="settings" data-action="settings" title="Réglages de l'extension Ocara">⚙ Paramètres de l'extension</button>
<script nonce="${n}">
  const vscode = acquireVsCodeApi();
  const $ = id => document.getElementById(id);
  const set = (key, value) => vscode.postMessage({ type: 'set', key, value });
  for (const key of ['os', 'arch']) {
    $(key).addEventListener('change', e => { $(key + 'Other').hidden = e.target.value !== 'other'; set(key, e.target.value); });
    $(key + 'Other').addEventListener('change', e => set(key + 'Other', e.target.value.trim()));
  }
  $('build').addEventListener('change', e => { set('build', e.target.value); paintButtons(); });
  document.querySelectorAll('button[data-action]').forEach(b => b.addEventListener('click', () => vscode.postMessage({ type: 'run', action: b.dataset.action })));
  $('back').addEventListener('click', () => vscode.postMessage({ type: 'nav', action: 'back' }));
  $('forward').addEventListener('click', () => vscode.postMessage({ type: 'nav', action: 'forward' }));
  $('navClear').addEventListener('click', () => vscode.postMessage({ type: 'nav', action: 'clear' }));
  $('history').addEventListener('change', e => vscode.postMessage({ type: 'nav-go', index: Number(e.target.value) }));
  let releaseReady = false;
  const ledColor = score => score >= 100 ? 'green' : score >= 66.6666 ? 'blue' : score >= 33.3333 ? 'orange' : 'red';
  const paintButtons = () => {
    document.querySelectorAll('button[data-action="compile"], button[data-action="run"]')
      .forEach(b => b.classList.toggle('ready', releaseReady && $('build').value === 'release'));
  };
  window.addEventListener('message', ({ data: msg }) => {
    if (msg.config) {
      $('startup').textContent = msg.config.startup || 'script affiché';
      $('startup').classList.toggle('set', !!msg.config.startup);
    }
    if (msg.quality) {
      const q = msg.quality;
      $('led').className = 'led ' + ledColor(q.score);
      const names = { lint: 'Linter', coverage: 'Coverage', architecture: 'Architecture' };
      const tips = [];
      for (const key of ['lint', 'coverage', 'architecture']) {
        $('q-' + key).className = 'light ' + q[key].light;
        $('q-' + key).title = names[key] + ' : ' + q[key].detail;
        tips.push(names[key] + ' : ' + q[key].detail);
      }
      $('quality').title = 'Qualité release : ' + Math.round(q.score) + ' %\\n' + tips.join('\\n');
      releaseReady = q.score >= 100;
      paintButtons();
      return;
    }
    if (msg.nav) {
      const { entries, index } = msg.nav;
      $('nav').hidden = entries.length < 2;
      $('back').disabled = index <= 0;
      $('forward').disabled = index >= entries.length - 1;
      $('history').innerHTML = '';
      entries.forEach((e, i) => {
        const o = document.createElement('option');
        o.value = String(i); o.textContent = (i + 1) + '. ' + e.label; o.title = e.detail;
        $('history').appendChild(o);
      });
      $('history').value = String(index);
      return;
    }
    const data = msg.config;
    if (!data) return;
    for (const key of ['os', 'arch', 'build']) { $(key).value = data[key]; }
    paintButtons();
    for (const key of ['os', 'arch']) {
      $(key + 'Other').hidden = data[key] !== 'other';
      if (document.activeElement !== $(key + 'Other')) { $(key + 'Other').value = data[key + 'Other']; }
    }
  });
</script></body></html>`;
}
