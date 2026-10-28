package com.ocara.demo

import android.annotation.SuppressLint
import android.content.pm.ApplicationInfo
import android.graphics.Color
import android.os.Bundle
import android.util.Log
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.statusBars
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import com.ocara.bridge.OcaraBridge
import com.ocara.bridge.OcaraInterface
import com.ocara.bridge.OcaraPermissions
import com.ocara.bridge.OcaraUI
import com.ocara.bridge.OcaraUIEngine
import com.ocara.demo.theme.OcaraUIHybridDemoTheme
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

// URL du serveur HTTP Ocara embarqué (voir configs/Server.oc de
// examples/advanced/mini_project — même port). Fixée en dur pour ce squelette
// minimal : aucun besoin connu de la rendre configurable pour l'instant.
private const val OCARA_SERVER_URL = "http://127.0.0.1:8081/"

class MainActivity : ComponentActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    // Fichiers statiques (ex: public/style.css de mini_project) : DOIVENT être
    // placés à la main dans app/src/main/assets/ avant de construire l'APK
    // (voir packaging/android/README.md) — Gradle ne sait pas automatiquement
    // qu'un projet Ocara a un dossier "public/" à embarquer. Un asset Android
    // n'est PAS un chemin de système de fichiers ordinaire (accessible
    // seulement via AssetManager, pas std::fs côté Ocara) : on le copie donc
    // ici, une fois par lancement, vers le répertoire que `nativeStartServer`
    // fait devenir le répertoire de travail du programme Ocara (voir sa doc).
    copyAssetsToFilesDir()

    // Fire-and-forget : voir la doc de OcaraBridge.nativeStartServer — ne
    // bloque jamais ce thread (le thread principal/UI). `filesDir` (PAS un
    // chemin construit à la main) : voir la doc du paramètre `dataDir`.
    // Contexte et Activity pour les builtins `ocara.Android*` (com.ocara.bridge.Ocara*).
    OcaraBridge.attach(this)
    OcaraUI.install(this)
    OcaraBridge.nativeStartServer(filesDir.absolutePath)
    showPreview()

    // Barre de statut transparente (le contenu edge-to-edge de la WebView —
    // la nav sticky en haut de public/style.css — passe déjà dessous) MAIS
    // avec des icônes CLAIRES forcées (`SystemBarStyle.dark`, qui décrit le
    // contenu SOUS la barre — donc "dark background → icônes claires" — pas
    // la couleur de la barre elle-même) : sans ça, `enableEdgeToEdge()` sans
    // argument choisit clair/sombre selon le thème SYSTÈME, qui peut rendre
    // des icônes sombres illisibles sur l'entête toujours sombre de l'app
    // (`--brand-dark` dans style.css), quel que soit le thème du téléphone.
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
    )
    setContent {
      OcaraUIHybridDemoTheme {
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
          // Interface d'un `.oc.ui` affichée par `ocara.UI` (moteur Compose),
          // sinon la page du serveur HTTP Ocara.
          val tree = OcaraUIEngine.tree.value
          if (tree != null) OcaraInterface(tree) else OcaraWebViewScreen()
        }
      }
    }
  }

  override fun onDestroy() {
    OcaraUI.release()
    OcaraBridge.detach(this)
    super.onDestroy()
  }

  /**
   * Aperçu d'une interface, en build debug seulement : arbre JSON produit par
   * `ocara x.oc.ui --dump`, poussé dans le dossier externe de l'app
   * (docs/oc-ui.md, « Aperçu ») :
   * `adb shell am start -n com.ocara.demo/.MainActivity --es ocara.ui.preview x.json`.
   * Le fichier peut aussi être `{ "tree": …, "screen": "…", "set": [[id, prop, valeur]…] }`
   * pour ouvrir un écran avec un état.
   */
  private fun showPreview() {
    val debuggable = (applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE) != 0
    val name = intent?.getStringExtra("ocara.ui.preview") ?: return
    if (!debuggable) return
    val file = File(getExternalFilesDir(null), name)
    try {
      val json = JSONObject(file.readText())
      val tree = json.optJSONObject("tree") ?: json
      OcaraUIEngine.show(tree.toString(), "{}")
      json.optString("screen").takeIf { it.isNotEmpty() }?.let { OcaraUIEngine.navigate(it) }
      val updates = json.optJSONArray("set") ?: JSONArray()
      for (i in 0 until updates.length()) {
        val update = updates.getJSONArray(i)
        val value = update.get(2)
        OcaraUIEngine.set(update.getString(0), update.getString(1), if (value is String) JSONObject.quote(value) else value.toString())
      }
    } catch (e: Exception) {
      Log.e("ocara.UI", "aperçu impossible ($file) : ${e.message}")
    }
  }

  /** Réponse à `AndroidPermissions::request`, attendue par le programme Ocara. */
  @Deprecated("Remplacée par les ActivityResult API, mais seule voie pour requestPermissions appelé hors de l'UI")
  override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<String>, grantResults: IntArray) {
    @Suppress("DEPRECATION")
    super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    OcaraPermissions.onResult(requestCode, grantResults)
  }

  /**
   * Copie récursivement `assets/` vers `filesDir/` — pas de mise en cache
   * "déjà fait" : ces fichiers sont petits (CSS d'un exemple), les recopier à
   * chaque lancement évite toute confusion de cache pendant le développement
   * (un `assets/public/style.css` mis à jour est repris au lancement suivant,
   * pas seulement à la désinstallation).
   */
  private fun copyAssetsToFilesDir(assetPath: String = "") {
    val entries = assets.list(assetPath) ?: return
    if (entries.isEmpty()) {
      // Fichier (pas un dossier) : list() sur un fichier renvoie un tableau vide.
      if (assetPath.isEmpty()) return
      val dest = File(filesDir, assetPath)
      dest.parentFile?.mkdirs()
      assets.open(assetPath).use { input -> dest.outputStream().use { input.copyTo(it) } }
      return
    }
    for (entry in entries) {
      val childPath = if (assetPath.isEmpty()) entry else "$assetPath/$entry"
      copyAssetsToFilesDir(childPath)
    }
  }
}

