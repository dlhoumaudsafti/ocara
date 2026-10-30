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

  // Événements communs : survol, focus, double clic, balayage.
  var zone = node("zone");
  zone.dispatchEvent(new PointerEvent("pointerenter"));
  check("survol → onHover", last().action === "zone_hover" && last().id === "zone");
  zone.dispatchEvent(new PointerEvent("pointerleave"));
  check("sortie → onLeave", last().action === "zone_leave");
  zone.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  check("double clic → onDoubleClick", last().action === "zone_double");
  zone.dispatchEvent(new PointerEvent("pointerdown", { clientX: 10, clientY: 10 }));
  zone.dispatchEvent(new PointerEvent("pointerup", { clientX: 120, clientY: 14 }));
  check("balayage → onSwipe", last().action === "zone_swipe" && last().value === "right");
  // Chrome sans écran ne donne pas le focus système à la page : événements simulés.
  node("name").dispatchEvent(new FocusEvent("focusin", { bubbles: true }));
  check("focus → onFocus", last().action === "name_focus" && last().value === "Ada");
  node("name").dispatchEvent(new FocusEvent("focusout", { bubbles: true }));
  check("perte du focus → onBlur", last().action === "name_blur");
  check("champ obligatoire", node("name").getAttribute("aria-required") === "true");
  OcaraUI.set("name", "invalid", true);
  check("état invalid", node("name").classList.contains("oc-invalid"));
  OcaraUI.set("name", "focus", true);
  check("focus demandé par l'hôte", document.activeElement === node("name") && OcaraUI.get("name", "focus") === undefined);
  OcaraUI.set("name", "invalid", false);
  check("état invalid levé sans perdre le focus", !node("name").classList.contains("oc-invalid") && document.activeElement === node("name"));
  node("name").blur();

  OcaraUI.set("save", "text", "Valider");
  check("set réaffiche le nœud", node("save").textContent === "Valider");
  OcaraUI.set("save", "visible", false);
  check("set visible", node("save").hidden && getComputedStyle(node("save")).display === "none");
  OcaraUI.set("name", "value", "Grace");
  check("set valeur", node("name").value === "Grace");
  check("get", OcaraUI.get("name", "value") === "Grace");

  OcaraUI.navigate("list");
  check("navigation", q(".oc-bar-title").textContent === "Liste" && q(".oc-bar button"));
  var placements = function (visible) {
    return sent.filter(function (m) { return m.webview === "page" && m.visible === visible; });
  };
  OcaraUI.set("people", "items", [{ name: "Ada", city: "Londres" }, { name: "Alan", city: "Wilmslow" }]);
  var rows = node("people").querySelectorAll(".oc-list-item");
  check("liste remplie", rows.length === 2 && rows[1].textContent.indexOf("Wilmslow") >= 0);
  rows[1].click();
  check("onSelect", last().action === "open_person" && last().index === 1 && last().value.name === "Alan");
  check("surcharge android ignorée", q(".oc-card .oc-text:last-child").style.fontSize === "");
  check("grille", getComputedStyle(q(".oc-grid")).gridTemplateColumns.split(" ").length === 3);
  OcaraUI.set("summary", "value", "2 éléments");
  check("set texte", node("summary").textContent === "2 éléments");

  // Rectangle de la vue web : envoyé à l'image suivante (requestAnimationFrame).
  var count = sent.length;
  Array.prototype.find.call(document.querySelectorAll("button"), function (b) { return b.textContent === "Retour"; }).click();
  check("back prédéfini", q(".oc-bar-title").textContent === "Formulaire" && sent.length === count);
  check("état gardé au retour", node("name").value === "Grace" && node("save").hidden);
  OcaraUI.back();
  check("back au départ → hôte", last().action === "back");

  // Variantes de taille : seuils autour de la largeur de la page.
  var wide = JSON.parse(JSON.stringify(TREE));
  wide.screens.form.layout.children[0].value = "Large";
  var sized = JSON.parse(JSON.stringify(TREE));
  sized.sizes = { petit: 0, large: window.innerWidth, immense: window.innerWidth + 1 };
  sized.variants = { large: wide };
  OcaraUI.reload(sized);
  check("variante de la largeur", q("h1.oc-heading").textContent === "Large");
  check("classe rapportée", sent.some(function (m) { return m.action === "resize" && m.value === "large"; }));
  check("état gardé entre variantes", node("name").value === "Grace");
  OcaraUI.reload(TREE);
  check("retour à l'arbre de base", q("h1.oc-heading").textContent === "Inscription");

  // Surcharge de style du thème affiché (`styles.dark`).
  var dark = JSON.parse(JSON.stringify(TREE));
  dark.window.theme = "dark";
  OcaraUI.reload(dark);
  check("style du thème sombre", q(".oc-text").style.color === "rgb(128, 203, 196)" && q(".oc-text").style.fontWeight === "bold");
  OcaraUI.reload(TREE);

  // Vue web : rectangle envoyé après le rendu (setTimeout).
  OcaraUI.navigate("list");
  OcaraUI.set("tiles", "items", [1, 2, 3, 4, 5].map(function (n) { return { name: "t" + n }; }));
  setTimeout(function () {
    // Serpentin sur 3 colonnes : t4 et t5 repartent de droite.
    var tiles = node("tiles").querySelectorAll(".oc-list-item");
    var cols = Array.prototype.map.call(tiles, function (t) { return t.style.gridColumn + "/" + t.style.gridRow; }).join(" ");
    check("grille de liste en serpentin (" + cols + ")", cols === "1/1 2/1 3/1 3/2 2/2");
    var shown = placements(true)[0] || {};
    check("webview placée", shown.width > 0 && Math.round(shown.height) === 140);
    OcaraUI.back();
    setTimeout(function () {
      check("webview masquée hors de son écran", placements(false).length >= 1);
      // Appui long : après 500 ms, sans clic au relâchement.
      var zone = node("zone");
      zone.dispatchEvent(new PointerEvent("pointerdown", { clientX: 10, clientY: 10 }));
      setTimeout(function () {
        check("appui long → onLongPress", last().action === "zone_long");
        zone.dispatchEvent(new PointerEvent("pointerup", { clientX: 10, clientY: 10 }));
        finish();
      }, 600);
    }, 100);
  }, 100);
  return;

  function finish() {
  var out = document.createElement("pre");
  out.id = "oc-test";
  out.textContent = failures.length ? "UI-ENGINE-FAIL: " + failures.join(" | ") : "UI-ENGINE-OK";
  document.body.appendChild(out);
  }
})();
