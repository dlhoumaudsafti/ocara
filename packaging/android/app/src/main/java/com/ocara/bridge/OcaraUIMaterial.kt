package com.ocara.bridge

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.PermanentDrawerSheet
import androidx.compose.material3.PermanentNavigationDrawer
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

/**
 * Composants d'écran (`.oc.ui`, docs/oc-ui.md « Composants d'écran ») :
 * bouton flottant (`fab`), tiroir de navigation (`drawer`), panneaux
 * inférieurs (`bottomSheet`) et conteneurs flottants fixes (`floating`).
 * Ils sont déclarés n'importe où dans la disposition de l'écran et placés
 * au-dessus d'elle ; le premier bouton flottant et le premier tiroir
 * comptent. Tiroir et panneau s'ouvrent par `ui.set(id, "open", true)` (le
 * tiroir aussi par l'icône de menu de la barre de titre).
 */
internal class MaterialNodes(layout: JSONObject) {
    var fab: JSONObject? = null
    var drawer: JSONObject? = null
    val sheets = ArrayList<JSONObject>()
    val floatings = ArrayList<JSONObject>()

    init {
        collect(layout)
    }

    private fun collect(node: JSONObject) {
        when (node.optString("type")) {
            "fab" -> if (fab == null) fab = node
            "drawer" -> if (drawer == null) drawer = node
            "bottomSheet" -> sheets.add(node)
            "floating" -> if (node.optString("position") != "absolute") floatings.add(node)
        }
        val children = node.optJSONArray("children") ?: return
        for (i in 0 until children.length()) children.optJSONObject(i)?.let { collect(it) }
    }
}

/** État `open` d'un tiroir ou d'un panneau (posé par Ocara ou par l'utilisateur). */
private fun isOpen(node: JSONObject, ctx: Ctx): Boolean = props(node, ctx)["open"] == true

private fun closed(node: JSONObject, ctx: Ctx) {
    val id = node.optString("id")
    OcaraUIEngine.changed(id, "open", false)
    OcaraUIEngine.fire(props(node, ctx), "onDismiss", id)
}

@Composable
internal fun Fab(node: JSONObject, ctx: Ctx) {
    val p = props(node, ctx)
    if (p["visible"] == false) return
    val id = node.optString("id").takeIf { it.isNotEmpty() }
    val click = { OcaraUIEngine.fire(p, "onClick", id) }
    val label = p["label"] as? String ?: ""
    val text = p["text"] as? String
    val icon = @Composable { NamedIcon(ctx, p["icon"] as? String, null, Modifier, null) }
    val modifier = Modifier.semantics { contentDescription = label }
    if (text != null) ExtendedFloatingActionButton(text = { Text(text) }, icon = icon, onClick = click, modifier = modifier)
    else FloatingActionButton(onClick = click, modifier = modifier) { icon() }
}

/** Point de référence (`position` d'un `fab`, `anchor` d'un `floating`). */
internal fun alignmentOf(name: String?): Alignment = when (name) {
    "topStart" -> Alignment.TopStart
    "topCenter" -> Alignment.TopCenter
    "topEnd" -> Alignment.TopEnd
    "startCenter" -> Alignment.CenterStart
    "center" -> Alignment.Center
    "endCenter" -> Alignment.CenterEnd
    "bottomStart" -> Alignment.BottomStart
    "bottomCenter" -> Alignment.BottomCenter
    else -> Alignment.BottomEnd
}

/** Décalage `x`/`y` (dp) vers l'intérieur depuis le point de référence `anchor`. */
internal fun Modifier.inward(anchor: String?, x: Float, y: Float): Modifier {
    val dx = if (anchor?.contains("End") == true || anchor == "endCenter") -x else x
    val dy = if (anchor?.startsWith("bottom") == true) -y else y
    return offset(dx.dp, dy.dp)
}

/** Ordre des classes de taille (variants.rs), pour `permanentFrom`. */
private val SIZE_ORDER = listOf("xxs", "xs", "sm", "md", "lg", "xl", "xxl")

/**
 * Écran entouré du tiroir `drawer` s'il y en a un ; `content` reçoit l'action
 * d'ouverture (icône de menu), `null` sans tiroir ou s'il est permanent
 * (`permanentFrom` atteint par la classe de taille `sizeClass`). Côté
 * `end` : sens de lecture inversé autour du tiroir seulement.
 */
@Composable
internal fun WithDrawer(drawer: JSONObject?, ctx: Ctx, sizeClass: String, content: @Composable (open: (() -> Unit)?) -> Unit) {
    if (drawer == null) return content(null)
    val p = props(drawer, ctx)
    val direction = LocalLayoutDirection.current
    val flipped = if (direction == LayoutDirection.Ltr) LayoutDirection.Rtl else LayoutDirection.Ltr
    val side = if (p["side"] == "end") flipped else direction
    val permanent = (p["permanentFrom"] as? String)?.let { SIZE_ORDER.indexOf(sizeClass) >= SIZE_ORDER.indexOf(it) } == true
    val sheet: @Composable () -> Unit = { CompositionLocalProvider(LocalLayoutDirection provides direction) { Panel(drawer, ctx) } }
    val body: @Composable (open: (() -> Unit)?) -> Unit = { open -> CompositionLocalProvider(LocalLayoutDirection provides direction) { content(open) } }
    if (permanent) {
        CompositionLocalProvider(LocalLayoutDirection provides side) {
            PermanentNavigationDrawer(drawerContent = { PermanentDrawerSheet { sheet() } }) { body(null) }
        }
        return
    }
    CompositionLocalProvider(LocalLayoutDirection provides side) { ModalDrawer(drawer, ctx, sheet, body) }
}

@Composable
private fun ModalDrawer(drawer: JSONObject, ctx: Ctx, sheet: @Composable () -> Unit, content: @Composable (open: (() -> Unit)?) -> Unit) {
    val state = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val id = drawer.optString("id")
    val wanted = isOpen(drawer, ctx)
    LaunchedEffect(wanted) { if (wanted) state.open() else state.close() }
    // Fermé par l'utilisateur (geste, clic à côté) : l'état suit.
    LaunchedEffect(state) {
        snapshotFlow { state.currentValue }.collect { value ->
            if (value == DrawerValue.Closed && isOpen(drawer, ctx)) closed(drawer, ctx)
        }
    }
    ModalNavigationDrawer(
        drawerState = state,
        drawerContent = { ModalDrawerSheet { sheet() } },
    ) {
        content { scope.launch { OcaraUIEngine.changed(id, "open", true) } }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun BottomSheet(sheet: JSONObject, ctx: Ctx) {
    if (!isOpen(sheet, ctx)) return
    ModalBottomSheet(onDismissRequest = { closed(sheet, ctx) }) { Panel(sheet, ctx) }
}

/** Contenu d'un tiroir ou d'un panneau : ses enfants en colonne. */
@Composable
private fun Panel(node: JSONObject, ctx: Ctx) {
    val p = props(node, ctx)
    val gap = ((p["spacing"] as? Number)?.toFloat() ?: 8f).dp
    Column(Modifier.padding(16.dp), Arrangement.spacedBy(gap)) {
        for (child in children(p)) Node(child, ctx, childModifier(child, ctx))
    }
}