/**
 * Attend que le serveur HTTP Ocara réponde (sondage, voir la boucle
 * équivalente dans examples/advanced/mini_project/main.oc côté desktop —
 * même principe, ici côté Kotlin puisque rien côté Ocara ne peut prévenir
 * l'Activity directement pour l'instant, voir OcaraBridge) avant de charger
 * la WebView — évite un écran d'erreur si la WebView tente de charger l'URL
 * avant que le serveur écoute réellement.
 */
@SuppressLint("SetJavaScriptEnabled")
@Composable
private fun OcaraWebViewScreen() {
  var serverReady by remember { mutableStateOf(false) }

  LaunchedEffect(Unit) {
    launch {
      while (!serverReady) {
        val ready =
          withContext(Dispatchers.IO) {
            try {
              val conn = URL(OCARA_SERVER_URL).openConnection() as HttpURLConnection
              conn.connectTimeout = 200
              conn.readTimeout = 200
              conn.requestMethod = "GET"
              val ok = conn.responseCode in 200..499 // n'importe quelle réponse HTTP = serveur vivant
              conn.disconnect()
              ok
            } catch (_: Exception) {
              false
            }
          }
        if (ready) {
          serverReady = true
        } else {
          delay(50)
        }
      }
    }
  }

  // Hauteur réelle de la barre de statut (varie selon l'appareil/l'encoche) —
  // le contenu est edge-to-edge (voir enableEdgeToEdge dans MainActivity),
  // donc la WebView ne le sait pas nativement : transmise en CSS px (≈ dp,
  // le viewport HTML fixe `initial-scale=1.0`, voir configs/components/
  // Layout.oc) via une variable custom, injectée en JS après chaque
  // chargement de page (voir OcaraWebViewClient plus bas) plutôt que
  // calculée côté CSS pur (`env(safe-area-inset-top)` resterait à 0 sans
  // plomberie WindowInsets supplémentaire côté WebView — plus de code natif
  // pour un résultat moins direct que cette seule variable).
  val statusBarHeightDp = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()

  Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
    if (serverReady) {
      AndroidView(
        factory = { context ->
          WebView(context).apply {
            settings.javaScriptEnabled = true
            // SANS WebViewClient (même la version de base, sans rien
            // surcharger), Android traite tout clic sur un lien (et toute
            // navigation déclenchée par la soumission d'un formulaire) comme
            // une intention à résoudre par le système — qui l'ouvre alors
            // dans le navigateur par défaut au lieu de rester dans cette
            // WebView. `OcaraWebViewClient` (voir plus bas) hérite de ce
            // comportement (ne surcharge pas `shouldOverrideUrlLoading`) tout
            // en injectant la hauteur de la barre de statut à chaque page.
            webViewClient = OcaraWebViewClient(statusBarHeightDp.value)
            loadUrl(OCARA_SERVER_URL)
          }
        },
        modifier = Modifier.fillMaxSize(),
      )
    } else {
      CircularProgressIndicator()
    }
  }
}

/**
 * `WebViewClient` qui, après CHAQUE chargement de page (pas seulement la
 * première — la navigation reste dans cette même WebView, voir la doc plus
 * haut), pose `--android-status-bar-height` sur `<html>` : `public/style.css`
 * l'utilise (`var(--android-status-bar-height, 0px)`) pour ajouter le padding
 * manquant en haut de la nav, UNIQUEMENT sur Android — sur desktop/navigateur
 * classique, cette variable n'existe jamais, la valeur par défaut `0px`
 * s'applique et le CSS reste inchangé.
 */
private class OcaraWebViewClient(private val statusBarHeightDp: Float) : WebViewClient() {
  override fun onPageFinished(view: WebView, url: String?) {
    super.onPageFinished(view, url)
    view.evaluateJavascript(
      "document.documentElement.style.setProperty('--android-status-bar-height', '${statusBarHeightDp}px')",
      null,
    )
  }
}
