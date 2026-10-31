// Composants d'écran du moteur desktop d'`ocara.UI` (docs/oc-ui.md,
// « Composants d'écran ») : tiroir de navigation (`drawer`), panneau
// inférieur (`bottomSheet`), bouton flottant (`fab`), conteneur flottant
// (`floating`) et indicateur d'attente (`spinner`). Chargé avant engine.js,
// qui lui passe ses fonctions (`api` : `el`, `container`, `icon`, `fire`,
// `changed`).
//
// Tiroir, panneau, bouton flottant et `floating` fixe sont déclarés
// n'importe où dans l'écran et rendus dans une couche au-dessus de lui (un
// ancêtre animé ne fausse pas leur position) ; à leur place dans l'arbre,
// rien.
var OcaraLayers = (function () {
  "use strict";

  var MENU_PATH = "M3 18h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V6H3z";
  var SIZE_ORDER = ["xxs", "xs", "sm", "md", "lg", "xl", "xxl"];

  /// Ancre → (vertical, horizontal) ; aussi les positions d'un `fab`.
  var ANCHORS = {
    topStart: ["top", "start"], topCenter: ["top", "center"], topEnd: ["top", "end"],
    startCenter: ["center", "start"], center: ["center", "center"], endCenter: ["center", "end"],
    bottomStart: ["bottom", "start"], bottomCenter: ["bottom", "center"], bottomEnd: ["bottom", "end"],
  };

  /// Rendu dans la couche de l'écran plutôt qu'à sa place ?
  function layered(node) {
    return node.type === "fab" || node.type === "drawer" || node.type === "bottomSheet"
      || (node.type === "floating" && node.position !== "absolute");
  }

  /// Nœuds de la couche de `layout`, dans l'ordre ; le premier `fab` seul
  /// (comme Material). Les gabarits de liste ne comptent pas.
  function collect(layout) {
    var out = [], fab = false;
    (function walk(node) {
      if (layered(node)) {
        if (node.type !== "fab" || !fab) out.push(node);
        if (node.type === "fab") fab = true;
      }
      (node.children || []).forEach(walk);
    })(layout);
    return out;
  }

  function permanent(p, sizeClass) {
    return !!p.permanentFrom && SIZE_ORDER.indexOf(sizeClass) >= SIZE_ORDER.indexOf(p.permanentFrom);
  }

  /// Place `e` (fixe : dans la fenêtre ; sinon dans son parent) au point
  /// `anchor`, décalé de `x`/`y` vers l'intérieur.
  function place(e, anchor, x, y, fixed) {
    var s = e.style, a = ANCHORS[anchor] || ANCHORS.topStart;
    var dx = (x || 0) + "px", dy = (y || 0) + "px";
    var shift = [];
    s.position = fixed ? "fixed" : "absolute";
    if (a[1] === "start") s.left = dx;
    else if (a[1] === "end") s.right = dx;
    else { s.left = "calc(50% + " + dx + ")"; shift.push("translateX(-50%)"); }
    if (a[0] === "top") s.top = dy;
    else if (a[0] === "bottom") s.bottom = dy;
    else { s.top = "calc(50% + " + dy + ")"; shift.push("translateY(-50%)"); }
    if (shift.length) s.translate = shift.length === 2 ? "-50% -50%" : (a[1] === "center" ? "-50% 0" : "0 -50%");
  }

  /// Fermeture par l'utilisateur (voile, Échap) : état et `onDismiss`.
  function dismiss(wrap, node, p, api) {
    wrap.classList.remove("oc-open");
    api.changed(node.id, "open", false);
    api.fire(p, "onDismiss", node.id);
  }

  /// Tiroir ou panneau : voile et boîte ; la décoration du nœud porte sur
  /// la boîte (`ocBox`).
  function panel(node, p, ctx, api, sizeClass) {
    var wrap = api.el("div", "oc-panel oc-" + node.type);
    if (node.type === "drawer") {
      wrap.classList.add("oc-side-" + (p.side || "start"));
      if (permanent(p, sizeClass)) wrap.classList.add("oc-permanent");
    }
    var scrim = api.el("div", "oc-scrim");
    scrim.addEventListener("click", function () { dismiss(wrap, node, p, api); });
    var box = api.container(node, p, ctx, "column");
    box.classList.add("oc-panel-box");
    wrap.append(scrim, box);
    if (p.open) wrap.classList.add("oc-open");
    wrap.ocBox = box;
    wrap.ocDismiss = function () { dismiss(wrap, node, p, api); };
    return wrap;
  }

  function fab(node, p, api) {
    var e = api.el("button", "oc-fab" + (p.text != null ? " oc-fab-extended" : ""));
    e.type = "button";
    if (p.icon) e.appendChild(api.icon(p.icon));
    if (p.text != null) e.appendChild(document.createTextNode(p.text));
    e.setAttribute("aria-label", p.label || p.text || "");
    place(e, p.position || "bottomEnd", 16, 16, true);
    e.addEventListener("click", function () { api.fire(p, "onClick", node.id); });
    return e;
  }

  function floating(node, p, ctx, api, fixed) {
    var e = api.container(node, p, ctx, "column");
    e.classList.add("oc-floating");
    place(e, p.anchor, p.x, p.y, fixed);
    return e;
  }

  /// Nombre d'éléments dessinés par chaque modèle (engine.css, `.oc-spin-*`).
  var SPINNER_PARTS = { circle: 0, dots: 3, bars: 4, pulse: 0, ring: 0, orbit: 1 };

  function spinner(node, p, api) {
    var size = (p.size || 32) + "px";
    if (p.src) {
      var img = api.el("img", "oc-spinner");
      img.src = p.src;
      img.style.width = img.style.height = size;
      img.alt = p.label || "";
      return img;
    }
    var kind = p.kind || "circle";
    var e = api.el("span", "oc-spinner oc-spin-" + kind);
    e.setAttribute("role", "progressbar");
    if (p.label) e.setAttribute("aria-label", p.label);
    e.style.width = e.style.height = size;
    e.style.setProperty("--oc-spin-size", size);
    for (var i = 0; i < SPINNER_PARTS[kind]; i++) e.appendChild(api.el("i"));
    return e;
  }

  /// Propositions d'un champ (`suggestions`, menu flottant sous lui) : choix
  /// à la souris ou au clavier (↑ ↓ Entrée, Échap), action `onSuggestion`
  /// avec sa valeur. `e.ocSuggest(liste)` les remplace sans reconstruire le
  /// champ (saisie et focus gardés).
  var menu = null;
  function suggest(e, node, p, api) {
    var items = [], active = -1;
    function normalized(list) {
      return (list || []).map(function (o) {
        return o != null && typeof o === "object" ? { value: o.value, text: o.text != null ? o.text : String(o.value) } : { value: o, text: String(o) };
      });
    }
    function close() {
      if (menu && menu.ocOwner === e) { menu.remove(); menu = null; }
      active = -1;
    }
    function pick(item) {
      close();
      api.fire(p, "onSuggestion", node.id, item.value);
    }
    function show() {
      if (!items.length || document.activeElement !== e) return close();
      if (!menu || menu.ocOwner !== e) {
        if (menu) menu.remove();
        menu = api.el("div", "oc-suggestions");
        menu.setAttribute("role", "listbox");
        menu.ocOwner = e;
        document.body.appendChild(menu);
      }
      var r = e.getBoundingClientRect();
      menu.style.left = r.left + "px";
      menu.style.top = r.bottom + 4 + "px";
      menu.style.minWidth = r.width + "px";
      menu.textContent = "";
      items.forEach(function (item, i) {
        var row = api.el("div", "oc-suggestion" + (i === active ? " oc-active" : ""));
        row.setAttribute("role", "option");
        row.textContent = item.text;
        // Avant le `blur` du champ : le choix ne ferme pas le menu trop tôt.
        row.addEventListener("mousedown", function (ev) { ev.preventDefault(); pick(item); });
        menu.appendChild(row);
      });
    }
    e.ocSuggest = function (list) { items = normalized(list); active = -1; show(); };
    items = normalized(p.suggestions);
    e.addEventListener("focus", show);
    e.addEventListener("blur", close);
    e.addEventListener("input", function () { active = -1; });
    e.addEventListener("keydown", function (ev) {
      if (!menu || menu.ocOwner !== e) return;
      if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
        ev.preventDefault();
        active = (active + (ev.key === "ArrowDown" ? 1 : items.length - 1)) % items.length;
        show();
      } else if (ev.key === "Enter" && active >= 0) {
        ev.preventDefault();
        ev.stopImmediatePropagation();
        pick(items[active]);
      } else if (ev.key === "Escape") {
        ev.stopPropagation();
        close();
      }
    });
  }

  /// Bouton de la barre de titre qui ouvre le tiroir `wrap` (sauf permanent).
  function menuButton(wrap, node, api) {
    var b = api.el("button", "oc-button oc-icon-button oc-menu-button");
    b.type = "button";
    b.setAttribute("aria-label", "Menu");
    b.innerHTML = '<span class="oc-icon"><svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor"></path></svg></span>';
    b.querySelector("path").setAttribute("d", MENU_PATH);
    b.addEventListener("click", function () {
      wrap.classList.add("oc-open");
      api.changed(node.id, "open", true);
    });
    return b;
  }

  /// Échap : ferme le dernier tiroir ou panneau ouvert (pas un tiroir permanent).
  function closeTop() {
    var open = document.querySelectorAll(".oc-panel.oc-open:not(.oc-permanent)");
    var top = open[open.length - 1];
    if (top && top.ocDismiss) top.ocDismiss();
    return !!top;
  }

  return {
    layered: layered, collect: collect, permanent: permanent, place: place,
    panel: panel, fab: fab, floating: floating, spinner: spinner, suggest: suggest,
    menuButton: menuButton, closeTop: closeTop,
  };
})();
