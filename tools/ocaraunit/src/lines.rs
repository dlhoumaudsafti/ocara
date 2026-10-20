// ─────────────────────────────────────────────────────────────────────────────
// Couverture par ligne (`--line-coverage`) : chaque test est compilé avec
// `ocara --coverage` et écrit, à sa sortie, ses points (`<0|1>\t<ligne>\t<fichier>`)
// dans `$OCARA_COVERAGE_FILE` ; les points de tous les tests sont fusionnés
// (une ligne est couverte si un test l'a exécutée). Un fichier source qu'aucun
// test n'a compilé reçoit ses points de `ocara --coverage-points`, tous non
// couverts.
// ─────────────────────────────────────────────────────────────────────────────

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Points de couverture : fichier → ligne → exécutée.
#[derive(Debug, Default)]
pub struct Hits {
    pub files: BTreeMap<PathBuf, BTreeMap<usize, bool>>,
}

impl Hits {
    /// Fusionne un rapport `<0|1>\t<ligne>\t<fichier>` (chemins relatifs à `root`).
    pub fn add_report(&mut self, report: &str, root: &Path) {
        for line in report.lines() {
            let mut fields = line.splitn(3, '\t');
            let (Some(hit), Some(at), Some(file)) = (fields.next(), fields.next(), fields.next()) else { continue };
            let Ok(at) = at.parse::<usize>() else { continue };
            self.add(&normalize(file, root), at, hit == "1");
        }
    }

    /// Points d'un fichier jamais compilé par un test : `<ligne>\t<fichier>`.
    pub fn add_points(&mut self, points: &str, root: &Path) {
        for line in points.lines() {
            let Some((at, file)) = line.split_once('\t') else { continue };
            let Ok(at) = at.parse::<usize>() else { continue };
            self.add(&normalize(file, root), at, false);
        }
    }

    fn add(&mut self, file: &Path, line: usize, hit: bool) {
        let covered = self.files.entry(file.to_path_buf()).or_default().entry(line).or_insert(false);
        *covered |= hit;
    }
}

fn normalize(file: &str, root: &Path) -> PathBuf {
    let path = Path::new(file);
    let joined = if path.is_absolute() { path.to_path_buf() } else { root.join(path) };
    fs::canonicalize(&joined).unwrap_or(joined)
}

#[derive(Debug, PartialEq)]
pub struct FileLines {
    pub path:      PathBuf,
    pub covered:   usize,
    pub total:     usize,
    /// Plages de lignes non couvertes (1-basées, incluses) : lignes
    /// instrumentées consécutives jamais exécutées.
    pub uncovered: Vec<(usize, usize)>,
    /// Lignes instrumentées et leur exécution (fusion de plusieurs projets
    /// par l'extension VS Code).
    pub points:    Vec<(usize, bool)>,
}

impl FileLines {
    pub fn of(path: &Path, lines: &BTreeMap<usize, bool>) -> FileLines {
        let mut uncovered: Vec<(usize, usize)> = Vec::new();
        let mut open: Option<(usize, usize)> = None;
        for (&line, &hit) in lines {
            match (hit, open) {
                (false, Some((start, _))) => open = Some((start, line)),
                (false, None) => open = Some((line, line)),
                (true, Some(range)) => { uncovered.push(range); open = None; }
                (true, None) => {}
            }
        }
        uncovered.extend(open);
        FileLines {
            path: path.to_path_buf(),
            covered: lines.values().filter(|&&h| h).count(),
            total: lines.len(),
            uncovered,
            points: lines.iter().map(|(&l, &h)| (l, h)).collect(),
        }
    }

    pub fn percent(&self) -> f64 {
        percent(self.covered, self.total)
    }
}

pub fn percent(covered: usize, total: usize) -> f64 {
    if total == 0 { 100.0 } else { covered as f64 / total as f64 * 100.0 }
}

/// Fichier du périmètre dont les points n'ont pas pu être obtenus.
#[derive(Debug)]
pub struct Unanalyzed {
    pub path:  PathBuf,
    pub error: String,
}

pub struct Report<'a> {
    pub pass:       usize,
    pub fail:       usize,
    pub errors:     usize,
    pub files:      &'a [FileLines],
    pub unanalyzed: &'a [Unanalyzed],
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Rapport lisible par l'extension VS Code (`--json <fichier>`).
pub fn to_json(report: &Report) -> String {
    let covered: usize = report.files.iter().map(|f| f.covered).sum();
    let total: usize = report.files.iter().map(|f| f.total).sum();
    let files: Vec<String> = report.files.iter().map(|f| {
        let ranges: Vec<String> = f.uncovered.iter()
            .map(|(start, end)| format!("{{\"start\":{},\"end\":{}}}", start, end))
            .collect();
        let points: Vec<String> = f.points.iter().map(|(l, h)| format!("[{},{}]", l, u8::from(*h))).collect();
        format!("{{\"path\":{},\"covered\":{},\"total\":{},\"percent\":{:.2},\"uncovered\":[{}],\"points\":[{}]}}",
            json_string(&f.path.to_string_lossy()), f.covered, f.total, f.percent(), ranges.join(","), points.join(","))
    }).collect();
    let unanalyzed: Vec<String> = report.unanalyzed.iter()
        .map(|u| format!("{{\"path\":{},\"error\":{}}}", json_string(&u.path.to_string_lossy()), json_string(&u.error)))
        .collect();
    format!("{{\"tests\":{{\"pass\":{},\"fail\":{},\"errors\":{}}},\"coverage\":{{\"covered\":{},\"total\":{},\"percent\":{:.2},\"files\":[{}],\"unanalyzed\":[{}]}}}}\n",
        report.pass, report.fail, report.errors, covered, total, percent(covered, total), files.join(","), unanalyzed.join(","))
}
