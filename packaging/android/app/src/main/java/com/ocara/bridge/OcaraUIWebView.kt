package com.ocara.bridge

import android.annotation.SuppressLint
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.webkit.JavascriptInterface
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.compose.BackHandler
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener
import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit

/**
 * Composant `webview` d'ocara.UI : une vraie `WebView` par nœud. Page →
 * Ocara : `window.ocara.post(data)` (action `onMessage`) ; Ocara → page :
 * `ui.postMessage(id, data)` (`window.ocara.onMessage(cb)`), `ui.eval(id, script)`.
 * Le bouton retour remonte d'abord l'historique de la page.
 */
object OcaraUIWebViews {
    /** Pont de la page, injecté au début de chaque chargement. */
    const val SHIM = """(function () {
  if (window.ocara && window.ocara.__native) return;
  window.ocara = window.ocara || {};
  var listeners = [];
  window.ocara.__native = true;
  window.ocara.post = function (data) { ocaraNative.post(JSON.stringify(data === undefined ? null : data)); };
  window.ocara.onMessage = function (callback) { listeners.push(callback); };
  window.ocara.__receive = function (data) {
    listeners.slice().forEach(function (callback) { try { callback(data); } catch (e) { console.error(e); } });
  };
})();"""

    /** Vues par `id` (une interface affichée à la fois), thread principal. */
    val views = HashMap<String, WebView>()
    private val main = Handler(Looper.getMainLooper())

    private fun onView(id: String, action: (WebView) -> Unit) {
        main.post { views[id]?.let(action) }
    }

    /** `ui.postMessage(id, data)` ; `dataJson` : la donnée en JSON. */
    @JvmStatic
    fun postMessage(id: String, dataJson: String) = onView(id) {
        it.evaluateJavascript("window.ocara && window.ocara.__receive($dataJson)", null)
    }

    /** `ui.eval(id, script)` → texte du résultat (JSON pour un objet), attendu. */
    @JvmStatic
    fun eval(id: String, script: String): String {
        val result = CompletableFuture<String>()
        val wrapped = "(function () { var r = eval(${JSONObject.quote(script)}); return typeof r === 'string' ? r : JSON.stringify(r); })()"
        main.post {
            val view = views[id]
            if (view == null) result.complete("") else view.evaluateJavascript(wrapped) { json ->
                // Le résultat arrive encodé en JSON (une chaîne entre guillemets).
                val value = runCatching { JSONTokener(json).nextValue() }.getOrNull()
                result.complete(if (value == null || value == JSONObject.NULL) "" else value.toString())
            }
        }
        return runCatching { result.get(10, TimeUnit.SECONDS) }.getOrDefault("")
    }

    @JvmStatic
    fun goBack(id: String) = onView(id) { if (it.canGoBack()) it.goBack() }

    @JvmStatic
    fun goForward(id: String) = onView(id) { if (it.canGoForward()) it.goForward() }

    @JvmStatic
    fun reload(id: String) = onView(id) { it.reload() }

    /** Contenu du nœud : `html`, `file` (relatif au dossier du programme) ou `url`. */
    fun load(view: WebView, props: Map<String, Any?>) {
        val html = props["html"] as? String
        val file = props["file"] as? String
        val url = props["url"] as? String
        when {
            html != null -> view.loadDataWithBaseURL(null, "<script>$SHIM</script>$html", "text/html", "utf-8", null)
            file != null -> view.loadUrl("file://" + OcaraBridge.file(file).absolutePath)
            url != null -> view.loadUrl(url)
        }
    }

    /** Hôte autorisé par `allow` (liste absente : tous). */
    fun allowed(props: Map<String, Any?>, uri: Uri): Boolean {
        val allow = props["allow"] as? JSONArray ?: return true
        val host = uri.host ?: return true
        return (0 until allow.length()).map { allow.optString(it) }.any { host == it || host.endsWith(".$it") }
    }
}

/** Messages `window.ocara.post` de la page (thread du pont JavaScript). */
private class OcaraWebBridge(private val id: String?, private val props: () -> Map<String, Any?>) {
    @JavascriptInterface
    fun post(data: String) {
        val value = runCatching { JSONTokener(data).nextValue() }.getOrDefault(data)
        OcaraUIEngine.fire(props(), "onMessage", id, value)
    }
}

private class OcaraWebClient(
    private val id: String?,
    private val props: () -> Map<String, Any?>,
    private val onFinished: (String, Boolean) -> Unit,
) : WebViewClient() {
    override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
        val uri = request.url
        val web = uri.scheme == "http" || uri.scheme == "https"
        if (!web || OcaraUIWebViews.allowed(props(), uri)) return false
        // Lien hors de `allow` : navigateur du système.
        runCatching { view.context.startActivity(Intent(Intent.ACTION_VIEW, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
        return true
    }

    override fun onPageStarted(view: WebView, url: String?, favicon: Bitmap?) {
        view.evaluateJavascript(OcaraUIWebViews.SHIM, null)
    }

    override fun onPageFinished(view: WebView, url: String?) {
        view.evaluateJavascript(OcaraUIWebViews.SHIM, null)
        onFinished(url ?: "", view.canGoBack())
        OcaraUIEngine.changed(id, "url", url ?: "")
        OcaraUIEngine.changed(id, "canGoBack", view.canGoBack())
        OcaraUIEngine.fire(props(), "onLoad", id, url ?: "")
    }

    override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
        if (!request.isForMainFrame) return
        val value = JSONObject().put("url", request.url.toString()).put("message", error.description.toString())
        OcaraUIEngine.fire(props(), "onError", id, value)
    }
}

/** Nœud `webview` : la vue, rechargée quand sa source change. */
@SuppressLint("SetJavaScriptEnabled", "JavascriptInterface")
@Composable
fun OcaraWebNode(id: String?, props: Map<String, Any?>, modifier: Modifier) {
    var canGoBack by remember { mutableStateOf(false) }
    val current = remember { arrayOf(props) }
    current[0] = props
    val source = listOf(props["url"], props["file"], props["html"])
    var loaded by remember { mutableStateOf<List<Any?>?>(null) }
    var webView by remember { mutableStateOf<WebView?>(null) }
    BackHandler(enabled = canGoBack) { webView?.goBack() }
    AndroidView(
        factory = { context ->
            WebView(context).apply {
                settings.javaScriptEnabled = props["javascript"] != false
                settings.domStorageEnabled = props["storage"] != false
                (props["userAgent"] as? String)?.let { settings.userAgentString = it }
                addJavascriptInterface(OcaraWebBridge(id) { current[0] }, "ocaraNative")
                webViewClient = OcaraWebClient(id, { current[0] }) { url, back ->
                    canGoBack = back
                    // L'état `url` devient la page courante : pas une
                    // nouvelle source à charger.
                    loaded = listOf(url, current[0]["file"], current[0]["html"])
                }
                if (id != null) OcaraUIWebViews.views[id] = this
                webView = this
            }
        },
        update = { view ->
            (props["zoom"] as? Number)?.let { view.settings.textZoom = (it.toDouble() * 100).toInt() }
            if (loaded != source) {
                loaded = source
                OcaraUIWebViews.load(view, props)
            }
        },
        onRelease = { view -> if (id != null && OcaraUIWebViews.views[id] === view) OcaraUIWebViews.views.remove(id) },
        modifier = modifier,
    )
}
