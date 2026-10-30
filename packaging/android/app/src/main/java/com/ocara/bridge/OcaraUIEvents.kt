package com.ocara.bridge

import androidx.compose.foundation.focusable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import kotlin.math.abs

/** Distance minimale d'un balayage, en dp (même seuil que engine.js). */
private const val SWIPE_DP = 50f

/** Nœuds qui prennent le focus d'eux-mêmes ; les autres le reçoivent pour `onFocus`/`onBlur`. */
private val FOCUSABLE = setOf("input", "date", "time", "select", "radio", "checkbox", "switch", "slider", "button", "iconButton")

/**
 * Survol, focus et gestes de tout nœud (docs/oc-ui.md, « Événements »),
 * comme `events` dans runtime_tauri/src/ui/engine.js. Les gestes sont
 * observés sans être consommés (le clic d'un bouton reste à lui), sauf le
 * relâchement d'un appui long, qui n'est pas aussi un clic. Valeur : la
 * direction d'un balayage, sinon l'élément de liste, sinon la valeur du
 * champ ; dans une liste, son rang.
 */
@Composable
internal fun Modifier.events(p: Map<String, Any?>, id: String?, ctx: Ctx): Modifier {
    fun send(event: String, value: Any? = ctx.item ?: p["value"] ?: p["checked"]) = OcaraUIEngine.fire(p, event, id, value, ctx.index)
    var m = this
    if (p.containsKey("onHover") || p.containsKey("onLeave")) {
        m = m.pointerInput(p) {
            awaitPointerEventScope {
                while (true) {
                    when (awaitPointerEvent(PointerEventPass.Initial).type) {
                        PointerEventType.Enter -> send("onHover")
                        PointerEventType.Exit -> send("onLeave")
                    }
                }
            }
        }
    }
    val requester = remember { FocusRequester() }
    val request = id?.let { OcaraUIEngine.focusRequests[it] } ?: 0
    LaunchedEffect(request) {
        if (request > 0) runCatching { requester.requestFocus() }
    }
    m = m.focusRequester(requester)
    if (p.containsKey("onFocus") || p.containsKey("onBlur")) {
        val focused = remember { booleanArrayOf(false) }
        m = m.onFocusChanged { state ->
            if (state.hasFocus != focused[0]) {
                focused[0] = state.hasFocus
                send(if (state.hasFocus) "onFocus" else "onBlur")
            }
        }
        if (p["type"] !in FOCUSABLE) m = m.focusable()
    }
    if (p.containsKey("onLongPress") || p.containsKey("onDoubleClick") || p.containsKey("onSwipe")) {
        val lastTap = remember { longArrayOf(0L) }
        m = m.pointerInput(p) {
            awaitEachGesture {
                val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
                val pressedAt = System.currentTimeMillis()
                var position = down.position
                var longPressed = false
                while (true) {
                    val remaining = viewConfiguration.longPressTimeoutMillis - (System.currentTimeMillis() - pressedAt)
                    val waitFor = if (longPressed || !p.containsKey("onLongPress")) Long.MAX_VALUE else remaining.coerceAtLeast(1)
                    val event = withTimeoutOrNull(waitFor) { awaitPointerEvent(PointerEventPass.Initial) }
                    if (event == null) {
                        longPressed = true
                        if ((position - down.position).getDistance() < viewConfiguration.touchSlop) send("onLongPress")
                        continue
                    }
                    val change = event.changes.firstOrNull { it.id == down.id } ?: return@awaitEachGesture
                    position = change.position
                    if (!change.pressed) {
                        if (longPressed) change.consume()
                        break
                    }
                }
                val dx = position.x - down.position.x
                val dy = position.y - down.position.y
                if (p.containsKey("onSwipe") && maxOf(abs(dx), abs(dy)) >= SWIPE_DP * density) {
                    send("onSwipe", if (abs(dx) > abs(dy)) (if (dx > 0) "right" else "left") else (if (dy > 0) "down" else "up"))
                } else if (!longPressed && p.containsKey("onDoubleClick")) {
                    val now = System.currentTimeMillis()
                    if (now - lastTap[0] <= viewConfiguration.doubleTapTimeoutMillis) {
                        lastTap[0] = 0L
                        send("onDoubleClick")
                    } else {
                        lastTap[0] = now
                    }
                }
            }
        }
    }
    return m
}
