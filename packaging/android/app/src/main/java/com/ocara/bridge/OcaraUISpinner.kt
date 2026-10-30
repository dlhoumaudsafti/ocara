package com.ocara.bridge

import android.graphics.ImageDecoder
import android.graphics.drawable.AnimatedImageDrawable
import android.os.Build
import android.util.Base64
import android.widget.ImageView
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import java.nio.ByteBuffer
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.sin

/**
 * Indicateur d'attente (`spinner`, docs/oc-ui.md « Composants d'écran ») :
 * mêmes modèles que engine.css (`circle`, `dots`, `bars`, `pulse`, `ring`,
 * `orbit`), ou image animée personnelle (`src`, GIF/WEBP animé décodé par
 * `AnimatedImageDrawable`, Android 9 et plus ; image fixe avant).
 */
@Composable
internal fun Spinner(p: Map<String, Any?>, modifier: Modifier) {
    val size = ((p["size"] as? Number)?.toFloat() ?: 32f).dp
    val color = parseColor(p["color"]) ?: MaterialTheme.colorScheme.primary
    val m = modifier.size(size)
    (p["src"] as? String)?.let { return AnimatedPicture(it, m) }
    when (val kind = p["kind"] as? String ?: "circle") {
        "circle" -> CircularProgressIndicator(m, color = color, strokeWidth = size / 8)
        "ring" -> CircularProgressIndicator(m, color = color, strokeWidth = size / 10, trackColor = color.copy(alpha = 0.2f))
        else -> {
            val transition = rememberInfiniteTransition(label = "spinner")
            val phase by transition.animateFloat(0f, 1f, infiniteRepeatable(tween(1200, easing = LinearEasing)), label = "phase")
            Canvas(m) { drawKind(kind, phase, color) }
        }
    }
}

/** Pic triangulaire de largeur `half` autour de `at`, sur un cycle de 1. */
private fun peak(phase: Float, at: Float, half: Float): Float = (1f - abs(((phase % 1f) + 1f) % 1f - at) / half).coerceAtLeast(0f)

private fun androidx.compose.ui.graphics.drawscope.DrawScope.drawKind(kind: String, phase: Float, color: Color) {
    val w = size.width
    when (kind) {
        "dots" -> for (i in 0 until 3) {
            val scale = 0.4f + 0.6f * peak(phase - i * 0.13f, 0.4f, 0.4f)
            drawCircle(color.copy(alpha = scale), radius = w * 0.14f * scale, center = Offset(w * (0.14f + i * 0.36f), w / 2))
        }
        "bars" -> for (i in 0 until 4) {
            val h = w * (0.35f + 0.65f * peak(phase - i * 0.08f, 0.2f, 0.2f))
            drawRoundRect(color, topLeft = Offset(w * i * 0.27f, (w - h) / 2), size = Size(w * 0.18f, h), cornerRadius = CornerRadius(2f, 2f))
        }
        "pulse" -> drawCircle(color.copy(alpha = 1f - phase), radius = w / 2 * (0.2f + 0.8f * phase))
        "orbit" -> {
            drawCircle(color.copy(alpha = 0.25f), radius = w / 2 - 1f, style = Stroke(1.dp.toPx()))
            val angle = phase * 2 * Math.PI - Math.PI / 2
            val r = w / 2 - w * 0.11f
            drawCircle(color, radius = w * 0.11f, center = Offset(w / 2 + (r * cos(angle)).toFloat(), w / 2 + (r * sin(angle)).toFloat()))
        }
    }
}

/** Image animée embarquée (`data:…;base64,…`). */
@Composable
private fun AnimatedPicture(src: String, modifier: Modifier) {
    val bytes = src.substringAfter("base64,", "").takeIf { it.isNotEmpty() }?.let { Base64.decode(it, Base64.DEFAULT) } ?: return
    AndroidView(
        factory = { context ->
            ImageView(context).apply {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    val drawable = ImageDecoder.decodeDrawable(ImageDecoder.createSource(ByteBuffer.wrap(bytes)))
                    setImageDrawable(drawable)
                    (drawable as? AnimatedImageDrawable)?.start()
                } else {
                    setImageBitmap(android.graphics.BitmapFactory.decodeByteArray(bytes, 0, bytes.size))
                }
            }
        },
        modifier = modifier,
    )
}
