// ─────────────────────────────────────────────────────────────────────────────
// ocara.Directory — Opérations sur répertoires
//
// Fonctions exportées (convention C) :
//
//   Directory_create(path_ptr) → void
//   Directory_create_recursive(path_ptr) → void
//   Directory_remove(path_ptr) → void
//   Directory_remove_recursive(path_ptr) → void
//   Directory_list(path_ptr) → i64            // string[]
//   Directory_list_files(path_ptr) → i64      // string[]
//   Directory_list_dirs(path_ptr) → i64       // string[]
//   Directory_exists(path_ptr) → i64          // bool
//   Directory_count(path_ptr) → i64           // int
//   Directory_copy(src_ptr, dst_ptr) → void
//   Directory_move(src_ptr, dst_ptr) → void
//   Directory_infos(path_ptr) → i64           // map
//
// Gestion d'erreurs : Les fonctions lèvent DirectoryException en cas d'erreur.
//
// Codes d'erreur DirectoryException :
//   101 - CREATE            : Erreur de création de répertoire
//   102 - CREATE_RECURSIVE  : Erreur de création récursive
//   103 - REMOVE            : Erreur de suppression de répertoire
//   104 - REMOVE_RECURSIVE  : Erreur de suppression récursive
//   105 - LIST              : Erreur de listage du contenu
//   106 - LIST_FILES        : Erreur de listage des fichiers
//   107 - LIST_DIRS         : Erreur de listage des sous-répertoires
//   108 - COUNT             : Erreur de comptage des entrées
//   109 - COPY              : Erreur de copie de répertoire
//   110 - MOVE              : Erreur de déplacement/renommage
//   111 - INFOS             : Erreur de lecture des métadonnées
// ─────────────────────────────────────────────────────────────────────────────

use std::fs;
use std::path::Path;
use crate::{alloc_str, ptr_to_str};
use crate::exception::throw_directory_exception;

// Codes d'erreur DirectoryException
const ERR_CREATE: i64 = 101;
const ERR_CREATE_RECURSIVE: i64 = 102;
const ERR_REMOVE: i64 = 103;
const ERR_REMOVE_RECURSIVE: i64 = 104;
const ERR_LIST: i64 = 105;
const ERR_LIST_FILES: i64 = 106;
const ERR_LIST_DIRS: i64 = 107;
const ERR_COUNT: i64 = 108;
const ERR_COPY: i64 = 109;
const ERR_MOVE: i64 = 110;
const ERR_INFOS: i64 = 111;

/// Directory::create(path:string) → void
/// Crée un répertoire (échoue si parent n'existe pas).
#[no_mangle]
pub unsafe extern "C" fn Directory_create(path_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    if let Err(e) = fs::create_dir(&path) {
        throw_directory_exception(
            &format!("Failed to create directory '{}': {}", path, e),
            ERR_CREATE,
            "Directory"
        );
    }
}

/// Directory::create_recursive(path:string) → void
/// Crée un répertoire et tous ses parents (équivalent mkdir -p).
#[no_mangle]
pub unsafe extern "C" fn Directory_create_recursive(path_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    if let Err(e) = fs::create_dir_all(&path) {
        throw_directory_exception(
            &format!("Failed to create directory recursively '{}': {}", path, e),
            ERR_CREATE_RECURSIVE,
            "Directory"
        );
    }
}

/// Directory::remove(path:string) → void
/// Supprime un répertoire vide.
#[no_mangle]
pub unsafe extern "C" fn Directory_remove(path_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    if let Err(e) = fs::remove_dir(&path) {
        throw_directory_exception(
            &format!("Failed to remove directory '{}': {}", path, e),
            ERR_REMOVE,
            "Directory"
        );
    }
}

/// Directory::remove_recursive(path:string) → void
/// Supprime un répertoire et tout son contenu (équivalent rm -rf).
#[no_mangle]
pub unsafe extern "C" fn Directory_remove_recursive(path_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    if let Err(e) = fs::remove_dir_all(&path) {
        throw_directory_exception(
            &format!("Failed to remove directory recursively '{}': {}", path, e),
            ERR_REMOVE_RECURSIVE,
            "Directory"
        );
    }
}

/// Directory::list(path:string) → string[]
/// Liste tous les fichiers et répertoires d'un répertoire.
#[no_mangle]
pub unsafe extern "C" fn Directory_list(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    match fs::read_dir(&path) {
        Ok(entries) => {
            let arr_ptr = crate::__array_new();
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Some(name) = entry.file_name().to_str() {
                        let name_ptr = alloc_str(name);
                        crate::__array_push(arr_ptr, name_ptr);
                    }
                }
            }
            arr_ptr
        }
        Err(e) => {
            throw_directory_exception(
                &format!("Failed to list directory '{}': {}", path, e),
                ERR_LIST,
                "Directory"
            );
        }
    }
}

