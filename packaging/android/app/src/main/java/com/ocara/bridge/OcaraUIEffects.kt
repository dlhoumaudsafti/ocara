package com.ocara.bridge

import android.provider.Settings
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.blur
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import org.json.JSONArray
import org.json.JSONObject

/**
 * Styles d'état, effets et animations d'un nœud (docs/oc-ui.md, « Styles
 * d'état et effets », « Animations »), comme runtime_tauri/src/ui/effects.js.
 * Animés ici : `opacity`, `scale`, `rotate`, `translateX`/`Y`, `background` ;
 * `color`, `blur`, `width`/`height` des étapes et `backdropBlur` ne le sont
 * que sur desktop.
 */

/** États tirés de l'interaction ; les autres viennent des propriétés. */
internal class NodeStates {
    var hovered by mutableStateOf(false)
    var pressed by mutableStateOf(false)
    var focused by mutableStateOf(false)
}

/** Ordre de fusion des tables d'état (comme `STATES` d'effects.js). */
private val STATES = listOf("hover", "focus", "active", "disabled", "invalid", "checked", "visited")

/** Valeurs animées d'un nœud à l'instant affiché. */
internal data class Frame(
    val opacity: Float = 1f,
    val scale: Float = 1f,
    val rotate: Float = 0f,
    val translateX: Float = 0f,
    val translateY: Float = 0f,
    val background: Color? = null,
)

private val EASINGS: Map<String, Easing> = mapOf(
    "linear" to LinearEasing,
    "ease" to CubicBezierEasing(0.25f, 0.1f, 0.25f, 1f),
    "ease-in" to CubicBezierEasing(0.42f, 0f, 1f, 1f),
    "ease-out" to CubicBezierEasing(0f, 0f, 0.58f, 1f),
    "ease-in-out" to CubicBezierEasing(0.42f, 0f, 0.58f, 1f),
)

/** Propriétés `p` avec les tables des états actifs par-dessus. */
internal fun withStates(p: Map<String, Any?>, s: NodeStates): Map<String, Any?> {
    val active = mapOf(
        "hover" to s.hovered, "focus" to s.focused, "active" to s.pressed,
        "disabled" to (p["enabled"] == false), "invalid" to (p["invalid"] == true),
        "checked" to (p["checked"] == true), "visited" to (p["visited"] == true),
    )
    val states = p["states"] as? JSONObject ?: return p
    val out = HashMap(p)
    for (state in STATES) {
        val table = states.optJSONObject(state) ?: continue
        if (active[state] != true) continue
        for (key in table.keys()) out[key] = OcaraUIEngine.plain(table.get(key))
    }
    return out
}

/** Le nœud dépend-il du survol, de l'appui ou du focus ? */
internal fun watchesInteraction(p: Map<String, Any?>): Boolean =
    listOf("hover", "focus", "active").any { (p["states"] as? JSONObject)?.has(it) == true } ||
        (p["animation"] as? JSONObject)?.optString("on") in setOf("hover", "active", "focus")

/** Réglage « réduire les animations » du système (échelle des durées à 0). */
@Composable
private fun reducedMotion(): Boolean {
    val resolver = LocalContext.current.contentResolver
    return remember { Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f }
}

/** `transition` : `background`, `color` et `opacity` passent en douceur. */
@Composable
internal fun transitioned(p: Map<String, Any?>): Map<String, Any?> {
    val t = p["transition"] as? JSONObject ?: return p
    val names = t.optJSONArray("properties")?.let { a -> (0 until a.length()).map { a.getString(it) } }
    val duration = if (reducedMotion()) 0 else t.optInt("duration", 0)
    val easing = EASINGS[t.optString("easing", "ease")] ?: LinearEasing
    fun animates(name: String) = names == null || name in names
    val out = HashMap(p)
    for (name in listOf("background", "color")) {
        val target = parseColor(p[name]) ?: continue
        if (!animates(name)) continue
        val color by animateColorAsState(target, tween(duration, t.optInt("delay", 0), easing), label = name)
        out[name] = color
    }
    (p["opacity"] as? Number)?.takeIf { animates("opacity") }?.let {
        val opacity by animateFloatAsState(it.toFloat(), tween(duration, t.optInt("delay", 0), easing), label = "opacity")
        out["opacity"] = opacity
    }
    return out
}

/** Valeur de `prop` à la position `at` (en %) des étapes `steps`. */
private fun interpolate(steps: JSONArray, prop: String, at: Float, mix: (Any, Any, Float) -> Any): Any? {
    val points = (0 until steps.length()).map { steps.getJSONObject(it) }.filter { it.has(prop) }
    if (points.isEmpty()) return null
    val after = points.indexOfFirst { it.getDouble("at") >= at }
    if (after <= 0) return points[if (after == 0) 0 else points.size - 1].get(prop)
    val (a, b) = points[after - 1] to points[after]
    val span = (b.getDouble("at") - a.getDouble("at")).toFloat()
    val f = if (span == 0f) 1f else (at - a.getDouble("at").toFloat()) / span
    return mix(a.get(prop), b.get(prop), f)
}

