package com.ocara.demo

import android.annotation.SuppressLint
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
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
import com.ocara.demo.theme.OcaraUIHybridDemoTheme
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

    // Fire-and-forget : voir la doc de OcaraBridge.nativeStartServer — ne
    // bloque jamais ce thread (le thread principal/UI).
    OcaraBridge.nativeStartServer()

    enableEdgeToEdge()
    setContent {
      OcaraUIHybridDemoTheme {
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
          OcaraWebViewScreen()
        }
      }
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

  Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
    if (serverReady) {
      AndroidView(
        factory = { context ->
          WebView(context).apply {
            settings.javaScriptEnabled = true
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
