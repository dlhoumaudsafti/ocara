use std::path::{Path, PathBuf};
use std::process::Command;

// ─────────────────────────────────────────────────────────────────────────────
// Liaison finale : fichier objet → exécutable natif
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct LinkerError(pub String);

impl std::fmt::Display for LinkerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "linker error: {}", self.0)
    }
}
impl std::error::Error for LinkerError {}

/// Runtime principal, embarqué dans le binaire du compilateur (build.rs + include_bytes!).
static RUNTIME_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime.a"));

/// Runtime Tauri, dans un crate SÉPARÉ du runtime principal (voir runtime_tauri/
/// Cargo.toml). Raison : rustc partitionne son code en unités de compilation sans
/// respecter les frontières de module — même avec `-Wl,--gc-sections` au lien, du
/// code toujours nécessaire (ex: thread.rs) peut finir dans la même unité qu'une
/// fonction Tauri, ce qui la garde vivante et exige GTK/WebKit installés sur la
/// machine cible pour un programme qui n'utilise même pas Tauri (mesuré : ~100
/// bibliothèques dynamiques / 42 Mo pour un simple "Hello World"). Un crate séparé
/// donne une frontière de lien dure : ce .a n'est extrait et lié que pour les
/// programmes qui importent réellement ocara.Tauri (voir `needs_tauri` ci-dessous).
/// Résultat mesuré sans Tauri : 7 bibliothèques dynamiques / 6 Mo.
static RUNTIME_TAURI_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime_tauri.a"));

/// Runtime SDL, même principe et même raison que RUNTIME_TAURI_BYTES ci-dessus
/// (voir runtime_sdl/Cargo.toml) : crate séparé pour que SDL3 (et sa chaîne de
/// build vendored — cmake, X11/Wayland dev) ne soit lié que pour les programmes
/// qui importent réellement ocara.SDL. Contrairement à Tauri, SDL3 est compilé
/// et lié STATIQUEMENT (feature `build-from-source-static`) — aucune bibliothèque
/// dynamique supplémentaire à ajouter au lien final (pas de pkg-config ici).
static RUNTIME_SDL_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime_sdl.a"));

/// Runtime Image (`runtime_image/`) : crate séparé pour que les codecs
/// (PNG, JPEG, WEBP, GIF, BMP — Rust pur, aucune bibliothèque C) ne soient
/// liés que dans les programmes qui importent `ocara.Image`.
static RUNTIME_IMAGE_BYTES: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/libocara_runtime_image.a"));

/// Runtime embarqué extrait sur disque pour l'édition de liens.
struct RuntimeArchive {
    path: PathBuf,
    /// Copie propre à cette compilation (pas de cache utilisateur) : supprimée
    /// à la fin, y compris si la liaison échoue.
    temporary: bool,
}

impl Drop for RuntimeArchive {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Cache utilisateur des runtimes extraits (`$XDG_CACHE_HOME/ocara`,
/// `~/.cache/ocara`, `%LOCALAPPDATA%\ocara`) — jamais un dossier partagé
/// comme `/tmp`, où un fichier déposé par un autre utilisateur serait lié.
fn cache_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))
        .or_else(|| std::env::var_os("LOCALAPPDATA").map(PathBuf::from))?;
    let dir = base.join("ocara");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Extrait des bytes embarqués : une seule copie par contenu dans le cache
