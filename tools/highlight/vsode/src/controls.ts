import * as vscode from 'vscode';

// ─────────────────────────────────────────────────────────────────────────────
// En-tête de la barre latérale Ocara : configuration `when` de l'éditeur
// (OS, ARCH, BUILD — réglages ocara.os/arch/build) et boutons Compiler,
// Lancer, Fixer, Dump (mode Debug/Release suivant BUILD), accès aux réglages.
// ─────────────────────────────────────────────────────────────────────────────

const OS = ['system', 'linux', 'windows', 'macos', 'android', 'other'];
const ARCH = ['system', 'x86_64', 'aarch64', 'arm', 'other'];
const BUILD = ['debug', 'release'];

type Message =
    | { type: 'set'; key: string; value: string }
    | { type: 'run'; action: 'compile' | 'run' | 'fix' | 'dump' | 'settings' };

export class ControlsProvider implements vscode.WebviewViewProvider {
    private view: vscode.WebviewView | undefined;

    register(context: vscode.ExtensionContext): void {
        context.subscriptions.push(
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
        view.onDidChangeVisibility(() => this.post());
        this.post();
    }

    /** Valeurs courantes des réglages → champs du formulaire. */
    private post(): void {
        const c = vscode.workspace.getConfiguration('ocara');
        void this.view?.webview.postMessage({
            os: c.get('os', 'system'), osOther: c.get('osOther', ''),
            arch: c.get('arch', 'system'), archOther: c.get('archOther', ''),
            build: c.get('build', 'debug'),
        });
    }

    private async handle(msg: Message): Promise<void> {
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
</style></head>
<body>
  <div class="row"><label for="os">OS</label><div><select id="os">${options(OS)}</select><input id="osOther" placeholder="valeur de System::OS" hidden></div></div>
  <div class="row"><label for="arch">ARCH</label><div><select id="arch">${options(ARCH)}</select><input id="archOther" placeholder="valeur de System::ARCH" hidden></div></div>
  <div class="row"><label for="build">BUILD</label><select id="build">${options(BUILD)}</select></div>
  <div class="buttons">
    <button class="primary" data-action="compile" title="Compiler le script affiché (mode BUILD)">Compiler</button>
    <button class="primary" data-action="run" title="Compiler et lancer le script affiché (mode BUILD)">Lancer</button>
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
  $('build').addEventListener('change', e => set('build', e.target.value));
  document.querySelectorAll('button').forEach(b => b.addEventListener('click', () => vscode.postMessage({ type: 'run', action: b.dataset.action })));
  window.addEventListener('message', ({ data }) => {
    for (const key of ['os', 'arch', 'build']) { $(key).value = data[key]; }
    for (const key of ['os', 'arch']) {
      $(key + 'Other').hidden = data[key] !== 'other';
      if (document.activeElement !== $(key + 'Other')) { $(key + 'Other').value = data[key + 'Other']; }
    }
  });
</script></body></html>`;
}
