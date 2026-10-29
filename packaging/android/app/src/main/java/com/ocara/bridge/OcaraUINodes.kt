package com.ocara.bridge

import android.graphics.BitmapFactory
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.TimePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.roundToInt

/** Contexte d'un nœud : élément de liste (`bind`) et parent défilant. */
internal data class Ctx(val tree: JSONObject, val item: JSONObject? = null, val scrolling: Boolean = false, val index: Int? = null)

/** Types qui prennent la largeur d'une colonne (règle commune avec engine.css). */
private val FILL_IN_COLUMN = setOf(
    "column", "row", "stack", "grid", "card", "list", "divider", "progress",
    "input", "select", "slider", "date", "time", "radio", "webview",
)

/** Types de taille fixe dans une ligne (jamais réduits). */
private val FIXED_IN_ROW = setOf("icon", "iconButton", "button", "spacer", "divider", "image")

private val CONTAINERS = setOf("column", "row", "stack", "grid", "card", "list")

/**
 * Écran courant de l'arbre, avec sa barre de titre, le retour système et les
 * composants Material de l'écran (OcaraUIMaterial.kt). La variante de taille
 * suit la largeur disponible (en dp), à la volée (rotation, écran plié).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun OcaraInterface(root: JSONObject) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val size = sizeOf(root, maxWidth.value.toInt())
        LaunchedEffect(size.first) { OcaraUIEngine.reportSize(size.first) }
        val tree = size.second?.let { root.optJSONObject("variants")?.optJSONObject(it) } ?: root
        Screen(tree)
    }
}

/** Classe de la largeur `width` et variante à afficher (la plus grande atteinte). */
private fun sizeOf(root: JSONObject, width: Int): Pair<String, String?> {
    val sizes = root.optJSONObject("sizes") ?: return "" to null
    val variants = root.optJSONObject("variants")
    var cls = ""
    var variant: String? = null
    for (name in sizes.keys().asSequence().sortedBy { sizes.optInt(it) }) {
        if (width >= sizes.optInt(name)) {
            cls = name
            if (variants?.has(name) == true) variant = name
        }
    }
    return cls to variant
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun Screen(tree: JSONObject) {
    val window = tree.optJSONObject("window") ?: JSONObject()
    val dark = when (window.optString("theme", "auto")) {
        "dark" -> true
        "light" -> false
        else -> isSystemInDarkTheme()
    }
    val base = if (dark) darkColorScheme() else lightColorScheme()
    val scheme = parseColor(window.opt("primary"))?.let { base.copy(primary = it) } ?: base
    val bars = window.optJSONObject("android")?.optString("systemBars")?.takeIf { it.isNotEmpty() }
    LaunchedEffect(dark, bars) {
        // Icônes des barres système lisibles sur le fond du thème.
        runCatching { OcaraUI.setSystemBarsStyle(bars ?: if (dark) "dark" else "light") }
    }
    MaterialTheme(colorScheme = scheme) {
        val stack = OcaraUIEngine.backStack
        val screens = tree.getJSONObject("screens")
        // Écran absent de cette variante : celui de départ.
        val current = stack.lastOrNull()?.takeIf { screens.has(it) } ?: tree.getString("start")
        val screen = screens.getJSONObject(current)
        val layout = screen.getJSONObject("layout")
        val ctx = Ctx(tree)
        val material = MaterialNodes(layout)
        // Écran affiché (départ, navigation, retour, nouvel arbre) : `onShow`.
        LaunchedEffect(current, tree) {
            screen.optString("onShow").takeIf { it.isNotEmpty() }?.let { OcaraUIEngine.shown(it, current) }
        }
        BackHandler(enabled = stack.size > 1) { OcaraUIEngine.back() }
        WithDrawer(material.drawer, ctx) { openDrawer ->
            Scaffold(
                topBar = {
                    val title = screen.optString("title", window.optString("title"))
                    if (title.isNotEmpty() || stack.size > 1 || openDrawer != null) {
                        TopAppBar(
                            title = { Text(title) },
                            navigationIcon = {
                                when {
                                    stack.size > 1 -> IconButton(onClick = { OcaraUIEngine.back() }) {
                                        Icon(iconVector(BACK_PATH), contentDescription = "Retour")
                                    }
                                    openDrawer != null -> IconButton(onClick = openDrawer) {
                                        Icon(iconVector(MENU_PATH), contentDescription = "Menu")
                                    }
                                }
                            },
                        )
                    }
                },
                floatingActionButton = { material.fab?.let { Fab(it, ctx) } },
            ) { padding ->
                Box(Modifier.padding(padding).fillMaxSize()) {
                    Node(layout, ctx, Modifier.fillMaxSize())
                }
            }
        }
        for (sheet in material.sheets) BottomSheet(sheet, ctx)
    }
}

private const val BACK_PATH = "M20 11H7.83l5.59-5.59L12 4l-8 8 8 8 1.41-1.41L7.83 13H20v-2z"
private const val MENU_PATH = "M3 18h18v-2H3v2zm0-5h18v-2H3v2zm0-7v2h18V6H3z"

/** Propriétés effectives : style nommé, nœud, surcharge `android`, état, `binds`. */
internal fun props(node: JSONObject, ctx: Ctx): Map<String, Any?> {
    val out = HashMap<String, Any?>()
    fun merge(layer: JSONObject?) {
        layer ?: return
        for (key in layer.keys()) if (key != "tauri" && key != "android") out[key] = layer.get(key)
    }
    node.optString("style").takeIf { it.isNotEmpty() }?.let { merge(ctx.tree.optJSONObject("styles")?.optJSONObject(it)) }
    merge(node)
    merge(node.optJSONObject("android"))
    node.optString("id").takeIf { it.isNotEmpty() && ctx.item == null }?.let { id ->
        out.putAll(OcaraUIEngine.stateOf(id))
    }
    // Élément de liste : `binds` lit les champs de l'élément.
    val binds = node.optJSONObject("binds")
    if (ctx.item != null && binds != null) {
        for (prop in binds.keys()) {
            var value: Any? = ctx.item
            for (part in binds.getString(prop).split(".")) value = (value as? JSONObject)?.opt(part)
            out[prop] = value
        }
    }
    return out.mapValues { OcaraUIEngine.plain(it.value) }
}

private fun Map<String, Any?>.text(key: String) = this[key]?.toString()
private fun Map<String, Any?>.num(key: String) = (this[key] as? Number)?.toDouble()
private fun Map<String, Any?>.flag(key: String, default: Boolean) = this[key] as? Boolean ?: default

internal fun parseColor(value: Any?): Color? {
    val hex = (value as? String)?.removePrefix("#") ?: return null
    val rgba = hex.toLongOrNull(16) ?: return null
    return when (hex.length) {
        6 -> Color(0xFF000000 or rgba)
        8 -> Color(((rgba and 0xFF) shl 24) or (rgba shr 8))
        else -> null
    }
}

private fun bound(p: Map<String, Any?>, ctx: Ctx): String {
    val field = p.text("bind")
    if (field != null && ctx.item != null) {
        var value: Any? = ctx.item
        for (part in field.split(".")) value = (value as? JSONObject)?.opt(part)
        return OcaraUIEngine.plain(value)?.toString() ?: ""
    }
    return p.text("value") ?: ""
}

private fun spacing(value: Any?): PaddingValues? = when (value) {
    is Number -> PaddingValues(value.toFloat().dp)
    is JSONArray -> {
        val v = (0 until value.length()).map { value.optDouble(it).toFloat().dp }
        if (v.size == 2) PaddingValues(horizontal = v[1], vertical = v[0])
        else PaddingValues(start = v[3], top = v[0], end = v[1], bottom = v[2])
    }
    else -> null
}

internal fun iconVector(path: String): ImageVector =
    ImageVector.Builder(defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f)
        .addPath(PathParser().parsePathString(path).toNodes(), fill = SolidColor(Color.Black))
        .build()

/** Taille, marges, fond, bordure et ombre d'un nœud. */
private fun decorate(modifier: Modifier, p: Map<String, Any?>): Modifier {
    var m = modifier
    spacing(p["margin"])?.let { m = m.padding(it) }
    val shape = RoundedCornerShape((p.num("radius") ?: 0.0).toFloat().dp)
    (p["width"] as? Number)?.let { m = m.width(it.toFloat().dp) }
    (p["height"] as? Number)?.let { m = m.height(it.toFloat().dp) }
    p.num("elevation")?.let { m = m.shadow(it.toFloat().dp, shape) }
    parseColor(p["background"])?.let { m = m.background(it, shape) }
    parseColor(p["border"])?.let { m = m.border(BorderStroke(1.dp, it), shape) }
    spacing(p["padding"])?.let { m = m.padding(it) }
    p.text("label")?.let { label -> m = m.semantics { contentDescription = label } }
    return m
}

private fun textStyle(p: Map<String, Any?>, base: TextStyle): TextStyle = base.copy(
    fontSize = p.num("fontSize")?.sp ?: base.fontSize,
    fontWeight = if (p["bold"] == true) FontWeight.Bold else base.fontWeight,
    fontStyle = if (p["italic"] == true) FontStyle.Italic else base.fontStyle,
    color = parseColor(p["color"]) ?: base.color,
)

/** Modificateur posé par une colonne sur un enfant : `fill`, `weight`. */
@Composable
internal fun ColumnScope.childModifier(child: JSONObject, ctx: Ctx): Modifier {
    val p = props(child, ctx)
    var m: Modifier = Modifier
    val type = child.optString("type")
    if (p["width"] == "fill" || type in FILL_IN_COLUMN) m = m.fillMaxWidth()
    if (p["align"] != null && type !in FILL_IN_COLUMN) m = m.align(horizontal(p.text("align")))
    if (!ctx.scrolling) {
        val weight = p.num("weight") ?: if (p["height"] == "fill" || type == "spacer") 1.0 else null
        weight?.let { m = m.weight(it.toFloat()) }
    }
    return m
}

/** Modificateur posé par une ligne sur un enfant. */
@Composable
private fun RowScope.childModifier(child: JSONObject, ctx: Ctx): Modifier {
    val p = props(child, ctx)
    var m: Modifier = Modifier
    val type = child.optString("type")
    val weight = p.num("weight") ?: if (p["width"] == "fill" || type == "spacer") 1.0 else null
    when {
        weight != null -> m = m.weight(weight.toFloat())
        // Taille naturelle, mais réduite si la ligne manque de place (comme
        // `flex-shrink` côté desktop) : sinon le premier enfant prend tout.
        p["width"] == null && type !in FIXED_IN_ROW -> m = m.weight(1f, fill = false)
    }
    if (p["height"] == "fill") m = m.fillMaxHeight()
    return m
}

private fun horizontal(align: String?) = when (align) {
    "center" -> Alignment.CenterHorizontally
    "end" -> Alignment.End
    else -> Alignment.Start
}

/** Clic ; dans un élément de liste, l'action porte l'élément et son rang. */
private fun itemClick(p: Map<String, Any?>, id: String?, ctx: Ctx) {
    if (ctx.item != null) OcaraUIEngine.fire(p, "onClick", id, ctx.item, ctx.index)
    else OcaraUIEngine.fire(p, "onClick", id)
}

internal fun children(p: Map<String, Any?>): List<JSONObject> {
    val array = p["children"] as? JSONArray ?: return emptyList()
    return (0 until array.length()).mapNotNull { array.optJSONObject(it) }
}

@Composable
internal fun Node(node: JSONObject, ctx: Ctx, modifier: Modifier) {
    val p = props(node, ctx)
    if (p["visible"] == false) return
    val id = node.optString("id").takeIf { it.isNotEmpty() }
    val enabled = p.flag("enabled", true)
    val m = decorate(modifier, p)
    val type = node.getString("type")
    if (type in CONTAINERS) return Container(node, type, p, id, ctx, m)
    when (type) {
        "divider" -> HorizontalDivider(m)
        "spacer" -> Spacer(m)
        "text" -> Text(bound(p, ctx), m, style = textStyle(p, bodyStyle()))
        "heading" -> {
            val base = when ((p.num("level") ?: 2.0).toInt()) {
                1 -> MaterialTheme.typography.headlineMedium
                3 -> MaterialTheme.typography.titleMedium
                else -> MaterialTheme.typography.titleLarge
            }
            Text(bound(p, ctx), m, style = textStyle(p, base))
        }
        "image" -> Picture(if (p.containsKey("bind")) bound(p, ctx) else p.text("src") ?: "", p, m)
        "icon" -> NamedIcon(ctx, p.text("name"), p.text("label"), m, parseColor(p["color"]))
        "progress" -> {
            val value = p.num("value")
            if (value == null) LinearProgressIndicator(m)
            else LinearProgressIndicator(progress = { value.toFloat() }, modifier = m)
        }
        "input" -> Input(type, p, id, ctx, enabled, m)
        "date", "time" -> DateTimeField(type, p, id, enabled, m)
        "checkbox", "switch" -> {
            val checked = if (p.containsKey("bind")) bound(p, ctx) == "true" else p.flag("checked", false)
            val toggle = { value: Boolean ->
                OcaraUIEngine.changed(id, "checked", value)
                OcaraUIEngine.fire(p, "onChange", id, value)
            }
            Row(m.toggleable(checked, enabled) { toggle(it) }, verticalAlignment = Alignment.CenterVertically) {
                if (type == "switch") Switch(checked, null, enabled = enabled) else Checkbox(checked, null, enabled = enabled)
                p.text("label")?.let { Text(it, Modifier.padding(start = 8.dp)) }
            }
        }
        "radio" -> Column(m) {
            for (option in options(p)) {
                val selected = option.first == p.text("value")
                Row(
                    Modifier.selectable(selected, enabled) { pick(p, id, option.first) }.padding(vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    RadioButton(selected, null, enabled = enabled)
                    Text(option.second, Modifier.padding(start = 8.dp))
                }
            }
        }
        "select" -> Select(p, id, enabled, m)
        "slider" -> {
            val min = (p.num("min") ?: 0.0).toFloat()
            val max = (p.num("max") ?: 100.0).toFloat()
            val step = p.num("step") ?: 0.0
            val steps = if (step > 0) ((max - min) / step).roundToInt() - 1 else 0
            Slider(
                value = (p.num("value") ?: min.toDouble()).toFloat(),
                onValueChange = { value ->
                    val number: Any = if (step > 0 && step % 1.0 == 0.0) value.roundToInt() else value.toDouble()
                    OcaraUIEngine.changed(id, "value", number)
                    OcaraUIEngine.fire(p, "onChange", id, number)
                },
                modifier = m,
                enabled = enabled,
                valueRange = min..max,
                steps = steps.coerceAtLeast(0),
            )
        }
        "button" -> {
            val click = { itemClick(p, id, ctx) }
            val content: @Composable RowScope.() -> Unit = {
                p.text("icon")?.let {
                    NamedIcon(ctx, it, null, Modifier.size(18.dp), null)
                    Spacer(Modifier.width(8.dp))
                }
                Text(p.text("text") ?: "")
            }
            when (p.text("variant")) {
                "secondary" -> OutlinedButton(click, m, enabled, content = content)
                "text" -> TextButton(click, m, enabled, content = content)
                else -> Button(click, m, enabled, content = content)
            }
        }
        "iconButton" -> IconButton({ itemClick(p, id, ctx) }, m, enabled) {
            NamedIcon(ctx, p.text("icon"), p.text("label"), Modifier, null)
        }
        // Placés par l'écran (OcaraUIMaterial.kt), pas à leur rang.
        "android.fab", "android.bottomSheet", "android.drawer" -> {}
        "webview" -> OcaraWebNode(id, p, if (p["height"] == null) m.height(240.dp) else m)
    }
}

@Composable
private fun bodyStyle(): TextStyle = MaterialTheme.typography.bodyLarge

@Composable
private fun Container(node: JSONObject, type: String, p: Map<String, Any?>, id: String?, ctx: Ctx, modifier: Modifier) {
    val gap = (p.num("spacing") ?: 0.0).toFloat().dp
    val scroll = p["scroll"] == true
    val m = if (scroll) modifier.verticalScroll(rememberScrollState()) else modifier
    val inner = ctx.copy(scrolling = scroll)
    when (type) {
        "row" -> Row(m, Arrangement.spacedBy(gap), verticalAlignment = Alignment.CenterVertically) {
            for (child in children(p)) Node(child, inner, childModifier(child, inner))
        }
        "stack" -> Box(m) {
            for (child in children(p)) {
                val align = when (props(child, inner).text("align")) {
                    "center" -> Alignment.Center
                    "end" -> Alignment.BottomEnd
                    else -> Alignment.TopStart
                }
                Node(child, inner, Modifier.align(align).then(if (child.optString("type") in FILL_IN_COLUMN) Modifier.fillMaxWidth() else Modifier))
            }
        }
        "grid" -> Column(m, Arrangement.spacedBy(gap)) {
            val columns = (p.num("columns") ?: 2.0).toInt()
            for (line in children(p).chunked(columns)) {
                Row(Modifier.fillMaxWidth(), Arrangement.spacedBy(gap)) {
                    for (child in line) Node(child, inner, Modifier.weight(1f))
                    repeat(columns - line.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
        "card" -> {
            val click = p["onClick"]?.let { { itemClick(p, id, ctx) } }
            val body: @Composable ColumnScope.() -> Unit = {
                Column(Modifier.padding(12.dp), Arrangement.spacedBy(gap)) {
                    for (child in children(p)) Node(child, inner, childModifier(child, inner))
                }
            }
            // Couleur de fond de la carte elle-même (sa surface couvrirait `background`).
            val colors = parseColor(p["background"])?.let { CardDefaults.cardColors(containerColor = it) } ?: CardDefaults.cardColors()
            if (click != null) Card(onClick = click, modifier = m, colors = colors, content = body)
            else Card(m, colors = colors, content = body)
        }
        "list" -> Column(m, Arrangement.spacedBy(gap)) {
            val template = node.getJSONObject("item")
            OcaraUIEngine.items(p["items"]).forEachIndexed { index, row ->
                val select = Modifier.clickable(enabled = p["onSelect"] != null) {
                    OcaraUIEngine.fire(p, "onSelect", id, row, index)
                }
                Node(template, inner.copy(item = row, index = index), select.fillMaxWidth().padding(8.dp))
            }
        }
        else -> Column(m, Arrangement.spacedBy(gap), horizontalAlignment = horizontal(p.text("align"))) {
            for (child in children(p)) Node(child, inner, childModifier(child, inner))
        }
    }
}

@Composable
internal fun NamedIcon(ctx: Ctx, name: String?, label: String?, modifier: Modifier, tint: Color?) {
    val path = ctx.tree.optJSONObject("icons")?.optString(name ?: "")?.takeIf { it.isNotEmpty() } ?: return
    val vector = remember(path) { iconVector(path) }
    Icon(vector, label, modifier, tint ?: LocalContentColor.current)
}

@Composable
private fun Picture(src: String, p: Map<String, Any?>, modifier: Modifier) {
    // Image embarquée par le compilateur (`data:…;base64,`) ou fichier du programme.
    val bitmap = remember(src) {
        if (src.startsWith("data:")) {
            val bytes = android.util.Base64.decode(src.substringAfter(","), android.util.Base64.DEFAULT)
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
        } else {
            BitmapFactory.decodeFile(OcaraBridge.file(src).path)?.asImageBitmap()
        }
    }
    val scale = when (p.text("fit")) {
        "cover" -> ContentScale.Crop
        "fill" -> ContentScale.FillBounds
        "none" -> ContentScale.None
        else -> ContentScale.Fit
    }
    if (bitmap != null) Image(bitmap, p.text("label"), modifier, contentScale = scale)
    else Box(modifier.size(48.dp).background(MaterialTheme.colorScheme.surfaceVariant))
}

@Composable
private fun Input(type: String, p: Map<String, Any?>, id: String?, ctx: Ctx, enabled: Boolean, modifier: Modifier) {
    val kind = p.text("kind") ?: "text"
    val multiline = kind == "multiline"
    var local by remember { mutableStateOf(if (p.containsKey("bind")) bound(p, ctx) else p.text("value") ?: "") }
    val value = if (id != null) p.text("value") ?: "" else local
    val keyboard = when {
        kind == "password" -> KeyboardType.Password
        kind == "email" -> KeyboardType.Email
        kind == "number" -> KeyboardType.Number
        else -> KeyboardType.Text
    }
    val placeholder = p.text("placeholder")
    OutlinedTextField(
        value = value,
        onValueChange = { text ->
            local = text
            OcaraUIEngine.changed(id, "value", text)
            OcaraUIEngine.fire(p, "onChange", id, text)
        },
        modifier = modifier,
        enabled = enabled,
        readOnly = p["readonly"] == true,
        placeholder = placeholder?.let { { Text(it) } },
        singleLine = !multiline,
        minLines = if (multiline) 3 else 1,
        visualTransformation = if (kind == "password") PasswordVisualTransformation() else VisualTransformation.None,
        keyboardOptions = KeyboardOptions(keyboardType = keyboard, imeAction = if (multiline) ImeAction.Default else ImeAction.Done),
        keyboardActions = KeyboardActions(onDone = { OcaraUIEngine.fire(p, "onSubmit", id, value) }),
    )
}

/** Date (`AAAA-MM-JJ`) ou heure (`HH:MM`) choisie dans la boîte native de Material 3. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun DateTimeField(type: String, p: Map<String, Any?>, id: String?, enabled: Boolean, modifier: Modifier) {
    var open by remember { mutableStateOf(false) }
    val value = p.text("value") ?: ""
    val choose = { text: String ->
        open = false
        pick(p, id, text)
    }
    Box(modifier) {
        OutlinedTextField(
            value = value,
            onValueChange = {},
            modifier = Modifier.fillMaxWidth(),
            enabled = enabled,
            readOnly = true,
            placeholder = { Text(p.text("placeholder") ?: if (type == "date") "AAAA-MM-JJ" else "HH:MM") },
            singleLine = true,
        )
        // Le champ est en lecture seule : un appui ouvre la boîte de choix.
        Box(Modifier.matchParentSize().clickable(enabled = enabled && p["readonly"] != true) { open = true })
    }
    if (!open) return
    val dismiss = { open = false }
    if (type == "date") {
        val utc = java.util.TimeZone.getTimeZone("UTC")
        val format = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.ROOT).apply { timeZone = utc }
        val initial = runCatching { format.parse(value)?.time }.getOrNull()
        val state = rememberDatePickerState(initialSelectedDateMillis = initial)
        DatePickerDialog(
            onDismissRequest = dismiss,
            confirmButton = {
                TextButton({ state.selectedDateMillis?.let { choose(format.format(java.util.Date(it))) } ?: dismiss() }) {
                    Text(stringResource(android.R.string.ok))
                }
            },
            dismissButton = { TextButton(dismiss) { Text(stringResource(android.R.string.cancel)) } },
        ) { DatePicker(state) }
    } else {
        val parts = value.split(":").mapNotNull { it.toIntOrNull() }
        val state = rememberTimePickerState(parts.getOrElse(0) { 12 }, parts.getOrElse(1) { 0 }, is24Hour = true)
        AlertDialog(
            onDismissRequest = dismiss,
            confirmButton = {
                TextButton({ choose("%02d:%02d".format(state.hour, state.minute)) }) { Text(stringResource(android.R.string.ok)) }
            },
            dismissButton = { TextButton(dismiss) { Text(stringResource(android.R.string.cancel)) } },
            text = { TimePicker(state) },
        )
    }
}

private fun options(p: Map<String, Any?>): List<Pair<String, String>> {
    val array = p["options"] as? JSONArray ?: return emptyList()
    return (0 until array.length()).mapNotNull { array.optJSONObject(it) }
        .map { it.optString("value") to it.optString("text") }
}

private fun pick(p: Map<String, Any?>, id: String?, value: String) {
    OcaraUIEngine.changed(id, "value", value)
    OcaraUIEngine.fire(p, "onChange", id, value)
}

@Composable
private fun Select(p: Map<String, Any?>, id: String?, enabled: Boolean, modifier: Modifier) {
    var open by remember { mutableStateOf(false) }
    val choices = options(p)
    val current = choices.firstOrNull { it.first == p.text("value") }?.second ?: ""
    Box(modifier) {
        OutlinedButton({ open = true }, Modifier.fillMaxWidth(), enabled, shape = RoundedCornerShape(8.dp)) {
            Text(current, Modifier.weight(1f))
            Text("▾")
        }
        DropdownMenu(open, { open = false }) {
            for (choice in choices) {
                DropdownMenuItem(text = { Text(choice.second) }, onClick = {
                    open = false
                    pick(p, id, choice.first)
                })
            }
        }
    }
}
