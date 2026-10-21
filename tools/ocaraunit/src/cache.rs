// ─────────────────────────────────────────────────────────────────────────────
// Cache des binaires de test (`<racine>/.ocaraunit_cache/`)
//
// Un binaire par fichier de test : `<stem>-<chemin>-<contenu>`, où
// `<chemin>` est le hash du chemin du test (deux `FooTest.oc` de dossiers
// différents ne se marchent pas dessus) et `<contenu>` le hash du test, de
// ses imports transitifs et du compilateur. À chaque nouvelle compilation,
// les versions précédentes du même test et tous les `.o` sont supprimés :
// le cache ne grossit plus sans fin.
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use crate::{config::collect_oc_files, imports::import_closure};

pub const CACHE_DIR: &str = ".ocaraunit_cache";

pub fn hash_metadata(path: &Path, hasher: &mut DefaultHasher) {
    path.hash(hasher);
    if let Ok(meta) = fs::metadata(path) {
        meta.len().hash(hasher);
        if let Ok(modified) = meta.modified() {
            modified.hash(hasher);
        }
    }
}

/// Empreinte commune à tous les tests : le compilateur (qui embarque le
/// runtime) et chaque `.oc` sous la racine (fichiers chargés autrement que
/// par un import direct, wiring…) — métadonnées seulement (chemin, taille,
/// date), calculée une fois par exécution.
pub fn build_fingerprint(ocara: &Path, project_root: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_metadata(ocara, &mut hasher);
    let files = collect_oc_files(project_root, &[CACHE_DIR.to_string()], project_root);
    for file in files.iter().filter(|f| !f.to_string_lossy().contains("__ocaraunit__")) {
        hash_metadata(file, &mut hasher);
    }
    hasher.finish()
}

/// Clé du binaire d'un test : contenu du test, empreinte commune et
/// métadonnées de chaque fichier réellement importé (transitivement, y
/// compris hors de la racine — `import X from "../../lib/X"`).
pub fn test_key(src: &Path, project_root: &Path, fingerprint: u64) -> Option<u64> {
    let content = fs::read(src).ok()?;
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    fingerprint.hash(&mut hasher);
    for file in import_closure(src, project_root) {
        hash_metadata(&file, &mut hasher);
    }
    Some(hasher.finish())
}

/// Nom d'une entrée de cache : stem du test et hash de son chemin.
pub struct EntryName {
    stem:      String,
    path_hash: String,
}

impl EntryName {
    /// `variant` : variante `when` d'un `RuntimeTest.oc` (vide sinon), un
    /// binaire chacune.
    pub fn of(src: &Path, variant: &str) -> EntryName {
        let mut stem = src.file_stem().unwrap_or_default().to_string_lossy().to_string();
        if !variant.is_empty() {
            stem = format!("{}.{}", stem, variant);
        }
        let mut hasher = DefaultHasher::new();
        src.canonicalize().unwrap_or_else(|_| src.to_path_buf()).hash(&mut hasher);
        EntryName { stem, path_hash: format!("{:x}", hasher.finish()) }
    }

    pub fn path(&self, cache_dir: &Path, key: u64) -> PathBuf {
        cache_dir.join(format!("{}-{}-{:x}", self.stem, self.path_hash, key))
    }

    /// Vrai si `name` est une entrée périmée une fois `keep` compilé : son
    /// `.o`, les autres versions du même test (et leurs `.o`), et les
    /// entrées de l'ancien format `<stem>-<hash>` du même stem.
    pub fn is_stale(&self, name: &str, keep: &str) -> bool {
        let base = name.strip_suffix(".o").unwrap_or(name);
        if base == keep { return name != keep; }
        let Some(rest) = base.strip_prefix(&self.stem).and_then(|r| r.strip_prefix('-')) else {
            return false;
        };
        match rest.strip_prefix(&self.path_hash).and_then(|r| r.strip_prefix('-')) {
            Some(key) => is_hex(key),
            None => is_hex(rest),
        }
    }
}

fn is_hex(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// Supprime les entrées périmées après la compilation de `bin`.
pub fn purge(cache_dir: &Path, bin: &Path, entry: &EntryName) {
    let keep = bin.file_name().unwrap_or_default().to_string_lossy().to_string();
    let Ok(rd) = fs::read_dir(cache_dir) else { return };
    for item in rd.flatten() {
        if entry.is_stale(&item.file_name().to_string_lossy(), &keep) {
            let _ = fs::remove_file(item.path());
        }
    }
}
