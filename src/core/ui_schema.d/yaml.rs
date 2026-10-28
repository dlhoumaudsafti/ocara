// Lecture d'un `.oc.ui` en arbre YAML positionné. `serde_yaml::Value` perd
// les positions : les événements de libyaml (`unsafe-libyaml`, le parseur de
// `serde_yaml`) donnent la ligne et la colonne de chaque nœud, reprises dans
// les erreurs du schéma.

use std::mem::MaybeUninit;

use unsafe_libyaml as sys;

/// Nœud YAML et sa position (ligne et colonne à partir de 1).
#[derive(Debug, Clone)]
pub struct Node {
    pub kind: Kind,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub enum Kind {
    /// `plain` : scalaire sans guillemets, dont le texte peut être un booléen,
    /// un nombre ou `null` (résolu par le schéma).
    Scalar { text: String, plain: bool },
    Seq(Vec<Node>),
    Map(Vec<(Node, Node)>),
}

/// Erreur de lecture : ligne, colonne, message.
pub type YamlError = (usize, usize, String);

enum Frame {
    Seq(Vec<Node>, usize, usize),
    Map(Vec<(Node, Node)>, Option<Node>, usize, usize),
}

fn position(mark: sys::yaml_mark_t) -> (usize, usize) {
    (mark.line as usize + 1, mark.column as usize + 1)
}

/// Libère le parseur libyaml en sortie de `load`, erreur comprise.
struct Parser(MaybeUninit<sys::yaml_parser_t>);

impl Drop for Parser {
    fn drop(&mut self) {
        unsafe { sys::yaml_parser_delete(self.0.as_mut_ptr()) };
    }
}

/// Premier (et seul) document de `text` ; `None` pour un texte vide.
pub fn load(text: &str) -> Result<Option<Node>, YamlError> {
    let mut parser = Parser(MaybeUninit::uninit());
    let raw = parser.0.as_mut_ptr();
    unsafe {
        if sys::yaml_parser_initialize(raw).fail {
            return Err((1, 1, "lecteur YAML : initialisation impossible".to_string()));
        }
        sys::yaml_parser_set_input_string(raw, text.as_ptr(), text.len() as u64);
    }
    let mut stack: Vec<Frame> = Vec::new();
    let mut root: Option<Node> = None;
    let mut documents = 0;
    loop {
        let mut event = MaybeUninit::<sys::yaml_event_t>::uninit();
        if unsafe { sys::yaml_parser_parse(raw, event.as_mut_ptr()) }.fail {
            return Err(parse_error(unsafe { &*raw }));
        }
        let mut event = unsafe { event.assume_init() };
        let (line, col) = position(event.start_mark);
        let mut done = None;
        let mut stop = false;
        match event.type_ {
            sys::YAML_STREAM_END_EVENT => stop = true,
            sys::YAML_DOCUMENT_START_EVENT => {
                documents += 1;
                if documents > 1 {
                    unsafe { sys::yaml_event_delete(&mut event) };
                    return Err((line, col, "un seul document YAML attendu (séparateur '---' en trop)".to_string()));
                }
            }
            sys::YAML_ALIAS_EVENT => {
                unsafe { sys::yaml_event_delete(&mut event) };
                return Err((line, col, "les alias YAML (*nom) ne sont pas pris en charge".to_string()));
            }
            sys::YAML_SCALAR_EVENT => {
                let scalar = unsafe { event.data.scalar };
                let bytes = unsafe { std::slice::from_raw_parts(scalar.value, scalar.length as usize) };
                let text = String::from_utf8_lossy(bytes).into_owned();
                let plain = scalar.style == sys::YAML_PLAIN_SCALAR_STYLE;
                done = Some(Node { kind: Kind::Scalar { text, plain }, line, col });
            }
            sys::YAML_SEQUENCE_START_EVENT => stack.push(Frame::Seq(Vec::new(), line, col)),
            sys::YAML_MAPPING_START_EVENT => stack.push(Frame::Map(Vec::new(), None, line, col)),
            sys::YAML_SEQUENCE_END_EVENT | sys::YAML_MAPPING_END_EVENT => {
                done = match stack.pop() {
                    Some(Frame::Seq(items, line, col)) => Some(Node { kind: Kind::Seq(items), line, col }),
                    Some(Frame::Map(pairs, _, line, col)) => Some(Node { kind: Kind::Map(pairs), line, col }),
                    None => None,
                };
            }
            _ => {}
        }
        unsafe { sys::yaml_event_delete(&mut event) };
        if let Some(node) = done {
            match stack.last_mut() {
                Some(Frame::Seq(items, ..)) => items.push(node),
                Some(Frame::Map(pairs, pending, ..)) => match pending.take() {
                    None => *pending = Some(node),
                    Some(key) => pairs.push((key, node)),
                },
                None => root = Some(node),
            }
        }
        if stop {
            return Ok(root);
        }
    }
}

fn parse_error(parser: &sys::yaml_parser_t) -> YamlError {
    let text = |ptr: *const std::ffi::c_char| {
        if ptr.is_null() { None } else { Some(unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned()) }
    };
    let (line, col) = position(parser.problem_mark);
    let problem = text(parser.problem).unwrap_or_else(|| "erreur de lecture".to_string());
    let message = match text(parser.context) {
        Some(context) => format!("YAML invalide : {problem} ({context})"),
        None => format!("YAML invalide : {problem}"),
    };
    (line, col, message)
}