private fun frameAt(steps: JSONArray, progress: Float): Frame {
    val at = progress * 100f
    fun number(prop: String, default: Float) =
        (interpolate(steps, prop, at) { a, b, f -> (a as Number).toFloat() + ((b as Number).toFloat() - a.toFloat()) * f } as? Number)?.toFloat() ?: default
    val background = interpolate(steps, "background", at) { a, b, f ->
        lerp(parseColor(a) ?: Color.Transparent, parseColor(b) ?: Color.Transparent, f)
    }
    return Frame(
        opacity = number("opacity", 1f),
        scale = number("scale", 1f),
        rotate = number("rotate", 0f),
        translateX = number("translateX", 0f),
        translateY = number("translateY", 0f),
        background = background as? Color ?: parseColor(background),
    )
}

/**
 * Animation du nœud : demande de `ui.animate` (la plus récente), sinon celle
 * déclarée (`animation`) tant que son déclencheur est actif. `onAnimationEnd`
 * part à la fin d'une animation finie.
 */
@Composable
internal fun animationFrame(p: Map<String, Any?>, states: NodeStates, id: String?, ctx: Ctx): Frame {
    val animations = ctx.tree.optJSONObject("animations") ?: return Frame()
    val declared = p["animation"] as? JSONObject
    val request = id?.let { OcaraUIEngine.animationRequests[it] }
    val triggered = when (declared?.optString("on")) {
        "appear" -> true
        "hover" -> states.hovered
        "active" -> states.pressed
        "focus" -> states.focused
        else -> false
    }
    val (name, options, key) = when {
        request != null -> Triple(request.name, request.options, "ui.animate ${request.count}")
        declared != null && triggered -> Triple(declared.optString("name"), null, "déclarée")
        else -> return Frame()
    }
    val base = name?.let { animations.optJSONObject(it) } ?: return Frame()
    val anim = JSONObject(base.toString()).also { a -> options?.keys()?.forEach { a.put(it, options.get(it)) } }
    val progress = remember(key) { Animatable(0f) }
    var finished by remember(key) { mutableStateOf(false) }
    val reduced = reducedMotion()
    LaunchedEffect(key) {
        delay(anim.optLong("delay", 0))
        val iterations = if (anim.optString("iterations") == "infinite") Int.MAX_VALUE else anim.optInt("iterations", 1)
        val direction = anim.optString("direction", "normal")
        val spec = tween<Float>(if (reduced) 0 else anim.optInt("duration", 300), easing = EASINGS[anim.optString("easing")] ?: LinearEasing)
        var i = 0
        while (i < iterations) {
            val odd = i % 2 == 1
            val reverse = when (direction) {
                "reverse" -> true
                "alternate" -> odd
                "alternate-reverse" -> !odd
                else -> false
            }
            progress.snapTo(if (reverse) 1f else 0f)
            progress.animateTo(if (reverse) 0f else 1f, spec)
            i++
        }
        finished = true
        OcaraUIEngine.fire(p, "onAnimationEnd", id, name, ctx.index)
    }
    if (finished && anim.optString("fill", "none") in setOf("none", "backwards")) return Frame()
    return frameAt(anim.getJSONArray("keyframes"), progress.value)
}

/**
 * Suivi du survol, de l'appui et du focus (si le nœud en dépend) et
 * transformations animées, sur tout le nœud (ombre comprise) : à placer
 * avant sa décoration.
 */
internal fun Modifier.effects(states: NodeStates, frame: Frame, watch: Boolean): Modifier {
    var m = this
    if (watch) {
        m = m.pointerInput(Unit) {
            awaitPointerEventScope {
                while (true) {
                    when (awaitPointerEvent(PointerEventPass.Initial).type) {
                        PointerEventType.Enter -> states.hovered = true
                        PointerEventType.Exit -> { states.hovered = false; states.pressed = false }
                        PointerEventType.Press -> states.pressed = true
                        PointerEventType.Release -> states.pressed = false
                    }
                }
            }
        }.onFocusChanged { states.focused = it.hasFocus }
    }
    if (frame != Frame(opacity = frame.opacity, background = frame.background)) {
        m = m.graphicsLayer {
            scaleX = frame.scale
            scaleY = frame.scale
            rotationZ = frame.rotate
            translationX = frame.translateX * density
            translationY = frame.translateY * density
        }
    }
    return m
}

/**
 * Opacité et flou du nœud, posés par sa décoration juste après l'ombre :
 * le calque ne coupe pas l'ombre, et le contenu s'estompe d'un bloc (l'ombre,
 * elle, garde son intensité).
 */
internal fun fade(p: Map<String, Any?>, frame: Frame): Modifier {
    var m: Modifier = Modifier
    val opacity = ((p["opacity"] as? Number)?.toFloat() ?: 1f) * frame.opacity
    if (opacity < 1f) m = m.graphicsLayer { alpha = opacity }
    (p["blur"] as? Number)?.let { m = m.blur(it.toFloat().dp) }
    return m
}
