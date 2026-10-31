import java.awt.RenderingHints
import java.awt.image.BufferedImage
import javax.imageio.ImageIO
import java.io.File as JFile

plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.compose.compiler)
  alias(libs.plugins.kotlin.serialization)
}

// Nom et icône de l'app, fournis par le programme empaqueté (Makefile de
// l'exemple : `-PocaraAppName=Carfolio -PocaraIcon=/chemin/logo.png`) ;
// absents : ceux de l'app de démonstration. L'icône (PNG carré, idéalement
// 512 px) devient les icônes du lanceur de chaque densité.
val ocaraAppName = (findProperty("ocaraAppName") as String?) ?: "Ocara UIHybrid Demo"
val ocaraIcon = (findProperty("ocaraIcon") as String?)?.let { file(it) }
val ocaraIconRes = layout.buildDirectory.dir("generated/ocaraIcon/res")

android {
    namespace = "com.ocara.demo"
    compileSdk = 36
    defaultConfig {
        applicationId = "com.ocara.demo"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
        resValue("string", "app_name", ocaraAppName)
    }

    // Icône du programme : par-dessus celles de src/main (les jeux de sources
    // d'un type de build passent devant `main`).
    if (ocaraIcon != null) {
        sourceSets.getByName("debug").res.directories.add(ocaraIconRes.get().asFile.path)
        sourceSets.getByName("release").res.directories.add(ocaraIconRes.get().asFile.path)
    }

    // Build de production : signé avec un vrai keystore, jamais celui de
    // debug auto-généré (non installable/impossible à mettre à jour sur un
    // appareil ayant déjà l'app installée avec une autre signature). Les
    // secrets viennent UNIQUEMENT de variables d'environnement — jamais en
    // dur ici (ce fichier est commité) — voir `make android-production`
    // (examples/advanced/mini_project/Makefile) et docs/android.md pour la
    // procédure complète (génération du keystore, variables requises).
    // `storeFile` reste `null` si `OCARA_RELEASE_KEYSTORE` n'est pas défini :
    // `assembleRelease` échoue alors explicitement (pas de build non signé
    // silencieux) — voir la vérification faite par `make android-production`
    // avant même d'invoquer Gradle, pour un message d'erreur plus clair que
    // celui de Gradle seul.
    signingConfigs {
        create("release") {
            val keystorePath = System.getenv("OCARA_RELEASE_KEYSTORE")
            if (keystorePath != null) {
                storeFile = file(keystorePath)
                storePassword = System.getenv("OCARA_RELEASE_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("OCARA_RELEASE_KEY_ALIAS")
                keyPassword = System.getenv("OCARA_RELEASE_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.getByName("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    buildFeatures {
      compose = true
      aidl = false
      buildConfig = false
      resValues = true
      shaders = false
    }

    packaging {
      resources {
        excludes += "/META-INF/{AL2.0,LGPL2.1}"
      }
    }
}

kotlin {
    jvmToolchain(17)
}

/**
 * Icônes du lanceur depuis `ocaraIcon` : repli des API 24/25 (`ic_launcher`,
 * `ic_launcher_round`, 48 dp) et premier plan de l'icône adaptative
 * (`ic_launcher_foreground`, 108 dp, image réduite à la zone sûre de 72 dp).
 */
val generateOcaraIcon by tasks.registering {
    val source = ocaraIcon
    val out = ocaraIconRes
    onlyIf { source != null }
    if (source != null) inputs.file(source)
    outputs.dir(out)
    doLast {
        val image = ImageIO.read(source!!) ?: error("ocaraIcon : image illisible (${source})")
        fun write(file: JFile, size: Int, content: Int) {
            val canvas = BufferedImage(size, size, BufferedImage.TYPE_INT_ARGB)
            val g = canvas.createGraphics()
            g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BICUBIC)
            g.setRenderingHint(RenderingHints.KEY_RENDERING, RenderingHints.VALUE_RENDER_QUALITY)
            g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON)
            val offset = (size - content) / 2
            g.drawImage(image, offset, offset, content, content, null)
            g.dispose()
            file.parentFile.mkdirs()
            ImageIO.write(canvas, "png", file)
        }
        val root = out.get().asFile
        root.deleteRecursively()
        for ((density, scale) in listOf("mdpi" to 1.0, "hdpi" to 1.5, "xhdpi" to 2.0, "xxhdpi" to 3.0, "xxxhdpi" to 4.0)) {
            val dir = JFile(root, "mipmap-$density")
            val legacy = (48 * scale).toInt()
            write(JFile(dir, "ic_launcher.png"), legacy, legacy)
            write(JFile(dir, "ic_launcher_round.png"), legacy, legacy)
            write(JFile(dir, "ic_launcher_foreground.png"), (108 * scale).toInt(), (72 * scale).toInt())
        }
    }
}
tasks.named("preBuild") { dependsOn(generateOcaraIcon) }

dependencies {
  val composeBom = platform(libs.androidx.compose.bom)
  implementation(composeBom)
  androidTestImplementation(composeBom)

  // Core Android dependencies
  implementation(libs.androidx.core.ktx)
  implementation(libs.androidx.lifecycle.runtime.ktx)
  implementation(libs.androidx.activity.compose)

  // Arch Components
  implementation(libs.androidx.lifecycle.runtime.compose)
  implementation(libs.androidx.lifecycle.viewmodel.compose)

  // Compose
  implementation(libs.androidx.compose.ui)
  implementation(libs.androidx.compose.ui.tooling.preview)
  implementation(libs.androidx.compose.material3)
  // Tooling
  debugImplementation(libs.androidx.compose.ui.tooling)
  // Instrumented tests
  androidTestImplementation(libs.androidx.compose.ui.test.junit4)
  debugImplementation(libs.androidx.compose.ui.test.manifest)

  // Local tests: jUnit, coroutines, Android runner
  testImplementation(libs.junit)
  testImplementation(libs.kotlinx.coroutines.test)

  // Instrumented tests: jUnit rules and runners
  androidTestImplementation(libs.androidx.test.core)
  androidTestImplementation(libs.androidx.test.ext.junit)
  androidTestImplementation(libs.androidx.test.runner)
  androidTestImplementation(libs.androidx.test.espresso.core)

  // Navigation
  implementation(libs.androidx.navigation3.ui)
  implementation(libs.androidx.navigation3.runtime)
  implementation(libs.androidx.lifecycle.viewmodel.navigation3)
}
