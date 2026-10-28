// Tests du moteur de rendu desktop d'`ocara.UI` (runtime_tauri/src/ui/engine.js),
// joués par ci/ui_engine.sh dans Chrome sans écran sur l'aperçu de
// examples/ui/components.oc.ui. Résultat écrit dans #oc-test.
(function () {
  var sent = [];
  var failures = [];
  OcaraUI.start(TREE, { platform: "tauri", send: function (m) { sent.push(m); } });

  function q(selector) { return document.querySelector(selector); }
  function node(id) { return q('[data-id="' + id + '"]'); }
  function check(name, ok) { if (!ok) failures.push(name); }
  function last() { return sent[sent.length - 1] || {}; }
  function type(e, value) { e.value = value; e.dispatchEvent(new Event("input")); }

  check("ready envoyé", sent[0] && sent[0].action === "ready");
  check("titre de la fenêtre", document.title === "Composants");
  check("barre de titre", q(".oc-bar-title").textContent === "Formulaire");
  check("titre h1", q("h1.oc-heading").textContent === "Inscription");
  check("style nommé", q(".oc-text").style.fontWeight === "bold");
  check("mot de passe", node("password").type === "password");
  check("multiligne", node("notes").tagName === "TEXTAREA");

  type(node("name"), "Ada");
  check("saisie → état", last().id === "name" && last().prop === "value" && last().value === "Ada");
  node("name").dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));
  check("Entrée → onSubmit", last().action === "save" && last().value === "Ada");

  var terms = node("terms").querySelector("input");
  terms.click();
  check("case → onChange", last().action === "terms_changed" && last().value === true);
  check("interrupteur coché", node("news").querySelector("input").checked);
  check("radio", node("plan").querySelectorAll("input")[0].checked);
  check("liste déroulante", node("country").value === "fr" && node("country").options.length === 3);
  var volume = node("volume");
  volume.value = "7";
  volume.dispatchEvent(new Event("input"));
  check("curseur → nombre", last().action === "volume_changed" && last().value === 7);
  check("date", node("birthday").type === "date");
  check("progression", Math.abs(node("upload").value - 0.4) < 1e-9);
  check("bouton désactivé", Array.prototype.some.call(document.querySelectorAll("button"), function (b) { return b.disabled && b.textContent === "Désactivé"; }));
  check("icône du bouton", node("save").querySelector("svg path").getAttribute("d").indexOf("M9 16.17") === 0);

  node("save").click();
  check("clic → onClick", last().action === "save" && last().id === "save");

  OcaraUI.set("save", "text", "Valider");
  check("set réaffiche le nœud", node("save").textContent === "Valider");
  OcaraUI.set("save", "visible", false);
  check("set visible", node("save").hidden);
  OcaraUI.set("name", "value", "Grace");
  check("set valeur", node("name").value === "Grace");
  check("get", OcaraUI.get("name", "value") === "Grace");

  OcaraUI.navigate("list");
  check("navigation", q(".oc-bar-title").textContent === "Liste" && q(".oc-bar button"));
  OcaraUI.set("people", "items", [{ name: "Ada", city: "Londres" }, { name: "Alan", city: "Wilmslow" }]);
  var rows = node("people").querySelectorAll(".oc-list-item");
  check("liste remplie", rows.length === 2 && rows[1].textContent.indexOf("Wilmslow") >= 0);
  rows[1].click();
  check("onSelect", last().action === "open_person" && last().index === 1 && last().value.name === "Alan");
  check("surcharge android ignorée", q(".oc-card .oc-text:last-child").style.fontSize === "");
  check("grille", getComputedStyle(q(".oc-grid")).gridTemplateColumns.split(" ").length === 3);
  OcaraUI.set("summary", "value", "2 éléments");
  check("set texte", node("summary").textContent === "2 éléments");

  var count = sent.length;
  Array.prototype.find.call(document.querySelectorAll("button"), function (b) { return b.textContent === "Retour"; }).click();
  check("back prédéfini", q(".oc-bar-title").textContent === "Formulaire" && sent.length === count);
  check("état gardé au retour", node("name").value === "Grace" && node("save").hidden);
  OcaraUI.back();
  check("back au départ → hôte", last().action === "back");

  var out = document.createElement("pre");
  out.id = "oc-test";
  out.textContent = failures.length ? "UI-ENGINE-FAIL: " + failures.join(" | ") : "UI-ENGINE-OK";
  document.body.appendChild(out);
})();
