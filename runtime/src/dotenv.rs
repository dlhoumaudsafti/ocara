// ─────────────────────────────────────────────────────────────────────────────
// runtime/src/dotenv.rs — Implémentation DotEnv
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use std::fs;
use std::sync::Mutex;
use once_cell::sync::Lazy;

// Stockage global des variables d'environnement chargées depuis .env
static ENV_VARS: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

/// Charge un fichier .env dans le stockage global
/// Si env_ptr est null (0) ou pointe vers une chaîne vide, charge ".env"
/// Sinon charge ".env.{env}"
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DotEnv_load(env_ptr: i64) {
    unsafe {
        let filename = if env_ptr == 0 {
            ".env".to_string()
        } else {
            let env_str = std::ffi::CStr::from_ptr(env_ptr as *const i8)
                .to_string_lossy()
                .to_string();
            
            if env_str.is_empty() {
                ".env".to_string()
            } else {
                format!(".env.{}", env_str)
            }
        };

        load_env_file(&filename);
    }
}

/// Surcharge de load() sans paramètre - charge ".env" par défaut
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DotEnv_load_0() {
    load_env_file(".env");
}

/// Fonction interne pour charger un fichier .env
fn load_env_file(filename: &str) {
    // Lire le fichier
    let content = match fs::read_to_string(filename) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: Failed to load {}: {}", filename, e);
            return;
        }
    };

    // Parser le fichier .env
    let mut env_vars = ENV_VARS.lock().unwrap();
    
    for line in content.lines() {
        let line = line.trim();
        
        // Ignorer les lignes vides et les commentaires
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parser KEY=VALUE
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let mut value = line[eq_pos + 1..].trim().to_string();

            // Retirer les guillemets si présents
            if (value.starts_with('"') && value.ends_with('"')) ||
               (value.starts_with('\'') && value.ends_with('\'')) {
                value = value[1..value.len() - 1].to_string();
            }

            // Charger dans l'environnement système aussi
            unsafe {
                std::env::set_var(&key, &value);
            }
            
            // Stocker dans notre HashMap
            env_vars.insert(key, value);
        }
    }
}

/// Récupère une variable d'environnement
/// Retourne un pointeur vers la valeur (string) ou 0 (null) si non trouvée
#[unsafe(no_mangle)]
pub unsafe extern "C" fn DotEnv_get(key_ptr: i64) -> i64 {
    unsafe {
        if key_ptr == 0 {
            return 0;
        }

        let key = std::ffi::CStr::from_ptr(key_ptr as *const i8)
            .to_string_lossy()
            .to_string();

        // D'abord chercher dans notre HashMap
        let env_vars = ENV_VARS.lock().unwrap();
        if let Some(value) = env_vars.get(&key) {
            let c_str = std::ffi::CString::new(value.as_str()).unwrap();
            return c_str.into_raw() as i64;
        }

        // Sinon chercher dans l'environnement système
        if let Ok(value) = std::env::var(&key) {
            let c_str = std::ffi::CString::new(value.as_str()).unwrap();
            return c_str.into_raw() as i64;
        }

        // Non trouvé
        0
    }
}
