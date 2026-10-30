// Moteur de rendu desktop d'`ocara.UI` : construit le DOM de l'arbre
// normalisé d'un `.oc.ui` (src/core/ui_schema.d, docs/oc-ui.md) dans la vue
// de la fenêtre Tauri, sans framework.
//
//   OcaraUI.start(tree, { platform, send })  affiche l'écran `start`
//   OcaraUI.set(id, prop, value)             état d'un nœud (réaffiche ce nœud)
//   OcaraUI.navigate(screen), OcaraUI.back(), OcaraUI.reload(tree)
//
// Variantes de taille (`variants`, `sizes`) : la variante de la largeur de la
// fenêtre est affichée, et remplacée à la volée ; la classe est rapportée par
// l'action `resize`.
//
// `send(message)` remet chaque événement à l'hôte : `{ id, prop, value }`
// pour une valeur changée par l'utilisateur (état gardé côté Ocara),
// `{ action, id, value, index }` pour une action déclarée (`onClick`…).
(function () {
  "use strict";

  var root, tree, platform, send; // root : arbre complet ; tree : variante affichée
  var sizeClass = "", variant = "";
  var state = {};      // id → { prop: valeur } (modifié par set ou par l'utilisateur)
  var mounted = {};    // id → { node, ctx, el } (nœud affiché)
  var history = [];    // écrans précédents
  var current;

  function el(tag, cls) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    return e;
  }

  var FLEX_ALIGN = { start: "flex-start", center: "center", end: "flex-end", stretch: "stretch" };

  function px(v) { return typeof v === "number" ? v + "px" : v; }

  function spacing(v) {
    return Array.isArray(v) ? v.map(px).join(" ") : px(v);
  }

  /// Propriétés effectives : style nommé, nœud, surcharge de plateforme, état.
  /// Propriétés effectives : style nommé, nœud, surcharge de plateforme,
  /// état ; dans un élément de liste, `binds` lit les champs de l'élément.
  function props(node, ctx) {
    var out = {};
    var item = ctx && ctx.item;
    var named = node.style && tree.styles && tree.styles[node.style];
    // Surcharge du style pour le thème affiché (`styles.dark`, `styles.light`).
    var themed = node.style && tree.themeStyles && tree.themeStyles[theme()] && tree.themeStyles[theme()][node.style];
    var layers = [named || {}, themed || {}, node, node[platform] || {}, (node.id && !item && state[node.id]) || {}];
    layers.forEach(function (layer) {
      for (var k in layer) if (k !== "tauri" && k !== "android") out[k] = layer[k];
    });
    if (item && node.binds) {
      for (var prop in node.binds) out[prop] = field(item, node.binds[prop]);
    }
    return out;
  }

  /// Thème affiché : `window.theme`, ou celui du système pour `auto`.
  var darkQuery = window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;
  function theme() {
    var t = (tree.window || {}).theme;
    if (t === "dark" || t === "light") return t;
    return darkQuery && darkQuery.matches ? "dark" : "light";
  }

  function field(item, path) {
    return path.split(".").reduce(function (o, k) { return o == null ? o : o[k]; }, item);
  }

  /// Clic d'un nœud ; dans un élément de liste, l'action porte l'élément et
  /// son rang (et ne sélectionne pas la ligne).
  function onClick(e, node, p, ctx) {
    e.addEventListener("click", function (ev) {
      if (ctx && ctx.item) {
        ev.stopPropagation();
        fire(p, "onClick", node.id, ctx.item, ctx.index);
      } else {
        fire(p, "onClick", node.id);
      }
    });
  }

  function fire(p, event, id, value, index) {
    var action = p[event];
    if (!action) return;
    if (action === "back") return OcaraUI.back();
    var message = { action: action, id: id || null, value: value === undefined ? null : value };
    if (index !== undefined) message.index = index;
    send(message);
  }

  /// Valeur changée par l'utilisateur : gardée et remise à l'hôte.
  function changed(id, prop, value) {
    if (!id) return;
    (state[id] = state[id] || {})[prop] = value;
    send({ id: id, prop: prop, value: value });
  }

  /// Flèche de la barre de titre : présente même si le `.oc.ui` n'utilise
  /// pas l'icône `back`.
  var BACK_PATH = "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z";

  function icon(name) {
    var path = (tree.icons && tree.icons[name]) || (name === "back" ? BACK_PATH : null);
    var span = el("span", "oc-icon");
    if (path) {
      span.innerHTML = '<svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor"></path></svg>';
      span.querySelector("path").setAttribute("d", path);
    }
    return span;
  }

  function text(p, ctx) {
    if (p.bind && ctx.item) {
      var v = p.bind.split(".").reduce(function (o, k) { return o == null ? o : o[k]; }, ctx.item);
      return v == null ? "" : String(v);
    }
    return p.value == null ? "" : String(p.value);
  }

  /// Présentation commune : dimensions, marges, couleurs, police.
  function decorate(e, p, parentAxis, isContainer) {
    var s = e.style;
    if (p.align && !isContainer) {
      s.alignSelf = s.justifySelf = FLEX_ALIGN[p.align];
      if (p.align === "center" || p.align === "end") s.textAlign = p.align;
    }
    if (p.padding != null) s.padding = spacing(p.padding);
    if (p.margin != null) s.margin = spacing(p.margin);
    if (p.fontSize != null) s.fontSize = px(p.fontSize);
    if (p.bold) s.fontWeight = "bold";
    if (p.italic) s.fontStyle = "italic";
    if (p.color) s.color = p.color;
    if (p.background) s.background = p.background;
    if (p.radius != null) s.borderRadius = px(p.radius);
    if (p.border) s.border = "1px solid " + p.border;
    if (p.elevation) s.boxShadow = "0 " + p.elevation / 2 + "px " + p.elevation * 1.5 + "px rgba(0,0,0,.25)";
    ["width", "height"].forEach(function (dim) {
      var v = p[dim];
      var main = (dim === "width") === (parentAxis === "row");
      if (v === "fill") {
        if (main) { s.flex = "1 1 0"; s[dim === "width" ? "minWidth" : "minHeight"] = "0"; }
        else s.alignSelf = "stretch";
      } else if (typeof v === "number") s[dim] = px(v);
    });
    if (p.weight) { s.flex = p.weight + " 1 0"; s.minWidth = "0"; }
    if (p.tooltip) e.title = p.tooltip;
    if (p.label) e.setAttribute("aria-label", p.label);
    if (p.visible === false) e.hidden = true;
  }

  function container(node, p, ctx, axis) {
    var e = el("div", "oc-" + node.type);
    var s = e.style;
    if (p.spacing != null) s.gap = px(p.spacing);
    if (p.scroll) s.overflow = "auto";
    if (node.type === "grid") s.gridTemplateColumns = "repeat(" + (p.columns || 2) + ", minmax(0, 1fr))";
    if (p.align) s.alignItems = FLEX_ALIGN[p.align];
    (p.children || []).forEach(function (child) { e.appendChild(build(child, ctx, axis)); });
    if (node.type === "card" && p.onClick) {
      e.classList.add("oc-clickable");
      onClick(e, node, p, ctx);
    }
    return e;
  }

  function list(node, p, ctx) {
    var e = el("div", "oc-list");
    if (p.spacing != null) e.style.gap = px(p.spacing);
    var cells = (p.items || []).map(function (row, index) {
      var item = build(node.item, { item: row, index: index }, "column");
      item.classList.add("oc-list-item");
      if (p.onSelect) item.addEventListener("click", function () { fire(p, "onSelect", node.id, row, index); });
      e.appendChild(item);
      return item;
    });
    if (p.display === "grid") grid(e, cells, p);
    return e;
  }

  /// Liste en grille (`display: grid`). Sans `itemWidth`, chaque case garde
  /// sa largeur naturelle et passe à la ligne quand la place manque
  /// (`minColumns`/`maxColumns` bornent sa largeur) ; avec `itemWidth`
  /// (50 par défaut en serpentin), le nombre de colonnes suit la largeur de
  /// la liste et chaque case est placée selon `direction` : `ltr`, `rtl`,
  /// `ltr-snake`, `rtl-snake` (chaque ligne repart du côté où la précédente
  /// s'arrête). L'ordre des éléments (rang, `onSelect`) reste celui des données.
  function grid(e, cells, p) {
    var gap = p.spacing || 0;
    var dir = p.direction || "ltr";
    var snake = /snake$/.test(dir), rtl = dir.indexOf("rtl") === 0;
    var width = p.itemWidth || (snake ? 50 : null);
    e.classList.add("oc-list-grid");
    if (width == null) {
      e.style.flexFlow = (rtl ? "row-reverse" : "row") + " wrap";
      cells.forEach(function (c) {
        if (p.maxColumns) c.style.minWidth = "calc((100% - " + (p.maxColumns - 1) * gap + "px) / " + p.maxColumns + ")";
        if (p.minColumns) c.style.maxWidth = "calc((100% - " + (p.minColumns - 1) * gap + "px) / " + p.minColumns + ")";
      });
      return;
    }
    e.style.display = "grid";
    e.ocPlace = function () {
      var n = Math.floor((e.clientWidth + gap) / (width + gap));
      n = Math.max(p.minColumns || 1, Math.min(p.maxColumns || n, n), 1);
      e.style.gridTemplateColumns = "repeat(" + n + ", minmax(0, 1fr))";
      cells.forEach(function (c, i) {
        var line = Math.floor(i / n), col = i % n;
        var reversed = rtl !== (snake && line % 2 === 1);
        c.style.gridRow = String(line + 1);
        c.style.gridColumn = String((reversed ? n - 1 - col : col) + 1);
      });
    };
    if (gridObserver) gridObserver.observe(e); else setTimeout(e.ocPlace);
  }

  /// Grilles à colonnes calculées : replacées quand leur largeur change.
  var gridObserver = window.ResizeObserver ? new ResizeObserver(function (entries) {
    entries.forEach(function (entry) { if (entry.target.ocPlace) entry.target.ocPlace(); });
  }) : null;

  function input(node, p, ctx) {
    var multiline = p.kind === "multiline";
    var e = el(multiline ? "textarea" : "input", "oc-input");
    if (!multiline) e.type = { password: "password", email: "email", number: "number" }[p.kind] || "text";
    if (node.type === "date" || node.type === "time") e.type = node.type;
    e.value = p.bind ? text(p, ctx) : (p.value == null ? "" : p.value);
    if (p.placeholder) e.placeholder = p.placeholder;
    if (p.readonly) e.readOnly = true;
    e.addEventListener(node.type === "input" ? "input" : "change", function () {
      changed(node.id, "value", e.value);
      fire(p, "onChange", node.id, e.value);
    });
    if (!multiline) e.addEventListener("keydown", function (ev) {
      if (ev.key === "Enter") fire(p, "onSubmit", node.id, e.value);
    });
    return e;
  }

  function check(node, p, ctx) {
    var e = el("label", "oc-" + node.type);
    var box = el("input");
    box.type = "checkbox";
    box.checked = p.bind ? text(p, ctx) === "true" : !!p.checked;
    box.addEventListener("change", function () {
      changed(node.id, "checked", box.checked);
      fire(p, "onChange", node.id, box.checked);
    });
    e.appendChild(box);
    if (node.type === "switch") e.appendChild(el("span", "oc-switch-track"));
    if (p.label) e.appendChild(document.createTextNode(p.label));
    return e;
  }

  function choice(node, p) {
    var pick = function (value) {
      changed(node.id, "value", value);
      fire(p, "onChange", node.id, value);
    };
    if (node.type === "select") {
      var select = el("select", "oc-input");
      (p.options || []).forEach(function (o) {
        var option = el("option");
        option.value = o.value;
        option.textContent = o.text;
        option.selected = o.value === p.value;
        select.appendChild(option);
      });
      select.addEventListener("change", function () { pick(select.value); });
      return select;
    }
    var group = el("div", "oc-radio");
    var name = node.id || "oc-radio-" + Math.random().toString(36).slice(2);
    (p.options || []).forEach(function (o) {
      var label = el("label");
      var radio = el("input");
      radio.type = "radio";
      radio.name = name;
      radio.checked = o.value === p.value;
      radio.addEventListener("change", function () { pick(o.value); });
      label.appendChild(radio);
      label.appendChild(document.createTextNode(o.text));
      group.appendChild(label);
    });
    return group;
  }

  function slider(node, p) {
    var e = el("input", "oc-slider");
    e.type = "range";
    e.min = p.min == null ? 0 : p.min;
    e.max = p.max == null ? 100 : p.max;
    e.step = p.step || "any";
    e.value = p.value == null ? e.min : p.value;
    e.addEventListener("input", function () {
      var value = Number(e.value);
      changed(node.id, "value", value);
      fire(p, "onChange", node.id, value);
    });
    return e;
  }

  /// Valeur courante du champ porté par `e` (case : cochée), `null` sinon.
  function fieldValue(e) {
    var field = e.matches("input, textarea, select") ? e : e.querySelector("input:checked, input, textarea, select");
    if (!field) return null;
    return field.type === "checkbox" ? field.checked : field.value;
  }

  var LONG_PRESS_MS = 500;
  var SWIPE_PX = 50;
  var MOVE_TOLERANCE_PX = 10;

  /// Appui long et balayage, au pointeur comme au toucher. Un appui long
  /// n'est pas aussi un clic.
  function gestures(e, p, send) {
    var start = null, timer = null, pressed = false;
    e.addEventListener("pointerdown", function (ev) {
      start = { x: ev.clientX, y: ev.clientY };
      pressed = false;
      if (p.onLongPress) timer = setTimeout(function () { timer = null; pressed = true; send("onLongPress"); }, LONG_PRESS_MS);
    });
    e.addEventListener("pointermove", function (ev) {
      if (start && timer && Math.hypot(ev.clientX - start.x, ev.clientY - start.y) > MOVE_TOLERANCE_PX) {
        clearTimeout(timer);
        timer = null;
      }
    });
    e.addEventListener("pointerup", function (ev) {
      clearTimeout(timer);
      timer = null;
      if (!start) return;
      var dx = ev.clientX - start.x, dy = ev.clientY - start.y;
      start = null;
      if (p.onSwipe && Math.max(Math.abs(dx), Math.abs(dy)) >= SWIPE_PX) {
        send("onSwipe", Math.abs(dx) > Math.abs(dy) ? (dx > 0 ? "right" : "left") : (dy > 0 ? "down" : "up"));
      }
    });
    e.addEventListener("pointercancel", function () { clearTimeout(timer); timer = null; start = null; });
    e.addEventListener("click", function (ev) {
      if (!pressed) return;
      pressed = false;
      ev.stopImmediatePropagation();
      ev.preventDefault();
    }, true);
  }

  /// Survol, focus et gestes de tout nœud. La valeur : celle donnée
  /// (direction d'un balayage), sinon l'élément de liste, sinon la valeur du
  /// champ ; dans une liste, le rang de l'élément.
  function events(e, node, p, ctx) {
    var item = ctx && ctx.item;
    function send(event, value) {
      if (value === undefined) value = item || fieldValue(e);
      fire(p, event, node.id, value, item ? ctx.index : undefined);
    }
    if (p.onHover) e.addEventListener("pointerenter", function () { send("onHover"); });
    if (p.onLeave) e.addEventListener("pointerleave", function () { send("onLeave"); });
    if (p.onFocus || p.onBlur) {
      // Le focus entre dans le nœud ou en sort (pas d'un enfant à l'autre).
      if (e.tabIndex < 0 && !e.querySelector("input, select, textarea, button")) e.tabIndex = 0;
      if (p.onFocus) e.addEventListener("focusin", function (ev) { if (!e.contains(ev.relatedTarget)) send("onFocus"); });
      if (p.onBlur) e.addEventListener("focusout", function (ev) { if (!e.contains(ev.relatedTarget)) send("onBlur"); });
    }
    if (p.onDoubleClick) e.addEventListener("dblclick", function () { send("onDoubleClick"); });
    if (p.onLongPress || p.onSwipe) gestures(e, p, send);
  }

  /// Focus demandé par l'hôte (premier champ obligatoire vide).
  function focusIn(e) {
    var field = e.matches("input, textarea, select, button") ? e : e.querySelector("input, textarea, select, button");
    (field || e).focus();
  }

  function button(node, p, ctx) {
    var e = el("button", "oc-button " + (node.type === "iconButton" ? "oc-icon-button" : "oc-variant-" + (p.variant || "primary")));
    e.type = "button";
    if (p.icon) e.appendChild(icon(p.icon));
    if (node.type === "button" && p.text != null) e.appendChild(document.createTextNode(p.text));
    onClick(e, node, p, ctx);
    return e;
  }

  /// Élément d'un nœud ; `axis` : sens du conteneur parent (`row`/`column`).
  function build(node, ctx, axis) {
    var p = props(node, ctx);
    var e;
    switch (node.type) {
      case "column": case "row": case "stack": case "grid": case "card":
        e = container(node, p, ctx, node.type === "row" ? "row" : "column"); break;
      case "divider": e = el("hr", "oc-divider"); break;
      case "spacer": e = el("div", "oc-spacer"); break;
      case "text": e = el("span", "oc-text"); e.textContent = text(p, ctx); break;
      case "heading":
        e = el("h" + (p.level || 2), "oc-heading"); e.textContent = text(p, ctx); break;
      case "image":
        e = el("img", "oc-image");
        e.src = p.bind ? text(p, ctx) : p.src;
        e.alt = p.label || "";
        if (p.fit) e.style.objectFit = p.fit;
        break;
      case "icon": e = icon(p.name); break;
      case "progress":
        e = el("progress", "oc-progress");
        if (p.value != null) e.value = p.value;
        break;
      case "list": e = list(node, p, ctx); break;
      case "input": case "date": case "time": e = input(node, p, ctx); break;
      case "checkbox": case "switch": e = check(node, p, ctx); break;
      case "radio": case "select": e = choice(node, p); break;
      case "slider": e = slider(node, p); break;
      case "button": case "iconButton": e = button(node, p, ctx); break;
      case "android.fab": case "android.bottomSheet": case "android.drawer":
        // Composants propres à Android : rien sur desktop.
        e = el("span"); e.hidden = true; break;
      case "webview":
        // Vraie vue web isolée, posée par l'hôte au rectangle de ce nœud
        // (`placeWebviews`) : ici, seulement la place réservée.
        e = el("div", "oc-webview");
        e.dataset.src = p.url || p.file || (p.html != null ? "html" : "");
        break;
      default: e = el("div");
    }
    decorate(e, p, axis, ["column", "row", "stack", "grid", "card", "list"].indexOf(node.type) >= 0);
    events(e, node, p, ctx);
    if (p.invalid) e.classList.add("oc-invalid");
    if (p.required) {
      var required = e.matches("input, textarea, select") ? e : e.querySelector("input, textarea, select");
      if (required) required.setAttribute("aria-required", "true");
    }
    if (p.enabled === false) {
      e.classList.add("oc-disabled");
      e.querySelectorAll("input, select, textarea, button").forEach(function (c) { c.disabled = true; });
      if ("disabled" in e) e.disabled = true;
    }
    if (node.id) {
      e.dataset.id = node.id;
      if (!ctx.item) mounted[node.id] = { node: node, ctx: ctx, axis: axis, el: e };
    }
    return e;
  }

  var placed = {};     // id de webview → dernier rectangle envoyé
  var placing = false;

  /// Rectangle de chaque composant `webview` affiché, envoyé à l'hôte qui y
  /// pose une vraie vue web ; `visible: false` pour celui qui a disparu.
  function placeWebviews() {
    placing = false;
    var main = document.querySelector(".oc-screen");
    var area = main ? main.getBoundingClientRect() : null;
    var seen = {};
    document.querySelectorAll(".oc-webview[data-id]").forEach(function (e) {
      var r = e.getBoundingClientRect();
      var visible = !e.closest("[hidden]") && r.width > 0 && r.height > 0 && !!area
        && r.bottom > area.top && r.top < area.bottom;
      var message = { webview: e.dataset.id, x: r.left, y: r.top, width: r.width, height: r.height, visible: visible };
      seen[e.dataset.id] = true;
      var key = JSON.stringify(message);
      if (placed[e.dataset.id] !== key) {
        placed[e.dataset.id] = key;
        send(message);
      }
    });
    for (var id in placed) {
      if (!seen[id] && placed[id] !== "hidden") {
        placed[id] = "hidden";
        send({ webview: id, x: 0, y: 0, width: 0, height: 0, visible: false });
      }
    }
  }

  /// Placement après le rendu en cours (`setTimeout` et non
  /// `requestAnimationFrame`, que le navigateur suspend pour une page cachée
  /// ou une fenêtre réduite : la vue web d'un écran quitté resterait affichée).
  function schedulePlacement() {
    if (placing) return;
    placing = true;
    setTimeout(placeWebviews, 0);
  }

  window.addEventListener("resize", schedulePlacement);
  document.addEventListener("scroll", schedulePlacement, true);
  var layoutObserver = window.ResizeObserver ? new ResizeObserver(schedulePlacement) : null;

  /// Classe de la largeur `width` et variante à afficher : la plus grande
  /// dont le seuil est atteint (docs/oc-ui.md, « Tailles d'écran »).
  function sizeOf(width) {
    var sizes = root.sizes || {}, variants = root.variants || {};
    var cls = "", best = "";
    Object.keys(sizes).sort(function (a, b) { return sizes[a] - sizes[b]; }).forEach(function (c) {
      if (width >= sizes[c]) {
        cls = c;
        if (variants[c]) best = c;
      }
    });
    return { cls: cls, variant: best };
  }

  /// Applique la variante de la largeur courante ; vrai si elle a changé.
  function chooseVariant() {
    var size = sizeOf(window.innerWidth);
    if (size.cls !== sizeClass) {
      sizeClass = size.cls;
      send({ action: "resize", id: null, value: sizeClass });
    }
    var next = size.variant ? root.variants[size.variant] : root;
    var changed = size.variant !== variant || next !== tree;
    variant = size.variant;
    tree = next;
    if (!tree.screens[current]) current = tree.start;
    history = history.filter(function (s) { return tree.screens[s]; });
    var win = tree.window || {};
    if (win.title) document.title = win.title;
    document.documentElement.dataset.theme = win.theme || "auto";
    if (win.primary) document.documentElement.style.setProperty("--oc-primary", win.primary);
    return changed;
  }

  // Thème `auto` : les surcharges `styles.dark`/`styles.light` suivent le système.
  if (darkQuery && darkQuery.addEventListener) darkQuery.addEventListener("change", function () {
    if (root && (tree.window || {}).theme !== "dark" && (tree.window || {}).theme !== "light") render();
  });

  window.addEventListener("resize", function () {
    if (root && chooseVariant()) {
      render();
      shown();
    }
  });

  /// Écran affiché (départ, navigation, retour, nouvel arbre) : son action
  /// `onShow`, comme une page rechargée.
  function shown() {
    var screen = tree.screens[current];
    if (screen && screen.onShow) send({ action: screen.onShow, id: null, value: current });
  }

  function render() {
    mounted = {};
    if (gridObserver) gridObserver.disconnect();
    var screen = tree.screens[current];
    var root = document.getElementById("oc-root");
    root.textContent = "";
    if (screen.title || history.length) {
      var bar = el("header", "oc-bar");
      if (history.length) {
        var back = button({ type: "iconButton" }, { icon: "back", onClick: "back" });
        back.setAttribute("aria-label", "Retour");
        bar.appendChild(back);
      }
      var title = el("span", "oc-bar-title");
      title.textContent = screen.title || "";
      bar.appendChild(title);
      root.appendChild(bar);
    }
    var body = el("main", "oc-screen");
    body.appendChild(build(screen.layout, {}, "column"));
    root.appendChild(body);
    if (layoutObserver) {
      layoutObserver.disconnect();
      layoutObserver.observe(body);
      document.querySelectorAll(".oc-webview").forEach(function (e) { layoutObserver.observe(e); });
    }
    schedulePlacement();
  }

  var OcaraUI = window.OcaraUI = {
    start: function (t, options) {
      root = t;
      platform = options.platform || "tauri";
      send = options.send || function () {};
      current = root.start;
      chooseVariant();
      render();
      send({ action: "ready", id: null, value: null });
      shown();
    },

    set: function (id, prop, value) {
      var m = mounted[id];
      if (prop === "focus") {
        if (m) focusIn(m.el);
        return;
      }
      (state[id] = state[id] || {})[prop] = value;
      if (!m) return;
      // Sans reconstruire : le champ en cours de saisie garde le focus.
      if (prop === "invalid") {
        m.el.classList.toggle("oc-invalid", !!value);
        return;
      }
      var input = m.el.matches("input, textarea, select") ? m.el : null;
      if (input && prop === "value" && document.activeElement === input) {
        input.value = value == null ? "" : value;
        return;
      }
      var fresh = build(m.node, m.ctx, m.axis);
      m.el.replaceWith(fresh);
      schedulePlacement();
    },

    get: function (id, prop) {
      var m = mounted[id];
      var p = m ? props(m.node, m.ctx) : (state[id] || {});
      return p[prop];
    },

    /// Nouvel arbre (variables, rechargement à chaud) : état et écran gardés.
    reload: function (t) {
      root = t;
      tree = null;
      chooseVariant();
      render();
      shown();
    },

    /// État complet (page rechargée : celui gardé par l'hôte).
    restore: function (saved) {
      for (var id in saved) {
        state[id] = state[id] || {};
        for (var prop in saved[id]) state[id][prop] = saved[id][prop];
      }
      render();
    },

    /// Message bref en bas de la fenêtre.
    toast: function (text) {
      var t = el("div", "oc-toast");
      t.textContent = text;
      document.body.appendChild(t);
      setTimeout(function () { t.classList.add("oc-toast-out"); }, 2600);
      setTimeout(function () { t.remove(); }, 3000);
    },

    /// Écran déjà ouvert plus tôt : on y revient (historique raccourci).
    /// L'écran affiché lui-même : son `onShow` est relancé (rechargement).
    navigate: function (screen) {
      if (!tree.screens[screen]) return;
      if (screen === current) return shown();
      var at = history.indexOf(screen);
      if (at >= 0) history = history.slice(0, at);
      else history.push(current);
      current = screen;
      render();
      shown();
    },

    back: function () {
      if (!history.length) return send({ action: "back", id: null, value: null });
      current = history.pop();
      render();
      shown();
    },
  };
})();
