// ─────────────────────────────────────────────────────────────────────────────
// ocara.File — Opérations sur fichiers
//
// Fonctions exportées (convention C) :
//
//   File_read(path_ptr) → i64                      // Lit contenu texte UTF-8
//   File_read_bytes(path_ptr) → i64                // Lit contenu binaire (int[])
//   File_write(path_ptr, content_ptr) → void       // Écrit (écrase)
//   File_write_bytes(path_ptr, data_ptr) → void    // Écrit binaire
//   File_append(path_ptr, content_ptr) → void      // Ajoute à la fin
//   File_exists(path_ptr) → i64                    // Test existence (bool)
//   File_size(path_ptr) → i64                      // Taille en octets
//   File_extension(path_ptr) → i64                 // Extension sans le point
//   File_remove(path_ptr) → void                   // Supprime fichier
//   File_copy(src_ptr, dst_ptr) → void             // Copie fichier
//   File_move(src_ptr, dst_ptr) → void             // Déplace fichier
//   File_infos(path_ptr) → i64                     // Métadonnées (map)
//
// Gestion d'erreurs : Les fonctions lèvent FileException en cas d'erreur.
//
// Codes d'erreur FileException :
//   101 - READ         : Erreur de lecture texte
//   102 - READ_BYTES   : Erreur de lecture binaire
//   103 - WRITE        : Erreur d'écriture texte
//   104 - WRITE_BYTES  : Erreur d'écriture binaire
//   105 - APPEND       : Erreur d'ajout de contenu
//   106 - SIZE         : Erreur de lecture de taille
//   107 - REMOVE       : Erreur de suppression
//   108 - COPY         : Erreur de copie
//   109 - MOVE         : Erreur de déplacement/renommage
//   110 - INFOS        : Erreur de lecture des métadonnées
// ─────────────────────────────────────────────────────────────────────────────

use std::fs;
use std::io::Write;
use std::path::Path;
use crate::{alloc_str, ptr_to_str};
use crate::exception::throw_file_exception;

// Codes d'erreur FileException
const ERR_READ: i64 = 101;
const ERR_READ_BYTES: i64 = 102;
const ERR_WRITE: i64 = 103;
const ERR_WRITE_BYTES: i64 = 104;
const ERR_APPEND: i64 = 105;
const ERR_SIZE: i64 = 106;
const ERR_REMOVE: i64 = 107;
const ERR_COPY: i64 = 108;
const ERR_MOVE: i64 = 109;
const ERR_INFOS: i64 = 110;

/// File::read(path:string) → string
/// Lit le contenu d'un fichier en UTF-8.
#[no_mangle]
pub unsafe extern "C" fn File_read(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    match fs::read_to_string(&path) {
        Ok(content) => alloc_str(&content),
        Err(e) => throw_file_exception(
            &format!("Failed to read file '{}': {}", path, e),
            ERR_READ,
            "File"
        ),
    }
}

/// File::read_bytes(path:string) → int[]
/// Lit le contenu d'un fichier en binaire (array d'octets).
#[no_mangle]
pub unsafe extern "C" fn File_read_bytes(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    match fs::read(&path) {
        Ok(bytes) => {
            // Créer un array Ocara d'entiers
            let arr_ptr = crate::__array_new();
            for byte in bytes {
                crate::__array_push(arr_ptr, byte as i64);
            }
            arr_ptr
        }
        Err(e) => {
            throw_file_exception(
                &format!("Failed to read binary file '{}': {}", path, e),
                ERR_READ_BYTES,
                "File"
            );
        }
    }
}

/// File::write(path:string, content:string) → void
/// Écrit du contenu texte dans un fichier (écrase si existe).
#[no_mangle]
pub unsafe extern "C" fn File_write(path_ptr: i64, content_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    let content = ptr_to_str(content_ptr).to_string();
    if let Err(e) = fs::write(&path, &content) {
        throw_file_exception(
            &format!("Failed to write file '{}': {}", path, e),
            ERR_WRITE,
            "File"
        );
    }
}

/// File::write_bytes(path:string, data:int[]) → void
/// Écrit des données binaires dans un fichier.
#[no_mangle]
pub unsafe extern "C" fn File_write_bytes(path_ptr: i64, data_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    
    // Lire l'array Ocara et convertir en Vec<u8>
    let len = crate::__array_len(data_ptr) as usize;
    let mut bytes = Vec::with_capacity(len);
    for i in 0..len {
        let val = crate::__array_get(data_ptr, i as i64);
        bytes.push(val as u8);
    }
    
    if let Err(e) = fs::write(&path, &bytes) {
        throw_file_exception(
            &format!("Failed to write binary file '{}': {}", path, e),
            ERR_WRITE_BYTES,
            "File"
        );
    }
}

