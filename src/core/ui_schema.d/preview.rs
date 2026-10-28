// Aperçu d'un `.oc.ui` (`ocara x.oc.ui --preview`) : page HTML autonome qui
// affiche l'arbre avec le moteur de rendu desktop d'`ocara.UI`
// (runtime_tauri/src/ui/), celui de la fenêtre Tauri. Les événements
// s'affichent dans un journal en bas de page au lieu d'aller à Ocara.

use std::path::{Path, PathBuf};

use super::UiSpec;

const ENGINE_JS: &str = include_str!("../../../runtime_tauri/src/ui/engine.js");
const ENGINE_CSS: &str = include_str!("../../../runtime_tauri/src/ui/engine.css");

const PREVIEW_CSS: &str = "#oc-log{position:fixed;right:8px;bottom:8px;max-width:420px;max-height:40vh;overflow:auto;\
margin:0;padding:8px 10px;font:12px/1.4 monospace;background:rgba(0,0,0,.78);color:#e0e0e0;border-radius:8px;white-space:pre-wrap}\
#oc-log:empty{display:none}";

const PREVIEW_BOOT: &str = r#"(function () {
  var log = document.getElementById("oc-log");
  OcaraUI.start(TREE, {
    platform: "tauri",
    send: function (message) {
      console.log("ocara.UI", JSON.stringify(message));
      log.textContent = (JSON.stringify(message) + "\n" + log.textContent).split("\n").slice(0, 12).join("\n");
    },
  });
})();"#;

/// Texte JSON sûr dans un `<script>` (`</script>` ne peut pas le fermer).
fn script_json(spec: &UiSpec) -> String {
    spec.tree.to_string().replace("</", "<\\/")
}

pub fn page(spec: &UiSpec) -> String {
    format!(
        "<!doctype html>\n<html lang=\"fr\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>Aperçu ocara.UI</title>\n\
<style>\n{ENGINE_CSS}\n{PREVIEW_CSS}\n</style>\n</head>\n<body>\n<div id=\"oc-root\"></div>\n<pre id=\"oc-log\"></pre>\n\
<script>\n{ENGINE_JS}\n</script>\n<script>\nvar TREE = {};\n{PREVIEW_BOOT}\n</script>\n</body>\n</html>\n",
        script_json(spec),
    )
}

/// `-o` s'il est donné, sinon `<nom>.preview.html` dans le répertoire
/// temporaire (jamais à côté du `.oc.ui`).
pub fn output_path(input: &Path, output: &Path) -> PathBuf {
    if output != Path::new("out") {
        return output.to_path_buf();
    }
    let name = input.file_name().map(|n| n.to_string_lossy().trim_end_matches(".oc.ui").to_string()).unwrap_or_default();
    std::env::temp_dir().join(format!("{name}.preview.html"))
}
