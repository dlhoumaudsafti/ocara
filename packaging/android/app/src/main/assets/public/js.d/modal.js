export class OcaraModal {
  static open(kind, message, confirmLabel, cancelLabel) {
    const dialog = document.querySelector(`[data-ocara-modal="${kind}"]`);
    if (!(dialog instanceof HTMLDialogElement)) {
      return Promise.reject(new Error(`Modal "${kind}" introuvable dans la page.`));
    }

    const messageElement = dialog.querySelector("[data-modal-message]");
    if (!(messageElement instanceof HTMLElement)) {
      return Promise.reject(new Error(`Contenu de la modal "${kind}" introuvable.`));
    }
    messageElement.textContent = message;

    const confirmButton = dialog.querySelector("[data-modal-confirm]");
    if (confirmButton instanceof HTMLButtonElement) {
      confirmButton.textContent = confirmLabel;
    }
    const cancelButton = dialog.querySelector("[data-modal-cancel]");
    if (cancelButton instanceof HTMLButtonElement && cancelLabel !== undefined) {
      cancelButton.textContent = cancelLabel;
    }

    return new Promise((resolve) => {
      dialog.addEventListener("close", () => {
        resolve(dialog.returnValue);
      }, { once: true });
      dialog.showModal();
    });
  }

  static confirm(message, confirmLabel = "Confirmer", cancelLabel = "Annuler") {
    return this.open("confirm", message, confirmLabel, cancelLabel)
      .then((result) => result === "confirm");
  }

  static alert(message, buttonLabel = "OK") {
    return this.open("alert", message, buttonLabel);
  }

  static info(message, buttonLabel = "OK") {
    return this.open("info", message, buttonLabel);
  }
}

export function initializeModal() {
  window.OcaraModal = OcaraModal;
  document.addEventListener("submit", (event) => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement) || !form.dataset.confirmMessage) {
      return;
    }
    if (form.dataset.confirmed === "true") {
      delete form.dataset.confirmed;
      return;
    }

    event.preventDefault();
    const submitter = event.submitter;
    void OcaraModal.confirm(
      form.dataset.confirmMessage,
      form.dataset.confirmLabel || "Confirmer",
      form.dataset.cancelLabel || "Annuler"
    ).then((confirmed) => {
      if (confirmed) {
        form.dataset.confirmed = "true";
        form.requestSubmit(submitter instanceof HTMLElement ? submitter : undefined);
      }
    });
  });
}
