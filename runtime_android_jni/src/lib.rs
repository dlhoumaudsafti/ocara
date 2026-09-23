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

use jni::objects::{JClass, JString};
use jni::sys::jboolean;
use jni::JNIEnv;

// `mallopt` existe bien dans Bionic (libc d'Android) mais n'est PAS exposé
// par le crate `libc` pour la cible `android` (seulement pour glibc/hurd/aix/
// nto, vérifié dans les sources du crate) — déclaré ici à la main, avec
// exactement la signature C de Bionic (`int mallopt(int, int)`).
unsafe extern "C" {
    fn mallopt(param: i32, value: i32) -> i32;
}

/// Voir https://source.android.com/docs/security/test/tagged-pointers : depuis
/// Android 11, l'allocateur natif (Scudo) peut renvoyer des pointeurs de tas
/// dont l'OCTET DE POIDS FORT (bits 56-63) est non nul (un "tag" matériel/logiciel
/// pour la détection d'use-after-free — le CPU ARM64 l'ignore pour les accès
/// mémoire réels via Top-Byte-Ignore, mais PAS une comparaison numérique i64
/// ordinaire). `ocara_runtime` distingue un entier brut d'un pointeur tas
/// par la MAGNITUDE de la valeur i64 (`is_ptr`/`get_value_type`, voir
/// runtime/src/lib.rs) — un pointeur ainsi tagué peut apparaître négatif ou
/// dépasser `MAX_USERSPACE_ADDR`, et se fait alors classer à tort comme un
/// entier brut. Confirmé par reproduction sur un vrai appareil (pas une
/// supposition) : `System::OS + "-world"` renvoyait un grand nombre au lieu
/// du texte concaténé — voir docs/roadmap.d/runtime-android-aarch64-pic-string-concat.md.
///
/// Corrigé ICI plutôt que dans la logique de classification du runtime :
/// `M_BIONIC_SET_HEAP_TAGGING_LEVEL` (`mallopt`, constantes vérifiées contre
/// les sources Bionic officielles) désactive le tagging pour TOUT le
/// processus, à partir de cet appel — tous les pointeurs alloués ensuite par
/// `alloc()` (donc par `alloc_str`/`box_int_if_needed`/... dans
/// `ocara_runtime`) reviennent avec un octet de poids fort à zéro, sans
/// modifier la représentation existante ni risquer de casser la distinction
/// entier négatif brut / pointeur (une correction par masquage de bits dans
/// le runtime aurait dû, elle, re-décider au cas par cas si un octet de poids
/// fort non nul est un tag ou un entier négatif légitime — ambigu par
/// construction, voir le ticket). Sans effet sur le chemin desktop (x86_64/
/// glibc ne tague jamais ses pointeurs ainsi) : ce fix n'existe que dans ce
/// crate, spécifique à Android.
const M_BIONIC_SET_HEAP_TAGGING_LEVEL: i32 = -204;
const M_HEAP_TAGGING_LEVEL_NONE: i32 = 0;

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
/// `data_dir` : le répertoire de stockage interne de l'app
/// (`Context.getFilesDir().getAbsolutePath()`, PAS deviné côté natif — voir
/// plus bas pourquoi). AVANT de démarrer le programme Ocara, ce processus
/// fait `chdir()` vers ce répertoire — ainsi, TOUT chemin relatif utilisé par
/// le programme Ocara (`"./app.db"`, `"./public/"`, comme sur desktop où le
/// répertoire de travail est celui depuis lequel on lance le programme)
/// pointe vers un endroit qui existe déjà et est garanti accessible en
/// écriture par l'app, SANS qu'aucun code Ocara n'ait besoin de connaître de
/// chemin Android spécifique ni de faire de branche `if System::OS ==
/// "android"`. Voir docs/roadmap.d/packaging-android-webview-hybrid.md.
///
/// Pourquoi passer ce chemin depuis Java plutôt que le deviner côté natif
/// (ex: à partir du nom de package lu dans `/proc/self/cmdline`) : `Context.
/// getFilesDir()` est la SEULE API qui garantit la création réelle du
/// répertoire ET son étiquetage SELinux correct par le framework Android —
/// une reproduction sur un vrai appareil (voir le ticket) a montré qu'un
/// chemin construit à la main, même dans le bac à sable de l'app, pouvait
/// échouer à l'ouverture SQLite là où `getFilesDir()` fonctionne.
///
/// Le thread lancé ici n'est PAS attaché à la JVM (`AttachCurrentThread`) :
/// volontairement, le programme Ocara qui tourne dessus n'a aujourd'hui aucun
/// besoin de rappeler du code Java (pas de callback JNI depuis `main`) — à
/// revoir seulement si un futur besoin concret l'exige.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ocara_bridge_OcaraBridge_nativeStartServer<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    data_dir: JString<'local>,
) -> jboolean {
    // EN PREMIER, avant toute allocation faite par le programme Ocara (voir
    // la doc de M_BIONIC_SET_HEAP_TAGGING_LEVEL plus haut) — un réglage
    // process-wide, une seule fois suffit avant que le thread `main()` ne
    // soit lancé plus bas. Best-effort : une valeur de retour 0 (échec,
    // Bionic absent/trop ancien) n'empêche pas de continuer, elle laisse
    // juste le bug de tagging non corrigé sur un tel appareil.
    unsafe { mallopt(M_BIONIC_SET_HEAP_TAGGING_LEVEL, M_HEAP_TAGGING_LEVEL_NONE) };

    let data_dir: String = match env.get_string(&data_dir) {
        Ok(s) => s.into(),
        Err(_) => return jni::sys::JNI_FALSE,
    };

    let c_data_dir = match std::ffi::CString::new(data_dir) {
        Ok(c) => c,
        Err(_) => return jni::sys::JNI_FALSE, // chemin avec un NUL interne — ne devrait jamais arriver
    };
    if unsafe { libc::chdir(c_data_dir.as_ptr()) } != 0 {
        return jni::sys::JNI_FALSE;
    }

    std::thread::spawn(|| {
        // `main` peut légitimement ne jamais retourner (un serveur HTTP tourne
        // en boucle) — c'est le comportement attendu, pas un thread qui fuit :
        // il vit aussi longtemps que le processus de l'app Android.
        unsafe { main(); }
    });
    jni::sys::JNI_TRUE
}
