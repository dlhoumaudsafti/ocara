package com.ocara.bridge

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import org.json.JSONObject

/**
 * Liste en grille (`display: grid`, docs/oc-ui.md), même règle que le moteur
 * desktop (engine.js, `grid`). Sans `itemWidth`, chaque case garde sa largeur
 * naturelle et passe à la ligne quand la place manque (`minColumns` /
 * `maxColumns` bornent sa largeur) ; avec `itemWidth` (50 par défaut en
 * serpentin), le nombre de colonnes suit la largeur de la liste et chaque
 * case est placée selon `direction` : `ltr`, `rtl`, `ltr-snake`, `rtl-snake`.
 * `cell(rang, élément, modifier)` dessine une case ; le rang reste celui des données.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun ListGrid(
    p: Map<String, Any?>,
    rows: List<JSONObject>,
    gap: Dp,
    modifier: Modifier,
    cell: @Composable (Int, JSONObject, Modifier) -> Unit,
) {
    val dir = p["direction"] as? String ?: "ltr"
    val snake = dir.endsWith("snake")
    val rtl = dir.startsWith("rtl")
    val width = (p["itemWidth"] as? Number)?.toFloat() ?: if (snake) 50f else null
    val minColumns = (p["minColumns"] as? Number)?.toInt()
    val maxColumns = (p["maxColumns"] as? Number)?.toInt()
    BoxWithConstraints(modifier) {
        if (width == null) {
            val total = maxWidth
            val least = maxColumns?.let { (total - gap * (it - 1)) / it } ?: Dp.Unspecified
            val most = minColumns?.let { (total - gap * (it - 1)) / it } ?: Dp.Infinity
            val outer = LocalLayoutDirection.current
            // De droite à gauche : la rangée s'inverse, pas le contenu des cases.
            CompositionLocalProvider(LocalLayoutDirection provides if (rtl) LayoutDirection.Rtl else LayoutDirection.Ltr) {
                FlowRow(Modifier.fillMaxWidth(), Arrangement.spacedBy(gap), Arrangement.spacedBy(gap)) {
                    rows.forEachIndexed { index, row ->
                        CompositionLocalProvider(LocalLayoutDirection provides outer) {
                            cell(index, row, Modifier.widthIn(min = least, max = most))
                        }
                    }
                }
            }
            return@BoxWithConstraints
        }
        val fit = ((maxWidth.value + gap.value) / (width + gap.value)).toInt()
        val columns = fit.coerceIn(minColumns ?: 1, maxColumns ?: Int.MAX_VALUE).coerceAtLeast(1)
        Column(Modifier.fillMaxWidth(), Arrangement.spacedBy(gap)) {
            rows.withIndex().chunked(columns).forEachIndexed { line, chunk ->
                val reversed = rtl != (snake && line % 2 == 1)
                Row(Modifier.fillMaxWidth(), Arrangement.spacedBy(gap)) {
                    for (col in 0 until columns) {
                        val slot = chunk.getOrNull(if (reversed) columns - 1 - col else col)
                        if (slot == null) Spacer(Modifier.weight(1f))
                        else cell(slot.index, slot.value, Modifier.weight(1f))
                    }
                }
            }
        }
    }
}
