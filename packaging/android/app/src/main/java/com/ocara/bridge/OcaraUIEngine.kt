package com.ocara.bridge

import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

/**
 * Moteur de rendu Android d'`ocara.UI` : l'arbre normalisé d'un `.oc.ui`
 * (src/core/ui_schema.d, docs/oc-ui.md) affiché en Compose par
 * [OcaraInterface] (OcaraUINodes.kt). Même contrat que le moteur desktop
 * (runtime_tauri/src/ui/engine.js) :
 *
 * - [show], [set], [navigate] : appelés depuis le thread Ocara, appliqués sur
 *   le thread principal ;
 * - événements remis à Ocara par [OcaraBridge.nativeEvent] (`"__ui"`, JSON) :
 *   `{ id, prop, value }` pour une valeur changée par l'utilisateur,
 *   `{ action, id, value, index }` pour une action déclarée (`onClick`…).
 */
object OcaraUIEngine {
    private const val TAG = "ocara.UI"
    private val main = Handler(Looper.getMainLooper())

    /** Arbre affiché ; `null` : l'app garde son contenu (WebView). */
    val tree = mutableStateOf<JSONObject?>(null)

    /** État des nœuds, clé [key] : valeurs posées par [set] ou par l'utilisateur. */
    val state = mutableStateMapOf<String, Any?>()

    /** Demandes de focus par `id` (compteur) : premier champ obligatoire vide (runtime_ui/src/form.rs). */
    val focusRequests = mutableStateMapOf<String, Int>()

    /** Écrans ouverts, le courant en dernier. */
    val backStack = mutableStateListOf<String>()

    fun key(id: String, prop: String) = "$id\u0000$prop"

    /** Valeur d'état d'un nœud (`JSONObject.NULL` pour un `null` posé). */
    fun stateOf(id: String): Map<String, Any?> {
        val prefix = "$id\u0000"
        return state.filterKeys { it.startsWith(prefix) }.mapKeys { it.key.removePrefix(prefix) }
    }

    private fun onMain(action: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) action() else main.post(action)
    }

    /**
     * Affiche l'arbre `treeJson` (écran `start`) à la place du contenu de
     * l'app ; `stateJson` : état posé avant (`{ id: { prop: valeur } }`).
     */
    @JvmStatic
    fun show(treeJson: String, stateJson: String) {
        val parsed = JSONObject(treeJson)
        val saved = JSONObject(stateJson)
        onMain {
            state.clear()
            for (id in saved.keys()) {
                val props = saved.getJSONObject(id)
                for (prop in props.keys()) state[key(id, prop)] = props.get(prop)
            }
            backStack.clear()
            backStack.add(parsed.getString("start"))
            tree.value = parsed
            applyWindowOptions(parsed.optJSONObject("window")?.optJSONObject("android"))
            send(JSONObject().put("action", "ready").put("id", JSONObject.NULL).put("value", JSONObject.NULL))
        }
    }

    /**
     * Nouvel arbre (variables `${nom}`, rechargement) : état et écrans
     * ouverts gardés.
     */
    @JvmStatic
    fun reload(treeJson: String) {
        val parsed = JSONObject(treeJson)
        onMain { tree.value = parsed }
    }

    private var sizeClass = ""

    /** Classe de taille courante, rapportée à Ocara (action `resize`) quand elle change. */
    fun reportSize(cls: String) {
        if (cls == sizeClass) return
        sizeClass = cls
        send(JSONObject().put("action", "resize").put("id", JSONObject.NULL).put("value", cls))
    }

    /** `window.android` : options de l'Activity, par `ocara.AndroidUI`. */
    private fun applyWindowOptions(options: JSONObject?) {
        if (options == null) return
        try {
            options.optString("orientation").takeIf { it.isNotEmpty() }?.let { OcaraUI.setOrientation(it) }
            options.optString("systemBars").takeIf { it.isNotEmpty() }?.let { OcaraUI.setSystemBarsStyle(it) }
            if (options.has("fullscreen")) OcaraUI.setFullscreen(if (options.getBoolean("fullscreen")) 1 else 0)
            if (options.has("keepScreenOn")) OcaraUI.setKeepScreenOn(if (options.getBoolean("keepScreenOn")) 1 else 0)
        } catch (e: Exception) {
            Log.w(TAG, "options de fenêtre : ${e.message}")
        }
    }

    /** État `prop` du nœud `id` ; `valueJson` : la valeur en JSON. */
    @JvmStatic
    fun set(id: String, prop: String, valueJson: String) {
        if (prop == "focus") return onMain { focusRequests[id] = (focusRequests[id] ?: 0) + 1 }
        val value = JSONTokener(valueJson).nextValue()
        onMain { state[key(id, prop)] = value }
    }

    /**
     * Écran déjà ouvert plus tôt : on y revient (pile raccourcie). L'écran
     * affiché lui-même : son `onShow` est relancé (rechargement).
     */
    @JvmStatic
    fun navigate(screen: String) = onMain {
        val screens = tree.value?.optJSONObject("screens") ?: return@onMain
        if (!screens.has(screen)) return@onMain
        if (backStack.lastOrNull() == screen) {
            screens.getJSONObject(screen).optString("onShow").takeIf { it.isNotEmpty() }?.let { shown(it, screen) }
            return@onMain
        }
        val at = backStack.indexOf(screen)
        if (at >= 0) {
            while (backStack.size > at + 1) backStack.removeAt(backStack.size - 1)
        } else {
            backStack.add(screen)
        }
    }

    /** Action `onShow` de l'écran affiché (valeur : son nom). */
    fun shown(action: String, screen: String) {
        send(JSONObject().put("action", action).put("id", JSONObject.NULL).put("value", screen))
    }

    /** `ui.back()` depuis Ocara. */
    @JvmStatic
    fun goBack() = onMain { back() }

    /** Écran précédent ; sur l'écran de départ, l'action `back` va à Ocara. */
    fun back() {
        if (backStack.size > 1) backStack.removeAt(backStack.size - 1)
        else send(JSONObject().put("action", "back").put("id", JSONObject.NULL).put("value", JSONObject.NULL))
    }

    /** Valeur changée par l'utilisateur : gardée et remise à Ocara. */
    fun changed(id: String?, prop: String, value: Any?) {
        if (id == null) return
        state[key(id, prop)] = value
        send(JSONObject().put("id", id).put("prop", prop).put("value", value ?: JSONObject.NULL))
    }

    /** Action déclarée par `event` (`onClick`…) dans les propriétés `props`. */
    fun fire(props: Map<String, Any?>, event: String, id: String?, value: Any? = null, index: Int? = null) {
        val action = props[event] as? String ?: return
        if (action == "back") return back()
        val message = JSONObject().put("action", action).put("id", id ?: JSONObject.NULL).put("value", value ?: JSONObject.NULL)
        if (index != null) message.put("index", index)
        send(message)
    }

    private fun send(message: JSONObject) {
        Log.d(TAG, message.toString())
        try {
            OcaraBridge.nativeEvent("__ui", message.toString())
        } catch (e: UnsatisfiedLinkError) {
            Log.w(TAG, "pont Ocara absent : ${e.message}")
        }
    }

    /** Valeur JSON en valeur Kotlin pour Compose (`JSONObject.NULL` → `null`). */
    fun plain(value: Any?): Any? = if (value == JSONObject.NULL) null else value

    fun items(value: Any?): List<JSONObject> {
        val array = plain(value) as? JSONArray ?: return emptyList()
        return (0 until array.length()).mapNotNull { array.optJSONObject(it) }
    }
}
