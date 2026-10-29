// Images embarquées : le `src` local d'un nœud `image` (relatif au `.oc.ui`)
// est lu à la compilation et remplacé par une URI `data:` ; le binaire (APK
// compris) n'a aucun fichier à livrer à côté. Une URL `http(s)://` ou une URI
// `data:` reste telle quelle.

use std::path::Path;

use serde_json::{Map, Value as Json};

use super::yaml::Node;
use super::Checker;

/// Au-delà, un avertissement serait utile ; la limite protège le binaire
/// d'une image de plusieurs Mo embarquée par erreur.
const MAX_BYTES: usize = 4 * 1024 * 1024;

fn mime(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => return None,
    })
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

impl Checker<'_> {
    /// Remplace le `src` local de l'image `props` par son contenu.
    pub(super) fn embed_image(&mut self, node: &Node, props: &mut Map<String, Json>) {
        let Some(src) = props.get("src").and_then(Json::as_str).map(str::to_string) else { return };
        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
            return;
        }
        let path = self.file.parent().unwrap_or(Path::new("")).join(&src);
        let Some(mime) = mime(&path) else {
            self.fail::<()>(node, format!("'image' : format de '{src}' non reconnu (png, jpg, gif, webp, svg)"));
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) if bytes.len() > MAX_BYTES => {
                self.fail::<()>(node, format!("'image' : '{src}' dépasse 4 Mo ({} octets)", bytes.len()));
            }
            Ok(bytes) => {
                props.insert("src".into(), Json::String(format!("data:{mime};base64,{}", base64(&bytes))));
            }
            Err(e) => {
                self.fail::<()>(node, format!("'image' : lecture de '{}' impossible : {e}", path.display()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_rfc4648() {
        assert_eq!(super::base64(b""), "");
        assert_eq!(super::base64(b"f"), "Zg==");
        assert_eq!(super::base64(b"fo"), "Zm8=");
        assert_eq!(super::base64(b"foo"), "Zm9v");
        assert_eq!(super::base64(b"foobar"), "Zm9vYmFy");
    }
}