/// utilisateur (`lib<label>-<empreinte>.a`, écrite à part puis renommée —
/// une compilation interrompue ne laisse ni copie partielle réutilisée, ni
/// fichier de 79 Mo par compilation), sinon une copie temporaire supprimée
/// après la liaison.
fn extract_runtime(bytes: &[u8], label: &str) -> Result<RuntimeArchive, LinkerError> {
    use std::hash::{Hash, Hasher};
    let err = |e: std::io::Error| LinkerError(format!("extraction de {}: {}", label, e));
    let Some(dir) = cache_dir() else {
        let path = std::env::temp_dir().join(format!("lib{}_{}.a", label, std::process::id()));
        std::fs::write(&path, bytes).map_err(err)?;
        return Ok(RuntimeArchive { path, temporary: true });
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    let path = dir.join(format!("lib{}-{:016x}.a", label, hasher.finish()));
    if std::fs::metadata(&path).is_ok_and(|m| m.len() == bytes.len() as u64) {
        return Ok(RuntimeArchive { path, temporary: false });
    }
    let partial = dir.join(format!(".lib{}-{}.partial", label, std::process::id()));
    std::fs::write(&partial, bytes).map_err(err)?;
    std::fs::rename(&partial, &path).map_err(err)?;
    // Nouvelle version du runtime : les copies des versions précédentes ne
    // servent plus (une seule copie par runtime dans le cache).
    let prefix = format!("lib{}-", label);
    for old in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = old.file_name().to_string_lossy().into_owned();
        let digits = name.strip_prefix(&prefix).and_then(|n| n.strip_suffix(".a"));
        if digits.is_some_and(|d| d.len() == 16 && d.chars().all(|c| c.is_ascii_hexdigit())) && old.path() != path {
            let _ = std::fs::remove_file(old.path());
        }
    }
    Ok(RuntimeArchive { path, temporary: false })
}

/// Résout les flags `--libs` de pkg-config pour un paquet, avec repli automatique
/// `<nom>-4.0` → `<nom>-4.1` (Ubuntu ≥ 24.04 n'a plus les `.pc` en 4.0 — voir le
/// même repli côté Makefile pour la compilation du runtime lui-même). Contrairement
/// au Makefile, ce repli doit être autonome ici : ce code tourne à l'intérieur du
/// binaire `ocara` déjà compilé, longtemps après que le PKG_CONFIG_PATH du build
/// (`.pkgconfig-shim/`) ait cessé d'exister.
fn pkg_config_libs(package: &str) -> Vec<String> {
    let try_pkg = |pkg: &str| -> Option<Vec<String>> {
        let out = Command::new("pkg-config").arg("--libs").arg(pkg).output().ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout);
        Some(s.split_whitespace().map(|f| f.to_string()).collect())
    };

    try_pkg(package)
        .or_else(|| {
            package.strip_suffix("-4.0").and_then(|base| try_pkg(&format!("{base}-4.1")))
        })
        .unwrap_or_default()
}

