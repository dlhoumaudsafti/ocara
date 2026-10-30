package com.ocara.bridge

import android.os.Build
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithCache
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.innerShadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.addOutline
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.layer.GraphicsLayer
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.ui.graphics.shadow.DropShadowPainter
import androidx.compose.ui.graphics.shadow.Shadow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import org.json.JSONArray

/**
 * Ombres et flou d'arrière-plan d'un nœud (docs/oc-ui.md, « Styles d'état et
 * effets »), comme `box-shadow` et `backdrop-filter` du moteur desktop :
 * ombres portées dessinées autour de la forme seulement (rien sous un nœud
 * translucide), ombres `inset` par-dessus le fond.
 */

/** Ombres de `shadow` : (portées, intérieures). */
internal fun shadowsOf(value: Any?): Pair<List<Shadow>, List<Shadow>> {
    val list = value as? JSONArray ?: return emptyList<Shadow>() to emptyList()
    val all = (0 until list.length()).mapNotNull { list.optJSONObject(it) }.map { s ->
        s.optBoolean("inset") to Shadow(
            radius = s.optDouble("blur", 0.0).toFloat().dp,
            color = parseColor(s.opt("color")) ?: Color.Black,
            spread = s.optDouble("spread", 0.0).toFloat().dp,
            offset = DpOffset(s.optDouble("x", 0.0).toFloat().dp, s.optDouble("y", 0.0).toFloat().dp),
        )
    }
    return all.filter { !it.first }.map { it.second } to all.filter { it.first }.map { it.second }
}

/** Ombre d'`elevation` comme engine.css (`0 e/2 e*1.5 rgba(0,0,0,.25)`). */
internal fun elevationShadow(elevation: Float) =
    Shadow(radius = (elevation * 1.5f).dp, color = Color(0x40000000), offset = DpOffset(0.dp, (elevation / 2).dp))

/** Ombres portées, hors de la forme `shape` (dans l'ordre de `box-shadow` : la première devant). */
internal fun Modifier.outerShadows(shadows: List<Shadow>, shape: Shape): Modifier {
    if (shadows.isEmpty()) return this
    return drawWithCache {
        val outline = Path().apply { addOutline(shape.createOutline(size, layoutDirection, this@drawWithCache)) }
        val painters = shadows.reversed().map { DropShadowPainter(shape, it) }
        onDrawBehind {
            clipPath(outline, ClipOp.Difference) {
                for (painter in painters) with(painter) { draw(size) }
            }
        }
    }
}

/** Ombres intérieures (`inset`), sur le fond. */
internal fun Modifier.innerShadows(shadows: List<Shadow>, shape: Shape): Modifier =
    shadows.reversed().fold(this) { m, s -> m.innerShadow(shape, s) }

/** Contenu de l'écran enregistré dans un calque, flouté sous les nœuds de sa couche. */
internal class Backdrop(val layer: GraphicsLayer) {
    var origin by mutableStateOf(Offset.Zero)
}

internal val LocalBackdrop = staticCompositionLocalOf<Backdrop?> { null }

@Composable
internal fun rememberBackdrop(): Backdrop {
    val layer = rememberGraphicsLayer()
    return remember(layer) { Backdrop(layer) }
}

/** Contenu de l'écran : dessiné par le calque de `backdrop`. */
internal fun Modifier.backdropSource(backdrop: Backdrop): Modifier =
    onGloballyPositioned { backdrop.origin = it.positionInRoot() }
        .drawWithContent {
            backdrop.layer.record { this@drawWithContent.drawContent() }
            drawLayer(backdrop.layer)
        }

/**
 * `backdropBlur` : contenu de l'écran flouté sous la forme du nœud. Android
 * 12 et plus (`RenderEffect`), nœuds de la couche de l'écran seulement (le
 * calque ne peut pas contenir le nœud qui le dessine).
 */
@Composable
internal fun Modifier.backdropBlur(radius: Float?, shape: Shape): Modifier {
    val backdrop = LocalBackdrop.current
    if (radius == null || radius <= 0f || backdrop == null || Build.VERSION.SDK_INT < Build.VERSION_CODES.S) return this
    val blurred = rememberGraphicsLayer()
    var at by remember { mutableStateOf(Offset.Zero) }
    return onGloballyPositioned { at = it.positionInRoot() }.drawWithCache {
        val outline = Path().apply { addOutline(shape.createOutline(size, layoutDirection, this@drawWithCache)) }
        val px = radius.dp.toPx()
        onDrawBehind {
            blurred.renderEffect = BlurEffect(px, px, TileMode.Clamp)
            blurred.record(backdrop.layer.size) { drawLayer(backdrop.layer) }
            val shift = backdrop.origin - at
            clipPath(outline) { translate(shift.x, shift.y) { drawLayer(blurred) } }
        }
    }
}