/// File::append(path:string, content:string) → void
/// Ajoute du contenu à la fin d'un fichier (crée si n'existe pas).
#[no_mangle]
pub unsafe extern "C" fn File_append(path_ptr: i64, content_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    let content = ptr_to_str(content_ptr).to_string();
    
    match fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Err(e) = file.write_all(content.as_bytes()) {
                throw_file_exception(
                    &format!("Failed to append to file '{}': {}", path, e),
                    ERR_APPEND,
                    "File"
                );
            }
        }
        Err(e) => {
            throw_file_exception(
                &format!("Failed to open file '{}' for appending: {}", path, e),
                ERR_APPEND,
                "File"
            );
        }
    }
}

/// File::exists(path:string) → bool
/// Teste si un fichier existe.
#[no_mangle]
pub unsafe extern "C" fn File_exists(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    Path::new(&path).is_file() as i64
}

/// File::size(path:string) → int
/// Retourne la taille d'un fichier en octets.
#[no_mangle]
pub unsafe extern "C" fn File_size(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    match fs::metadata(&path) {
        Ok(meta) => meta.len() as i64,
        Err(e) => {
            throw_file_exception(
                &format!("Failed to get size of file '{}': {}", path, e),
                ERR_SIZE,
                "File"
            );
        }
    }
}

/// File::extension(path:string) → string
/// Retourne l'extension du fichier sans le point ("txt", "json", "").
#[no_mangle]
pub unsafe extern "C" fn File_extension(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    let p = Path::new(&path);
    match p.extension() {
        Some(ext) => alloc_str(ext.to_str().unwrap_or("")),
        None => alloc_str(""),
    }
}

/// File::remove(path:string) → void
/// Supprime un fichier.
#[no_mangle]
pub unsafe extern "C" fn File_remove(path_ptr: i64) {
    let path = ptr_to_str(path_ptr).to_string();
    if let Err(e) = fs::remove_file(&path) {
        throw_file_exception(
            &format!("Failed to remove file '{}': {}", path, e),
            ERR_REMOVE,
            "File"
        );
    }
}

/// File::copy(src:string, dst:string) → void
/// Copie un fichier.
#[no_mangle]
pub unsafe extern "C" fn File_copy(src_ptr: i64, dst_ptr: i64) {
    let src = ptr_to_str(src_ptr).to_string();
    let dst = ptr_to_str(dst_ptr).to_string();
    if let Err(e) = fs::copy(&src, &dst) {
        throw_file_exception(
            &format!("Failed to copy file from '{}' to '{}': {}", src, dst, e),
            ERR_COPY,
            "File"
        );
    }
}

/// File::move(src:string, dst:string) → void
/// Déplace/renomme un fichier.
#[no_mangle]
pub unsafe extern "C" fn File_move(src_ptr: i64, dst_ptr: i64) {
    let src = ptr_to_str(src_ptr).to_string();
    let dst = ptr_to_str(dst_ptr).to_string();
    if let Err(e) = fs::rename(&src, &dst) {
        throw_file_exception(
            &format!("Failed to move file from '{}' to '{}': {}", src, dst, e),
            ERR_MOVE,
            "File"
        );
    }
}

/// File::infos(path:string) → map<string, mixed>
/// Retourne les métadonnées d'un fichier.
#[no_mangle]
pub unsafe extern "C" fn File_infos(path_ptr: i64) -> i64 {
    let path = ptr_to_str(path_ptr).to_string();
    let p = Path::new(&path);
    
    match fs::metadata(&path) {
        Ok(meta) => {
            let map_ptr = crate::__map_new();
            
            // size: int
            let size_key = alloc_str("size");
            crate::__map_set(map_ptr, size_key, meta.len() as i64);
            
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
            
            // is_file: bool
            let is_file_key = alloc_str("is_file");
            crate::__map_set(map_ptr, is_file_key, meta.is_file() as i64);
            
            // is_dir: bool
            let is_dir_key = alloc_str("is_dir");
            crate::__map_set(map_ptr, is_dir_key, meta.is_dir() as i64);
            
            // extension: string
            if let Some(ext) = p.extension() {
                let ext_key = alloc_str("extension");
                let ext_val = alloc_str(ext.to_str().unwrap_or(""));
                crate::__map_set(map_ptr, ext_key, ext_val);
            } else {
                let ext_key = alloc_str("extension");
                let ext_val = alloc_str("");
                crate::__map_set(map_ptr, ext_key, ext_val);
            }
            
            map_ptr
        }
        Err(e) => {
            throw_file_exception(
                &format!("Failed to get metadata for file '{}': {}", path, e),
                ERR_INFOS,
                "File"
            );
        }
    }
}