/// Écrit les bytes objet dans `obj_path` puis lance le linker système.
///
/// `needs_tauri` : vrai si le programme compilé importe `ocara.Tauri` (déterminé
/// par l'appelant via `ir_module.imports`). Contrôle si libocara_runtime_tauri.a
/// et les bibliothèques GTK/WebKit sont liés — voir la doc de RUNTIME_TAURI_BYTES.
/// `needs_sdl` : même principe pour `ocara.SDL` — voir la doc de RUNTIME_SDL_BYTES.
/// `needs_image` : même principe pour `ocara.Image` — voir RUNTIME_IMAGE_BYTES.
pub fn link(
    obj_bytes:   &[u8],
    obj_path:    &Path,
    out_path:    &Path,
    release:     bool,
    needs_tauri: bool,
    needs_sdl:   bool,
    needs_image: bool,
) -> Result<(), LinkerError> {
    // 1. Écriture du fichier objet
    std::fs::write(obj_path, obj_bytes)
        .map_err(|e| LinkerError(format!("écriture objet: {}", e)))?;

    // 2. Extraction du/des runtime(s) embarqué(s) dans /tmp
    let runtime_archive = extract_runtime(RUNTIME_BYTES, "ocara_runtime")?;
    let runtime = &runtime_archive.path;
    let tauri_archive = if needs_tauri { Some(extract_runtime(RUNTIME_TAURI_BYTES, "ocara_runtime_tauri")?) } else { None };
    let runtime_tauri = tauri_archive.as_ref().map(|a| a.path.clone());
    let sdl_archive = if needs_sdl { Some(extract_runtime(RUNTIME_SDL_BYTES, "ocara_runtime_sdl")?) } else { None };
    let runtime_sdl = sdl_archive.as_ref().map(|a| a.path.clone());
    let image_archive = if needs_image { Some(extract_runtime(RUNTIME_IMAGE_BYTES, "ocara_runtime_image")?) } else { None };

    // 3. Liaison : objet + runtime(s) → exécutable
    // --allow-multiple-definition : les symboles du .o (programme) priment sur la .a (runtime)
    // Pilote de lien : `cc` sur Unix (présent par convention sur toute
    // distribution Linux/macOS), mais les distributions MinGW-w64/MSYS2
    // usuelles sur Windows ne fournissent QUE `gcc.exe`, pas systématiquement
    // un alias `cc.exe` — `#[cfg(windows)]` ici reflète la cible RÉELLE de
    // CE `ocara` (le binaire tournant, celui qui exécute ce code), pas
    // l'hôte de compilation d'origine : correct aussi bien pour un `ocara`
    // natif Windows que pour le même binaire cross-compilé en
    // `x86_64-pc-windows-gnu` (voir docs/roadmap.d/packaging-windows.md).
    #[cfg(windows)]
    let linker_driver = "gcc";
    #[cfg(not(windows))]
    let linker_driver = "cc";

    let mut cmd = Command::new(linker_driver);
    cmd.arg(obj_path)
        .arg(&runtime);
    if let Some(rt) = &runtime_tauri {
        cmd.arg(rt);
    }
    if let Some(rt) = &runtime_sdl {
        cmd.arg(rt);
    }
    if let Some(rt) = &image_archive {
        cmd.arg(&rt.path);
    }
    cmd.arg("-o")
        .arg(out_path)
        .arg("-lm");
    // Pas de -lssl/-lcrypto/-lz : OpenSSL (runtime/Cargo.toml, feature
    // "vendored") et zlib (libz-sys, feature "static") sont compilés depuis
    // les sources et intégrés statiquement à libocara_runtime.a — aucun
    // programme compilé avec Ocara n'exige plus libssl.so/libcrypto.so/libz.so
    // sur la machine cible. Les passer ici serait non seulement inutile mais
    // ferait échouer le lien sur une machine de build sans libssl-dev/zlib1g-dev.
    #[cfg(not(windows))]
    // -no-pie : nécessaire, pas un réglage hérité non reconsidéré (voir
    // docs/roadmap.d/securite-lien-no-pie.md pour l'audit complet) —
    // Cranelift (`src/codegen/emit.d/emitter.rs`, `settings::Flags::new`)
    // n'active jamais `is_pic` (faux par défaut dans cranelift-codegen),
    // donc le `.o` généré utilise des relocations absolues, pas du code
    // indépendant de la position. Lier ce `.o` en PIE (sans ce flag)
    // fonctionne mais produit un binaire `DT_TEXTREL` — confirmé par
    // essai (`readelf -d` : `TEXTREL`, `ld` avertit "creating DT_TEXTREL
    // in a PIE") : le chargeur doit alors rendre le segment de code
    // inscriptible au démarrage pour appliquer les relocations, ce qui
    // affaiblit la protection W^X que PIE est censé renforcer — un vrai
    // recul de sécurité différent, pas un gain. Solution correcte pour
    // un jour avoir un vrai PIE (`is_pic = true` chez Cranelift) : chantier
    // séparé, plus large qu'un simple flag de lien (affecte tout
    // l'adressage émis par le codegen) — non entrepris ici.
    // NON APPLICABLE sur Windows : `-no-pie` est un concept ELF (relocations
    // absolues vs PIE+TEXTREL) sans équivalent PE direct — l'ASLR d'un .exe
    // Windows fonctionne différemment (table .reloc), jamais concerné par
    // cette distinction.
    cmd.arg("-no-pie");
    #[cfg(windows)]
    {
        // ocara_runtime utilise des sockets (ocara.HTTPServer/HTTPRequest) et
        // des nombres aléatoires cryptographiques (rand, OpenSSL vendored) —
        // sur Linux, ces symboles viennent de la libc ; sur Windows, ce sont
        // des bibliothèques système SÉPARÉES, jamais liées par défaut.
        // Confirmé par échec de lien réel (pas une supposition) :
        // WSARecv/WSASend/WSAGetLastError/recv/send/closesocket/freeaddrinfo
        // non résolus sans `-lws2_32` en liant un simple "Hello World" —
        // voir docs/roadmap.d/packaging-windows.md.
        cmd.arg("-lws2_32")
            .arg("-lbcrypt")
            .arg("-luserenv")
            .arg("-lntdll");
    }
    cmd.arg("-Wl,--allow-multiple-definition")
        // --gc-sections/--as-needed : élague au lien tout ce qui n'est pas
        // réellement atteignable depuis le programme (rustc émet une section ELF
        // par fonction par défaut). Redondant avec la séparation de crate pour
        // Tauri (déjà exclu du lien si needs_tauri est faux), mais garde le même
        // bénéfice pour les autres builtins volumineux (SQLite, MySQL, regex...).
        .arg("-Wl,--gc-sections")
        .arg("-Wl,--as-needed");

    // GTK/WebKit : uniquement si le programme importe réellement ocara.Tauri.
    if needs_tauri {
        for pkg in ["gtk+-3.0", "webkit2gtk-4.0", "javascriptcoregtk-4.0"] {
            for flag in pkg_config_libs(pkg) {
                cmd.arg(flag);
            }
        }
    }

    // --release : demande au linker de supprimer les symboles (strip intégré)
    if release {
        cmd.arg("-Wl,-s");
    }

    let status = cmd.status()
        .map_err(|e| LinkerError(format!("impossible de lancer {}: {}", linker_driver, e)))?;

    // 4. L'objet intermédiaire ne reste pas à côté du binaire (les runtimes
    //    temporaires sont supprimés par `RuntimeArchive`).
    let _ = std::fs::remove_file(obj_path);

    if !status.success() {
        return Err(LinkerError(format!(
            "{} a échoué avec le code: {:?}",
            linker_driver, status.code()
        )));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Liaison croisée Android : fichier objet → bibliothèque partagée (.so)
// Sous-chantier 2 de docs/roadmap.d/packaging-android.md.
// ─────────────────────────────────────────────────────────────────────────────

/// Niveau d'API Android visé par le clang du NDK (`<arch><API>-clang`).
/// DOIT correspondre au niveau utilisé pour compiler `runtime_lib` (voir
/// `ANDROID_API` dans le Makefile, cible `build-runtime-android`) — un
/// décalage ne casse rien à la liaison elle-même (le clang du NDK accepte de
/// lier des objets visant une API différente de la sienne) mais peut exposer
/// des symboles libc absents de l'API réellement visée à l'exécution. Valeur
/// unique ici faute de besoin de la rendre configurable pour l'instant.
const ANDROID_API_LEVEL: u32 = 24;

/// Symbole exporté par `runtime_android_jni` (voir `runtime_android_jni/src/lib.rs`)
/// — DOIT correspondre exactement au nom de fonction `#[unsafe(no_mangle)]`
/// défini là-bas (convention JNI `Java_<package_>_<Classe>_<méthode>`, points
/// remplacés par des underscores). Utilisé ci-dessous avec `-Wl,-u,` : sans ce
/// flag, l'éditeur de liens n'a AUCUNE raison d'extraire ce symbole de
/// `libocara_runtime_android_jni.a` (rien dans le programme Ocara ni dans le
/// runtime ne l'appelle — seule la JVM le fait, dynamiquement, à l'exécution),
/// et l'extraction paresseuse habituelle d'un `.a` le laisse alors
/// silencieusement de côté : confirmé par reproduction, `nm -D` sur le `.so`
/// produit sans ce flag ne montre AUCUNE trace du symbole, ni dans le binaire
/// ni dans sa table de symboles dynamiques.
const ANDROID_JNI_BRIDGE_ENTRY_SYMBOL: &str =
    "Java_com_ocara_bridge_OcaraBridge_nativeStartServer";

/// Événements Kotlin → Ocara (`ocara.AndroidUI`), même raison : appelé par
/// la JVM, jamais référencé par le programme.
const ANDROID_JNI_BRIDGE_EVENT_SYMBOL: &str =
    "Java_com_ocara_bridge_OcaraBridge_nativeEvent";

/// Résout le nom du clang du NDK pour un triple Android donné. Seuls les
/// triples effectivement vérifiés par ce chantier (voir packaging-android.md)
/// sont acceptés — pas de correspondance approximative pour les autres, qui
/// échoueraient probablement de façon subtile (ex: `armv7-linux-androideabi`
/// utilise le préfixe clang `armv7a`, pas `armv7`, une des nombreuses
/// irrégularités de nommage du NDK).
fn android_clang_prefix(triple: &str) -> Result<&'static str, LinkerError> {
    match triple {
        "aarch64-linux-android" => Ok("aarch64-linux-android"),
        "armv7-linux-androideabi" => Ok("armv7a-linux-androideabi"),
        "x86_64-linux-android" => Ok("x86_64-linux-android"),
        "i686-linux-android" => Ok("i686-linux-android"),
        other => Err(LinkerError(format!(
            "cible Android non prise en charge pour la liaison: '{}' \
             (seuls aarch64-linux-android, armv7-linux-androideabi, \
             x86_64-linux-android, i686-linux-android sont vérifiés)",
            other
        ))),
    }
}

/// Écrit les bytes objet dans `obj_path` puis lie une bibliothèque partagée
/// Android (`.so`) avec le clang du NDK, à partir d'un `libocara_runtime.a`
/// pré-compilé pour la même cible (voir `CliArgs::android_runtime` —
/// contrairement au runtime hôte, PAS embarqué dans le binaire `ocara`).
///
/// `runtime_sdl_lib` : `libocara_runtime_sdl.a` optionnel (voir
/// `CliArgs::android_runtime_sdl`, produit par `make build-runtime-sdl-android`)
/// — requis si et seulement si le programme importe `ocara.SDL` (voir
/// `main.rs`, `needs_sdl`). Tauri, lui, n'est jamais supporté ici : GTK n'a
/// structurellement aucun équivalent Android (hors périmètre définitif).
///
/// `jni_bridge_lib` : `libocara_runtime_android_jni.a` optionnel (voir
/// `CliArgs::android_jni_bridge`, crate `runtime_android_jni`) — indépendant
/// de tout import Ocara (contrairement à `runtime_sdl_lib`) : c'est un choix
/// de packaging (« ce `.so` doit être chargeable par une Activity Android via
/// JNI », voir docs/roadmap.d/packaging-android-webview-hybrid.md), pas une
/// dépendance du programme compilé.
/// Archives des builtins `ocara.Android*` importés
/// (`libocara_runtime_android_<domaine>.a`, `make build-android-features`),
/// cherchées dans le dossier de `runtime_lib` : un domaine non importé n'entre
/// pas dans le `.so`.
/// `ocara.UI` utilise l'archive de `ocara.AndroidUI` (backend Compose,
/// runtime_android/ui), une seule fois si les deux sont importés.
pub fn android_feature_libs(imports: &[String], runtime_lib: &Path) -> Vec<std::path::PathBuf> {
    let imported = |name: &str| imports.iter().any(|m| m == name);
    let mut domains: Vec<&str> = crate::builtins::android::ANDROID_BUILTINS.iter()
        .filter(|(name, _)| imported(name))
        .map(|(_, domain)| *domain)
        .collect();
    if imported("UI") && !domains.contains(&"ui") {
        domains.push("ui");
    }
    domains.into_iter()
        .map(|domain| runtime_lib.with_file_name(format!("libocara_runtime_android_{domain}.a")))
        .collect()
}

pub fn link_android(
    obj_bytes:      &[u8],
    obj_path:       &Path,
    out_path:       &Path,
    target:         &str,
    ndk_home:       &Path,
    runtime_lib:    &Path,
    runtime_sdl_lib: Option<&Path>,
    feature_libs:   &[std::path::PathBuf],
    jni_bridge_lib: Option<&Path>,
    release:        bool,
) -> Result<(), LinkerError> {
    // 1. Écriture du fichier objet
    std::fs::write(obj_path, obj_bytes)
        .map_err(|e| LinkerError(format!("écriture objet: {}", e)))?;

    if !runtime_lib.exists() {
        return Err(LinkerError(format!(
            "runtime Android introuvable: '{}' (voir --android-runtime, produit par \
             `make build-runtime-android`, docs/roadmap.d/packaging-android.md)",
            runtime_lib.display()
        )));
    }
    if let Some(sdl_lib) = runtime_sdl_lib {
        if !sdl_lib.exists() {
            return Err(LinkerError(format!(
                "runtime SDL Android introuvable: '{}' (voir --android-runtime-sdl, produit par \
                 `make build-runtime-sdl-android`, docs/roadmap.d/packaging-android.md)",
                sdl_lib.display()
            )));
        }
    }
    if let Some(missing) = feature_libs.iter().find(|lib| !lib.exists()) {
        return Err(LinkerError(format!(
            "builtin Android introuvable: '{}' (produit par `make build-android-features`, \
             à côté de --android-runtime, docs/android.md)",
            missing.display()
        )));
    }
    if let Some(jni_lib) = jni_bridge_lib {
        if !jni_lib.exists() {
            return Err(LinkerError(format!(
                "pont JNI Android introuvable: '{}' (voir --android-jni-bridge, produit par \
                 `cargo build --target <triple> -p ocara_runtime_android_jni`, \
                 docs/roadmap.d/packaging-android-webview-hybrid.md)",
                jni_lib.display()
            )));
        }
    }

    let clang_prefix = android_clang_prefix(target)?;
    let clang_path = ndk_home
        .join("toolchains/llvm/prebuilt/linux-x86_64/bin")
        .join(format!("{}{}-clang", clang_prefix, ANDROID_API_LEVEL));
    if !clang_path.exists() {
        return Err(LinkerError(format!(
            "clang du NDK introuvable: '{}' (--android-ndk / $ANDROID_NDK_HOME pointe-t-il \
             bien vers la racine d'un NDK installé ?)",
            clang_path.display()
        )));
    }

    // 2. Liaison : objet + runtime → bibliothèque partagée
    // -shared -fPIC : un `.so` Android, jamais un exécutable autonome — le
    // code natif est chargé via JNI dans le processus de l'app. Pas de
    // -no-pie (propre au chemin exécutable hôte, voir `link()` ci-dessus) :
    // le dynamic linker Bionic refuse tout `.so` avec DT_TEXTREL, d'où
    // `is_pic=true` déjà activé côté codegen pour toute cible croisée (voir
    // `CraneliftEmitter::new`).
    let mut cmd = Command::new(&clang_path);
    cmd.arg(obj_path)
        .arg(runtime_lib);
    if let Some(sdl_lib) = runtime_sdl_lib {
        cmd.arg(sdl_lib);
    }
    cmd.args(feature_libs);
    if let Some(jni_lib) = jni_bridge_lib {
        cmd.arg(jni_lib);
        // -u force l'extraction du membre de l'archive qui définit ce symbole,
        // même sans référence entrante — voir la doc d'ANDROID_JNI_BRIDGE_ENTRY_SYMBOL.
        cmd.arg(format!("-Wl,-u,{}", ANDROID_JNI_BRIDGE_ENTRY_SYMBOL));
        cmd.arg(format!("-Wl,-u,{}", ANDROID_JNI_BRIDGE_EVENT_SYMBOL));
    }
    let status = cmd.arg("-o").arg(out_path)
        .arg("-shared")
        .arg("-fPIC")
        .arg("-lm")
        // -lz : contrairement à OpenSSL (vendored, vraiment statique y
        // compris pour Android — vérifié, `libssl`/`libcrypto` n'apparaissent
        // dans AUCUN NEEDED du .so produit), `libz-sys` (dépendance
        // transitive d'ureq/mysql via flate2) IGNORE délibérément la feature
        // "static" sur Android : son build.rs suppose que « tout compilateur
        // Android est livré avec libz » et émet toujours un lien dynamique
        // (`cargo:rustc-link-lib=z`) — confirmé en lisant
        // libz-sys/build.rs et en observant `deflate`/`inflate`/`zlibVersion`
        // rester UND (non résolus, non "weak") dans le .so tant que ce -lz
        // est absent. Le NDK fournit bien un `libz.so` de liaison (présent
        // sur Android depuis très longtemps), donc ce lien dynamique est sûr.
        .arg("-lz")
        .args(if runtime_sdl_lib.is_some() {
            // Bibliothèques système NDK requises par SDL3 (+ image/ttf/mixer)
            // sur Android — résolues empiriquement (`nm -D --undefined-only`
            // sur un premier `.so` sans ces flags, voir la vérification dans
            // docs/roadmap.d/packaging-android.md sous-chantier 4), pas
            // devinées à l'avance :
            // -landroid  : AAsset*/AAssetManager* (assets APK), ALooper*,
            //              ANativeWindow* (surface de rendu), ASensor*
            //              (accéléromètre/gyroscope) — API NDK "android".
            // -llog      : __android_log_print/__android_log_write (logs
            //              système, utilisés par SDL en interne).
            // -lGLESv2   : rendu OpenGL ES 2 (backend graphique de SDL_render).
            // -lOpenSLES : slCreateEngine/SL_IID_* (backend audio de SDL_audio).
            // -lc++      : runtime C++ (__cxa_*, operator new/delete,
            //              std::terminate) — au moins un codec vendored de
            //              SDL_mixer (ex: "gme", chiptune) est en C++. Résolu
            //              par le NDK vers `libc++_shared.so` (voir le script
            //              de lien `libc++.so` du sysroot) — DOIT être copié
            //              dans `jniLibs/<abi>/` de l'APK final à côté de ce
            //              `.so` (bibliothèque partagée, pas embarquée ici ;
            //              NDK/Google déconseillent explicitement de lier
            //              libc++ statiquement dans plusieurs `.so` d'un même
            //              processus — état partagé de la gestion d'exceptions).
            vec!["-landroid", "-llog", "-lGLESv2", "-lOpenSLES", "-lc++"]
        } else { vec![] })
        // -landroid : `ASensorManager*`/`ALooper*` d'`ocara.AndroidSensors`
        // (inutile, donc écarté par --as-needed, pour les autres domaines).
        .args(if !feature_libs.is_empty() && runtime_sdl_lib.is_none() { vec!["-landroid"] } else { vec![] })
        .arg("-Wl,--allow-multiple-definition")
        .arg("-Wl,--gc-sections")
        .arg("-Wl,--as-needed")
        .args(if release { vec!["-Wl,-s"] } else { vec![] })
        .status()
        .map_err(|e| LinkerError(format!("impossible de lancer {}: {}", clang_path.display(), e)))?;
    let _ = std::fs::remove_file(obj_path);

    if !status.success() {
        return Err(LinkerError(format!(
            "{} a échoué avec le code: {:?}",
            clang_path.display(), status.code()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::android_feature_libs;
    use std::path::Path;

    #[test]
    fn android_feature_libs_follow_imports() {
        let imports = vec!["IO".to_string(), "AndroidSensors".to_string(), "AndroidCamera".to_string()];
        let libs = android_feature_libs(&imports, Path::new("/t/release/libocara_runtime.a"));
        let names: Vec<String> = libs.iter().map(|p| p.display().to_string()).collect();
        assert_eq!(names, vec![
            "/t/release/libocara_runtime_android_sensors.a",
            "/t/release/libocara_runtime_android_camera.a",
        ]);
        assert!(android_feature_libs(&["IO".to_string()], Path::new("/t/libocara_runtime.a")).is_empty());
    }
}
