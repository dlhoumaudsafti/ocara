//! Pont JNI générique entre une Activity Android et un programme Ocara compilé
//! pour Android (voir docs/roadmap.d/packaging-android-webview-hybrid.md).
//!
//! Ce crate ne contient RIEN de spécifique à un programme Ocara particulier —
//! il expose une seule fonction native, `nativeStartServer`, censée être
//! appelée par une classe Java/Kotlin fixe (`com.ocara.bridge.OcaraBridge`,
//! voir packaging/android/) après `System.loadLibrary`. Le nom de cette
//! classe est une convention figée plutôt que configurable : aucun besoin
//! connu de la rendre variable (n'importe quelle app peut inclure cette même
//! classe Java, quel que soit son propre package), et JNI résout les méthodes
//! natives par correspondance exacte de nom (`Java_<package_>_<Classe>_<méthode>`,
//! points remplacés par des underscores) — un nom de classe différent
//! exigerait de régénérer ce symbole, pas juste une valeur de configuration.

use jni_sys::{jboolean, jclass, JNIEnv, JNI_TRUE};

// Le point d'entrée du programme Ocara lié dans le même `.so` final (voir
// `link_android`, src/codegen/link.rs) — Cranelift l'émet comme une fonction
// ordinaire nommée `main`, sans paramètres, retournant un `i64`
// (`src/lower/builder.d/runtime.rs`, `LowerBuilder::new(module, "main", vec![],
// IrType::I64)`) — PAS le point d'entrée d'un exécutable façon libc (`argc`/
// `argv`, retour `i32`) : cette bibliothèque n'est jamais exécutée comme un
// processus autonome, juste chargée par `dlopen` (Bionic, via
// `System.loadLibrary`), donc `main` n'est ici qu'un symbole exporté ordinaire
// à appeler explicitement.
unsafe extern "C" {
    fn main() -> i64;
}

/// Démarre le programme Ocara (donc, pour l'exemple visé par ce chantier,
/// `examples/advanced/mini_project` et son `ocara.HTTPServer`) sur un thread
/// séparé, et revient IMMÉDIATEMENT — ce n'est PAS le thread principal de
/// l'Activity Android (bloquer ce thread bloquerait toute l'UI). Fire-and-forget
/// volontaire pour cette première version : aucun moyen pour l'appelant Java
/// de savoir quand le serveur est réellement prêt à répondre, ni de l'arrêter
/// proprement (voir la doc de packaging-android-webview-hybrid.md, "cycle de
/// vie Android", pas encore résolu) — l'Activity doit pour l'instant deviner
/// un délai ou sonder le port avant de charger la WebView.
///
/// Le thread lancé ici n'est PAS attaché à la JVM (`AttachCurrentThread`) :
/// volontairement, le programme Ocara qui tourne dessus n'a aujourd'hui aucun
/// besoin de rappeler du code Java (pas de callback JNI depuis `main`) — à
/// revoir seulement si un futur besoin concret l'exige.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ocara_bridge_OcaraBridge_nativeStartServer(
    _env: JNIEnv,
    _clazz: jclass,
) -> jboolean {
    std::thread::spawn(|| {
        // `main` peut légitimement ne jamais retourner (un serveur HTTP tourne
        // en boucle) — c'est le comportement attendu, pas un thread qui fuit :
        // il vit aussi longtemps que le processus de l'app Android.
        unsafe { main(); }
    });
    JNI_TRUE
}
