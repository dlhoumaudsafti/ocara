// Moteur de rendu desktop d'`ocara.UI` : construit le DOM de l'arbre
// normalisé d'un `.oc.ui` (src/core/ui_schema.d, docs/oc-ui.md) dans la vue
// de la fenêtre Tauri, sans framework.
//
//   OcaraUI.start(tree, { platform, send })  affiche l'écran `start`
//   OcaraUI.set(id, prop, value)             état d'un nœud (réaffiche ce nœud)
//   OcaraUI.navigate(screen), OcaraUI.back()
//
// `send(message)` remet chaque événement à l'hôte : `{ id, prop, value }`
// pour une valeur changée par l'utilisateur (état gardé côté Ocara),
// `{ action, id, value, index }` pour une action déclarée (`onClick`…).
(function () {
  "use strict";

  var tree, platform, send;
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
  function props(node) {
    var out = {};
    var named = node.style && tree.styles && tree.styles[node.style];
    var layers = [named || {}, node, node[platform] || {}, (node.id && state[node.id]) || {}];
    layers.forEach(function (layer) {
      for (var k in layer) if (k !== "tauri" && k !== "android") out[k] = layer[k];
    });
    return out;
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

  function icon(name) {
    var path = tree.icons && tree.icons[name];
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
      e.addEventListener("click", function () { fire(p, "onClick", node.id); });
    }
    return e;
  }

  function list(node, p, ctx) {
    var e = el("div", "oc-list");
    if (p.spacing != null) e.style.gap = px(p.spacing);
    (p.items || []).forEach(function (row, index) {
      var item = build(node.item, { item: row, index: index }, "column");
      item.classList.add("oc-list-item");
      if (p.onSelect) item.addEventListener("click", function () { fire(p, "onSelect", node.id, row, index); });
      e.appendChild(item);
    });
    return e;
  }

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
    e.addEventListener("focus", function () { fire(p, "onFocus", node.id, e.value); });
    e.addEventListener("blur", function () { fire(p, "onBlur", node.id, e.value); });
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

  function button(node, p) {
    var e = el("button", "oc-button " + (node.type === "iconButton" ? "oc-icon-button" : "oc-variant-" + (p.variant || "primary")));
    e.type = "button";
    if (p.icon) e.appendChild(icon(p.icon));
    if (node.type === "button" && p.text != null) e.appendChild(document.createTextNode(p.text));
    e.addEventListener("click", function () { fire(p, "onClick", node.id); });
    return e;
  }

  /// Élément d'un nœud ; `axis` : sens du conteneur parent (`row`/`column`).
  function build(node, ctx, axis) {
    var p = props(node);
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
      case "button": case "iconButton": e = button(node, p); break;
      case "webview":
        // Vraie vue web isolée, posée par l'hôte (étape 6) : réserve sa place.
        e = el("div", "oc-webview"); break;
      default: e = el("div");
    }
    decorate(e, p, axis, ["column", "row", "stack", "grid", "card", "list"].indexOf(node.type) >= 0);
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

  function render() {
    mounted = {};
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
  }

  var OcaraUI = window.OcaraUI = {
    start: function (t, options) {
      tree = t;
      platform = options.platform || "tauri";
      send = options.send || function () {};
      current = tree.start;
      var win = tree.window || {};
      if (win.title) document.title = win.title;
      document.documentElement.dataset.theme = win.theme || "auto";
      if (win.primary) document.documentElement.style.setProperty("--oc-primary", win.primary);
      render();
      send({ action: "ready", id: null, value: null });
    },

    set: function (id, prop, value) {
      (state[id] = state[id] || {})[prop] = value;
      var m = mounted[id];
      if (!m) return;
      var input = m.el.matches("input, textarea, select") ? m.el : null;
      if (input && prop === "value" && document.activeElement === input) {
        input.value = value == null ? "" : value;
        return;
      }
      var fresh = build(m.node, m.ctx, m.axis);
      m.el.replaceWith(fresh);
    },

    get: function (id, prop) {
      var m = mounted[id];
      var p = m ? props(m.node) : (state[id] || {});
      return p[prop];
    },

    navigate: function (screen) {
      if (!tree.screens[screen] || screen === current) return;
      history.push(current);
      current = screen;
      render();
    },

    back: function () {
      if (!history.length) return send({ action: "back", id: null, value: null });
      current = history.pop();
      render();
    },
  };
})();
