plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.compose.compiler)
  alias(libs.plugins.kotlin.serialization)
}

android {
    namespace = "com.ocara.demo"
    compileSdk = 36
    defaultConfig {
        applicationId = "com.ocara.demo"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
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
