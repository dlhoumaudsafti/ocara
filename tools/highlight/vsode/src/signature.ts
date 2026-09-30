import * as vscode from 'vscode';
import { findCallSite, resolveCall, paramLabel, CallSite, ResolvedCall } from './callsite';

// ─────────────────────────────────────────────────────────────────────────────
// Signature help : signature de la cible de l'appel sous le curseur, avec le
// paramètre actif surligné — déterminé par NOM pour un argument nommé
// (`f(b: 1, a: |)` → `a`), par position sinon (voir
// docs/roadmap.d/langage-named-arguments.md).
// ─────────────────────────────────────────────────────────────────────────────

export class OcaraSignatureHelpProvider implements vscode.SignatureHelpProvider {

    async provideSignatureHelp(
        document: vscode.TextDocument,
        position: vscode.Position,
        _token: vscode.CancellationToken,
        _context: vscode.SignatureHelpContext
    ): Promise<vscode.SignatureHelp | undefined> {
        const site = findCallSite(document, position);
        if (!site) { return undefined; }
        const call = await resolveCall(document, position, site);
        if (!call) { return undefined; }

        const labels = call.params.map(paramLabel);
        const label = `${call.owner}(${labels.join(', ')})${call.returnType ? ': ' + call.returnType : ''}`;
        const info = new vscode.SignatureInformation(label);
        let cursor = call.owner.length + 1;
        info.parameters = labels.map(l => {
            const range: [number, number] = [cursor, cursor + l.length];
            cursor += l.length + 2;
            return new vscode.ParameterInformation(range);
        });

        const help = new vscode.SignatureHelp();
        help.signatures = [info];
        help.activeSignature = 0;
        help.activeParameter = activeParameter(call, site);
        return help;
    }
}

function activeParameter(call: ResolvedCall, site: CallSite): number {
    const byName = (name: string | undefined) => call.params.findIndex(p => p.name === name);
    if (site.currentName !== undefined) { return byName(site.currentName); }
    if (site.usedNames.length > 0) {
        return call.params.findIndex(p => !p.variadic && !site.usedNames.includes(p.name));
    }
    const variadicIndex = call.params.findIndex(p => p.variadic);
    return variadicIndex >= 0 ? Math.min(site.argIndex, variadicIndex) : site.argIndex;
}
