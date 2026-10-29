// Boîtes de saisie et de confirmation : `prompt` et `confirm` sont bloqués
// dans les webviews de VS Code.

function box(text, withInput, value) {
    return new Promise(resolve => {
        const back = document.createElement('div');
        back.className = 'd-modal';
        const form = document.createElement('form');
        form.className = 'd-modal-box';
        const label = document.createElement('p');
        label.textContent = text;
        form.appendChild(label);
        const input = document.createElement('input');
        input.className = 'd-input';
        input.value = value ?? '';
        if (withInput) { form.appendChild(input); }
        const tools = document.createElement('div');
        tools.className = 'd-inline d-right-align';
        const cancel = document.createElement('button');
        cancel.type = 'button';
        cancel.textContent = 'Annuler';
        const ok = document.createElement('button');
        ok.type = 'submit';
        ok.className = 'd-primary';
        ok.textContent = 'OK';
        tools.append(cancel, ok);
        form.appendChild(tools);
        back.appendChild(form);
        document.body.appendChild(back);
        const close = result => { back.remove(); resolve(result); };
        cancel.addEventListener('click', () => close(withInput ? null : false));
        back.addEventListener('keydown', ev => { if (ev.key === 'Escape') { close(withInput ? null : false); } });
        form.addEventListener('submit', ev => { ev.preventDefault(); close(withInput ? input.value.trim() : true); });
        (withInput ? input : ok).focus();
        if (withInput) { input.select(); }
    });
}

/** Texte saisi, ou `null` si annulé. */
export const ask = (text, value) => box(text, true, value);
export const confirmBox = text => box(text, false);
