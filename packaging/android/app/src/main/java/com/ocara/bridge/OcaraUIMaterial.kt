package com.ocara.bridge

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import org.json.JSONObject

/**
 * Composants Material propres à Android d'un écran (`.oc.ui`, préfixe
 * `android.`) : bouton flottant (`android.fab`), tiroir de navigation
 * (`android.drawer`) et panneaux inférieurs (`android.bottomSheet`). Ils
 * sont déclarés n'importe où dans la disposition de l'écran et placés là où
 * Material les attend ; le premier bouton flottant et le premier tiroir
 * comptent. Tiroir et panneau s'ouvrent par `ui.set(id, "open", true)` (le
 * tiroir aussi par l'icône de menu de la barre de titre).
 */
internal class MaterialNodes(layout: JSONObject) {
    var fab: JSONObject? = null
    var drawer: JSONObject? = null
    val sheets = ArrayList<JSONObject>()

    init {
        collect(layout)
    }

    private fun collect(node: JSONObject) {
        when (node.optString("type")) {
            "android.fab" -> if (fab == null) fab = node
            "android.drawer" -> if (drawer == null) drawer = node
            "android.bottomSheet" -> sheets.add(node)
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

/**
 * Écran entouré du tiroir `drawer` s'il y en a un ; `content` reçoit l'action
 * d'ouverture (icône de menu), `null` sans tiroir.
 */
@Composable
internal fun WithDrawer(drawer: JSONObject?, ctx: Ctx, content: @Composable (open: (() -> Unit)?) -> Unit) {
    if (drawer == null) return content(null)
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
        drawerContent = { ModalDrawerSheet { Panel(drawer, ctx) } },
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