/// Directory::list_files(path:string) → string[]
/// Liste uniquement les fichiers d'un répertoire.
#[no_mangle]
pub unsafe extern "C" fn Directory_list_files(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    match fs::read_dir(&path) {
        Ok(entries) => {
            let arr_ptr = crate::__array_new();
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_file() {
                            if let Some(name) = entry.file_name().to_str() {
                                let name_ptr = alloc_str(name);
                                crate::__array_push(arr_ptr, name_ptr);
                            }
                        }
                    }
                }
            }
            arr_ptr
        }
        Err(e) => {
            throw_directory_exception(
                &format!("Failed to list files in directory '{}': {}", path, e),
                ERR_LIST_FILES,
                "Directory"
            );
        }
    }
}

/// Directory::list_dirs(path:string) → string[]
/// Liste uniquement les sous-répertoires d'un répertoire.
#[no_mangle]
pub unsafe extern "C" fn Directory_list_dirs(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    match fs::read_dir(&path) {
        Ok(entries) => {
            let arr_ptr = crate::__array_new();
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Ok(meta) = entry.metadata() {
                        if meta.is_dir() {
                            if let Some(name) = entry.file_name().to_str() {
                                let name_ptr = alloc_str(name);
                                crate::__array_push(arr_ptr, name_ptr);
                            }
                        }
                    }
                }
            }
            arr_ptr
        }
        Err(e) => {
            throw_directory_exception(
                &format!("Failed to list subdirectories in directory '{}': {}", path, e),
                ERR_LIST_DIRS,
                "Directory"
            );
        }
    }
}

/// Directory::exists(path:string) → bool
/// Teste si un répertoire existe.
#[no_mangle]
pub unsafe extern "C" fn Directory_exists(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    Path::new(&path).is_dir() as i64
}

/// Directory::count(path:string) → int
/// Compte le nombre d'entrées dans un répertoire.
#[no_mangle]
pub unsafe extern "C" fn Directory_count(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    match fs::read_dir(&path) {
        Ok(entries) => entries.count() as i64,
        Err(e) => {
            throw_directory_exception(
                &format!("Failed to count entries in directory '{}': {}", path, e),
                ERR_COUNT,
                "Directory"
            );
        }
    }
}

/// Directory::copy(src:string, dst:string) → void
/// Copie un répertoire et tout son contenu (récursif).
#[no_mangle]
pub unsafe extern "C" fn Directory_copy(src_ptr: i64, dst_ptr: i64) {
    let src = ptr_to_str(src_ptr).to_string();
    let dst = ptr_to_str(dst_ptr).to_string();
    
    if let Err(e) = copy_dir_recursive(&src, &dst) {
        throw_directory_exception(
            &format!("Failed to copy directory from '{}' to '{}': {}", src, dst, e),
            ERR_COPY,
            "Directory"
        );
    }
}

/// Helper fonction récursive pour copier un répertoire
fn copy_dir_recursive(src: &str, dst: &str) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = Path::new(dst).join(entry.file_name());
        
        if src_path.is_dir() {
            copy_dir_recursive(
                src_path.to_str().unwrap(),
                dst_path.to_str().unwrap()
            )?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    
    Ok(())
}

/// Directory::move(src:string, dst:string) → void
/// Déplace/renomme un répertoire.
#[no_mangle]
pub unsafe extern "C" fn Directory_move(src_ptr: i64, dst_ptr: i64) {
    let src = ptr_to_str(src_ptr).to_string();
    let dst = ptr_to_str(dst_ptr).to_string();
    if let Err(e) = fs::rename(&src, &dst) {
        throw_directory_exception(
            &format!("Failed to move directory from '{}' to '{}': {}", src, dst, e),
            ERR_MOVE,
            "Directory"
        );
    }
}

/// Directory::infos(path:string) → map<string, mixed>
/// Retourne les métadonnées d'un répertoire.
#[no_mangle]
pub unsafe extern "C" fn Directory_infos(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    
    match fs::metadata(&path) {
        Ok(meta) => {
            let map_ptr = crate::__map_new();
            
            // modified: string (timestamp)
            if let Ok(modified) = meta.modified() {
                if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                    let timestamp = duration.as_secs();
                    let mod_key = alloc_str("modified");
                    let mod_val = alloc_str(&timestamp.to_string());
                    crate::__map_set(map_ptr, mod_key, mod_val);
                }
            }
            
            // created: string (timestamp)
            if let Ok(created) = meta.created() {
                if let Ok(duration) = created.duration_since(std::time::UNIX_EPOCH) {
                    let timestamp = duration.as_secs();
                    let cre_key = alloc_str("created");
                    let cre_val = alloc_str(&timestamp.to_string());
                    crate::__map_set(map_ptr, cre_key, cre_val);
                }
            }
            
            // is_dir: bool
            let is_dir_key = alloc_str("is_dir");
            crate::__map_set(map_ptr, is_dir_key, meta.is_dir() as i64);
            
            // count: int (nombre d'entrées)
            if let Ok(entries) = fs::read_dir(&path) {
                let count = entries.count() as i64;
                let count_key = alloc_str("count");
                crate::__map_set(map_ptr, count_key, count);
            }
            
            map_ptr
        }
        Err(e) => {
            throw_directory_exception(
                &format!("Failed to get metadata for directory '{}': {}", path, e),
                ERR_INFOS,
                "Directory"
            );
        }
    }
}
