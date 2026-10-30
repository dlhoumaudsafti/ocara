// Styles d'état, ombres, flou, transitions et animations du moteur desktop
// d'`ocara.UI` (docs/oc-ui.md, « Styles d'état et effets », « Animations »),
// chargé avant engine.js :
//
//   OcaraEffects.setTree(tree)                    animations du `.oc.ui` (@keyframes)
//   OcaraEffects.apply(e, p)                      effets d'un nœud (styles en ligne, classe d'états)
//   OcaraEffects.animate(e, name, anim, options)  `ui.animate`
//   OcaraEffects.stop(e)                          `ui.stopAnimation`
//
// Une table d'état (`states: { hover: {…} }`) devient une règle CSS sur une classe
// partagée par les nœuds aux mêmes états (`.oc-fx3:hover { … }`), marquée
// `!important` pour passer devant les styles en ligne du nœud.
var OcaraEffects = (function () {
  "use strict";

  /// État → sélecteur ajouté à la classe du nœud.
  var STATES = {
    hover: ":hover",
    focus: ":focus-within",
    active: ":active",
    disabled: ".oc-disabled",
    invalid: ".oc-invalid",
    checked: ":has(input:checked)",
    visited: ".oc-visited",
  };

  /// Propriété de style → propriété CSS animée par `transition`.
  var TRANSITIONS = {
    fontSize: "font-size", color: "color", background: "background", padding: "padding",
    margin: "margin", radius: "border-radius", border: "border-color", elevation: "box-shadow",
    shadow: "box-shadow", opacity: "opacity", blur: "filter", backdropBlur: "backdrop-filter",
  };

  var stateSheet = null, keyframeSheet = null;
  var classes = {};   // texte des règles → classe
  var count = 0;

  function sheet(id) {
    var e = document.createElement("style");
    e.id = id;
    document.head.appendChild(e);
    return e;
  }

  function px(v) { return typeof v === "number" ? v + "px" : v; }

  function spacing(v) {
    return Array.isArray(v) ? v.map(px).join(" ") : px(v);
  }

  function shadows(list) {
    return list.map(function (s) {
      return (s.inset ? "inset " : "") + [s.x, s.y, s.blur, s.spread].map(px).join(" ") + " " + s.color;
    }).join(", ");
  }

  /// Déclarations CSS des propriétés de style `p` (sans les états).
  function declarations(p) {
    var d = {};
    if (p.fontSize != null) d["font-size"] = px(p.fontSize);
    if (p.bold) d["font-weight"] = "bold";
    if (p.italic) d["font-style"] = "italic";
    if (p.color) d.color = p.color;
    if (p.background) d.background = p.background;
    if (p.padding != null) d.padding = spacing(p.padding);
    if (p.margin != null) d.margin = spacing(p.margin);
    if (p.radius != null) d["border-radius"] = px(p.radius);
    if (p.border) d.border = "1px solid " + p.border;
    if (p.elevation) d["box-shadow"] = "0 " + p.elevation / 2 + "px " + p.elevation * 1.5 + "px rgba(0,0,0,.25)";
    if (p.shadow) d["box-shadow"] = shadows(p.shadow);
    if (p.opacity != null) d.opacity = p.opacity;
    if (p.blur != null) d.filter = "blur(" + px(p.blur) + ")";
    if (p.backdropBlur != null) d["backdrop-filter"] = d["-webkit-backdrop-filter"] = "blur(" + px(p.backdropBlur) + ")";
    return d;
  }

  function cssText(d, important) {
    return Object.keys(d).map(function (k) { return k + ": " + d[k] + (important ? " !important" : ""); }).join("; ");
  }

  function transition(t) {
    var names = (t.properties || []).map(function (n) { return TRANSITIONS[n]; }).filter(Boolean);
    var timing = (t.duration || 0) + "ms " + (t.easing || "ease") + " " + (t.delay || 0) + "ms";
    return (names.length ? names : ["all"]).map(function (n) { return n + " " + timing; }).join(", ");
  }

  /// Valeur CSS `animation` de l'animation `name` (`options` : surcharges de `ui.animate`).
  function animationValue(name, anim, options) {
    var a = {};
    Object.keys(anim).forEach(function (k) { a[k] = anim[k]; });
    Object.keys(options || {}).forEach(function (k) { a[k] = options[k]; });
    return ["oc-a-" + name, a.duration + "ms", a.easing, a.delay + "ms", a.iterations, a.direction, a.fill].join(" ");
  }

  /// Étape d'une animation en déclarations CSS (transformations réunies).
  function step(k) {
    var d = {}, transform = [];
    if (k.translateX != null || k.translateY != null) transform.push("translate(" + px(k.translateX || 0) + ", " + px(k.translateY || 0) + ")");
    if (k.scale != null) transform.push("scale(" + k.scale + ")");
    if (k.rotate != null) transform.push("rotate(" + k.rotate + "deg)");
    if (transform.length) d.transform = transform.join(" ");
    if (k.opacity != null) d.opacity = k.opacity;
    if (k.background) d["background-color"] = k.background;
    if (k.color) d.color = k.color;
    if (k.blur != null) d.filter = "blur(" + px(k.blur) + ")";
    if (k.width != null) d.width = px(k.width);
    if (k.height != null) d.height = px(k.height);
    return d;
  }

  function setTree(tree) {
    keyframeSheet = keyframeSheet || sheet("oc-keyframes");
    var animations = tree.animations || {};
    keyframeSheet.textContent = Object.keys(animations).map(function (name) {
      var steps = animations[name].keyframes.map(function (k) { return k.at + "% { " + cssText(step(k)) + " }"; });
      return "@keyframes oc-a-" + name + " { " + steps.join(" ") + " }";
    }).join("\n");
  }

  /// Classe portant les règles d'état (et les animations déclenchées par un
  /// état) de `p` ; `""` s'il n'en a pas.
  function stateClass(p, animations) {
    var rules = [];
    var states = p.states || {};
    Object.keys(STATES).forEach(function (state) {
      if (states[state]) rules.push([STATES[state], cssText(declarations(states[state]), true)]);
    });
    var a = p.animation;
    if (a && a.on !== "appear" && animations[a.name]) {
      var selector = { hover: ":hover", active: ":active", focus: ":focus-within" }[a.on];
      rules.push([selector, "animation: " + animationValue(a.name, animations[a.name]) + " !important"]);
    }
    if (!rules.length) return "";
    var key = JSON.stringify(rules);
    if (!classes[key]) {
      var cls = classes[key] = "oc-fx" + (++count);
      stateSheet = stateSheet || sheet("oc-states");
      stateSheet.appendChild(document.createTextNode(rules.map(function (r) {
        return "." + cls + r[0] + " { " + r[1] + " }";
      }).join("\n") + "\n"));
    }
    return classes[key];
  }

  /// `states` de deux couches de propriétés : chaque table d'état de `over`
  /// complète celle de `base`.
  function mergeStates(base, over) {
    var out = {};
    [base || {}, over || {}].forEach(function (layer) {
      Object.keys(layer).forEach(function (s) { out[s] = Object.assign({}, out[s], layer[s]); });
    });
    return out;
  }

  /// Effets du nœud `e` (propriétés effectives `p`) ; `animations` : celles
  /// de l'arbre affiché.
  function apply(e, p, animations) {
    var s = e.style;
    if (p.shadow) s.boxShadow = shadows(p.shadow);
    if (p.opacity != null) s.opacity = p.opacity;
    if (p.blur != null) s.filter = "blur(" + px(p.blur) + ")";
    if (p.backdropBlur != null) s.backdropFilter = s.webkitBackdropFilter = "blur(" + px(p.backdropBlur) + ")";
    if (p.transition) s.transition = transition(p.transition);
    var a = p.animation;
    if (a && a.on === "appear" && animations[a.name]) s.animation = animationValue(a.name, animations[a.name]);
    var cls = stateClass(p, animations);
    if (cls) e.classList.add(cls);
  }

  /// `ui.animate` : relance l'animation même si elle tourne déjà.
  function animate(e, name, anim, options) {
    e.style.animation = "none";
    void e.offsetWidth;
    e.style.animation = animationValue(name, anim, options);
  }

  function stop(e) {
    e.style.animation = "none";
  }

  return { px: px, spacing: spacing, mergeStates: mergeStates, setTree: setTree, apply: apply, animate: animate, stop: stop };
})();
