// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires — ocara.HTTPServerRequest (headers/param/params, urlencoded
// et multipart) — voir docs/roadmap.d/stdlib-httpserver-request-object.md et
// docs/roadmap.d/stdlib-httpserver-post-body-parsing.md (les deux désormais
// clos par ce chantier).
//
// Ces tests exercent les fonctions PURES de `httpserver.rs` (aucune vraie
// connexion réseau/`tiny_http::Request` — pas d'équivalent "ouvrir une
// requête HTTP en mémoire" comme `open_memory_db()` pour SQLite) :
// `header_lookup`, `parse_boundary`, `parse_multipart`, `build_params_buckets`,
// `lookup_param`. Le chemin RÉEL bout-en-bout (vrai serveur, vraies requêtes
// `curl`) est couvert séparément par `examples/tests/59_httpserver_requestTest.oc`
// et son script `.sh`.
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::httpserver::{
    header_lookup, parse_boundary, parse_multipart, build_params_buckets, lookup_param,
    ParamValue, METHOD_BUCKETS,
};

// ── header_lookup ────────────────────────────────────────────────────────────

fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn header_lookup_matches_regardless_of_stored_or_queried_case() {
    let h = headers(&[("X-Custom-Header", "value1")]);
    assert_eq!(header_lookup(&h, "x-custom-header"), Some("value1"));
    assert_eq!(header_lookup(&h, "X-CUSTOM-HEADER"), Some("value1"));
    assert_eq!(header_lookup(&h, "X-Custom-Header"), Some("value1"));
}

#[test]
fn header_lookup_missing_header_is_none() {
    let h = headers(&[("Content-Type", "text/plain")]);
    assert_eq!(header_lookup(&h, "X-Absent"), None);
}

// ── parse_boundary ───────────────────────────────────────────────────────────

#[test]
fn parse_boundary_unquoted() {
    assert_eq!(
        parse_boundary("multipart/form-data; boundary=abc123"),
        Some("abc123".to_string())
    );
}

#[test]
fn parse_boundary_quoted_webkit_style() {
    // Forme réelle envoyée par la plupart des navigateurs — la valeur
    // contient des caractères hors du jeu "token" HTTP (tirets répétés),
    // toujours quotée en pratique même si RFC 2046 autoriserait la forme nue
    // pour CETTE valeur précise.
    assert_eq!(
        parse_boundary(r#"multipart/form-data; boundary="----WebKitFormBoundaryABC123""#),
        Some("----WebKitFormBoundaryABC123".to_string())
    );
}

#[test]
fn parse_boundary_case_insensitive_mime_type() {
    assert_eq!(
        parse_boundary("Multipart/Form-Data; boundary=xyz"),
        Some("xyz".to_string())
    );
}

#[test]
fn parse_boundary_none_for_non_multipart_content_type() {
    assert_eq!(parse_boundary("application/x-www-form-urlencoded"), None);
    assert_eq!(parse_boundary("application/json"), None);
    assert_eq!(parse_boundary("text/plain"), None);
}

#[test]
fn parse_boundary_none_when_boundary_param_missing() {
    assert_eq!(parse_boundary("multipart/form-data"), None);
    assert_eq!(parse_boundary("multipart/form-data; charset=utf-8"), None);
}

// ── parse_multipart ──────────────────────────────────────────────────────────

#[test]
fn parse_multipart_plain_field_crlf() {
    let body = b"--B\r\nContent-Disposition: form-data; name=\"field1\"\r\n\r\nhello\r\n--B--\r\n";
    let parts = parse_multipart(body, "B");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].name, "field1");
    assert_eq!(parts[0].filename, None);
    assert_eq!(parts[0].body, b"hello");
}

/// Tolérance explicitement demandée : un corps LF-seul (client non
/// conforme à RFC 7578, qui impose CRLF) doit être accepté à l'identique.
#[test]
fn parse_multipart_plain_field_lf_only() {
    let body = b"--B\nContent-Disposition: form-data; name=\"field1\"\n\nhello\n--B--\n";
    let parts = parse_multipart(body, "B");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].name, "field1");
    assert_eq!(parts[0].body, b"hello");
}

#[test]
fn parse_multipart_file_field_with_content_type() {
    let body = b"--B\r\nContent-Disposition: form-data; name=\"thefile\"; filename=\"a.txt\"\r\nContent-Type: text/plain\r\n\r\nfile content here\r\n--B--\r\n";
    let parts = parse_multipart(body, "B");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].name, "thefile");
    assert_eq!(parts[0].filename.as_deref(), Some("a.txt"));
    assert_eq!(parts[0].content_type.as_deref(), Some("text/plain"));
    assert_eq!(parts[0].body, b"file content here");
}

/// Un part fichier SANS son propre `Content-Type` (client minimal) — `None`
/// ici ; c'est `build_params_buckets` qui applique le défaut
/// "application/octet-stream", pas `parse_multipart` lui-même.
#[test]
fn parse_multipart_file_field_missing_content_type_is_none_not_defaulted() {
    let body = b"--B\r\nContent-Disposition: form-data; name=\"thefile\"; filename=\"a.bin\"\r\n\r\nRAW\r\n--B--\r\n";
    let parts = parse_multipart(body, "B");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].filename.as_deref(), Some("a.bin"));
    assert_eq!(parts[0].content_type, None);
}

#[test]
fn parse_multipart_binary_content_is_preserved_byte_for_byte() {
    // Contenu volontairement NON-UTF8 (0xFF, 0x00, 0xFE...) — un vrai fichier
    // binaire (JPEG/PNG/PDF) ressemble à ça. `parse_multipart` doit préserver
    // les octets exacts, jamais les interpréter comme du texte.
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(b"--B\r\nContent-Disposition: form-data; name=\"f\"; filename=\"x.bin\"\r\nContent-Type: application/octet-stream\r\n\r\n");
    let raw_bytes: [u8; 6] = [0xFF, 0x00, 0xFE, 0xD8, 0x01, 0x02];
    body.extend_from_slice(&raw_bytes);
    body.extend_from_slice(b"\r\n--B--\r\n");

    let parts = parse_multipart(&body, "B");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].body, raw_bytes.to_vec());
}

#[test]
fn parse_multipart_multiple_parts() {
    let body = b"--B\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--B\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\n2\r\n--B--\r\n";
    let parts = parse_multipart(body, "B");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].name, "a");
    assert_eq!(parts[0].body, b"1");
    assert_eq!(parts[1].name, "b");
    assert_eq!(parts[1].body, b"2");
}

#[test]
fn parse_multipart_empty_body_returns_no_parts() {
    assert!(parse_multipart(b"", "B").is_empty());
}

/// Corps qui ne contient JAMAIS le boundary demandé (mauvais boundary
/// extrait, ou corps totalement étranger) — ne doit jamais planter, juste ne
/// trouver aucun part.
#[test]
fn parse_multipart_boundary_not_found_returns_no_parts() {
    let body = b"this is not a multipart body at all";
    assert!(parse_multipart(body, "B").is_empty());
}

/// Corps mal terminé (pas de délimiteur final `--B--`) — tolérant : le
/// dernier part va jusqu'à la fin du corps plutôt que de tout rejeter.
#[test]
fn parse_multipart_missing_final_boundary_is_tolerated() {
    let body = b"--B\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\nvalue-no-final-boundary";
    let parts = parse_multipart(body, "B");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].body, b"value-no-final-boundary");
}

/// Part sans `Content-Disposition`/`name` — malformé, ignoré plutôt que de
/// planter ou produire un part sans nom.
#[test]
fn parse_multipart_part_without_name_is_skipped() {
    let body = b"--B\r\nContent-Type: text/plain\r\n\r\nno name here\r\n--B--\r\n";
    assert!(parse_multipart(body, "B").is_empty());
}

// ── build_params_buckets ─────────────────────────────────────────────────────

#[test]
fn build_params_buckets_always_has_all_10_buckets() {
    let h = HashMap::new();
    let buckets = build_params_buckets("", "GET", &h, b"");
    assert_eq!(buckets.len(), METHOD_BUCKETS.len());
    for name in METHOD_BUCKETS {
        assert!(buckets.contains_key(name), "bucket manquant : {name}");
    }
}

#[test]
fn build_params_buckets_query_string_always_in_get_bucket() {
    let h = HashMap::new();
    // Même avec une méthode réelle POST, la query string reste dans "GET".
    let buckets = build_params_buckets("id=42&name=Alice", "POST", &h, b"");
    let get = &buckets["GET"];
    assert!(matches!(get.get("id"), Some(ParamValue::Text(v)) if v == "42"));
    assert!(matches!(get.get("name"), Some(ParamValue::Text(v)) if v == "Alice"));
}

#[test]
fn build_params_buckets_urlencoded_body_in_actual_method_bucket() {
    let h = headers(&[("Content-Type", "application/x-www-form-urlencoded")]);
    let buckets = build_params_buckets("", "POST", &h, b"name=Bob&age=30");
    let post = &buckets["POST"];
    assert!(matches!(post.get("name"), Some(ParamValue::Text(v)) if v == "Bob"));
    assert!(matches!(post.get("age"), Some(ParamValue::Text(v)) if v == "30"));
    // Les autres buckets restent vides.
    assert!(buckets["PUT"].is_empty());
    assert!(buckets["GET"].is_empty());
}

/// Cas rare explicitement spécifié : méthode réelle GET AVEC un corps —
/// fusionné dans le MÊME bucket "GET" que la query string, le corps
/// l'emportant en cas de collision.
#[test]
fn build_params_buckets_get_with_body_merges_into_get_bucket() {
    let h = headers(&[("Content-Type", "application/x-www-form-urlencoded")]);
    let buckets = build_params_buckets("shared=fromQuery&onlyQuery=q", "GET", &h, b"shared=fromBody&onlyBody=b");
    let get = &buckets["GET"];
    assert!(matches!(get.get("shared"), Some(ParamValue::Text(v)) if v == "fromBody"), "le corps doit l'emporter sur la query string à clé égale");
    assert!(matches!(get.get("onlyQuery"), Some(ParamValue::Text(v)) if v == "q"));
    assert!(matches!(get.get("onlyBody"), Some(ParamValue::Text(v)) if v == "b"));
}

#[test]
fn build_params_buckets_unrecognized_content_type_leaves_body_bucket_empty() {
    let h = headers(&[("Content-Type", "application/json")]);
    let buckets = build_params_buckets("", "POST", &h, b"{\"a\":1}");
    assert!(buckets["POST"].is_empty(), "JSON est hors périmètre ici (JSON::decode(req.body()) côté appelant)");
}

#[test]
fn build_params_buckets_empty_body_leaves_all_method_buckets_empty() {
    let h = headers(&[("Content-Type", "application/x-www-form-urlencoded")]);
    let buckets = build_params_buckets("q=1", "POST", &h, b"");
    assert!(buckets["POST"].is_empty());
    assert!(matches!(buckets["GET"].get("q"), Some(ParamValue::Text(v)) if v == "1"));
}

#[test]
fn build_params_buckets_multipart_file_field_has_expected_shape() {
    let h = headers(&[("Content-Type", "multipart/form-data; boundary=B")]);
    let body = b"--B\r\nContent-Disposition: form-data; name=\"thefile\"; filename=\"x.jpg\"\r\nContent-Type: image/jpeg\r\n\r\n\xff\xd8\xff\r\n--B--\r\n";
    let buckets = build_params_buckets("", "POST", &h, body);
    match buckets["POST"].get("thefile") {
        Some(ParamValue::File { filename, content_type, content }) => {
            assert_eq!(filename, "x.jpg");
            assert_eq!(content_type, "image/jpeg");
            assert_eq!(content, &vec![0xffu8, 0xd8, 0xff]);
        }
        Some(ParamValue::Text(_)) => panic!("expected a File value, got a Text value"),
        None => panic!("expected a File value, got None"),
    }
}

#[test]
fn build_params_buckets_multipart_file_field_defaults_content_type_when_absent() {
    let h = headers(&[("Content-Type", "multipart/form-data; boundary=B")]);
    let body = b"--B\r\nContent-Disposition: form-data; name=\"f\"; filename=\"noext\"\r\n\r\ndata\r\n--B--\r\n";
    let buckets = build_params_buckets("", "POST", &h, body);
    match buckets["POST"].get("f") {
        Some(ParamValue::File { content_type, .. }) => assert_eq!(content_type, "application/octet-stream"),
        _ => panic!("expected a File value"),
    }
}

// ── lookup_param ─────────────────────────────────────────────────────────────

fn text_bucket(pairs: &[(&str, &str)]) -> HashMap<String, ParamValue> {
    pairs.iter().map(|(k, v)| (k.to_string(), ParamValue::Text(v.to_string()))).collect()
}

#[test]
fn lookup_param_explicit_method_searches_only_that_bucket() {
    let mut buckets: HashMap<String, HashMap<String, ParamValue>> = HashMap::new();
    buckets.insert("GET".to_string(), text_bucket(&[("id", "fromGet")]));
    buckets.insert("POST".to_string(), text_bucket(&[("id", "fromPost")]));

    assert!(matches!(lookup_param(&buckets, "id", Some("GET"), "POST"), Some(ParamValue::Text(v)) if v == "fromGet"));
    assert!(matches!(lookup_param(&buckets, "id", Some("POST"), "GET"), Some(ParamValue::Text(v)) if v == "fromPost"));
    // Absent du bucket demandé même si présent ailleurs : pas de repli.
    assert!(lookup_param(&buckets, "id", Some("PUT"), "GET").is_none());
}

#[test]
fn lookup_param_default_precedence_actual_method_get_only_checks_get() {
    let mut buckets: HashMap<String, HashMap<String, ParamValue>> = HashMap::new();
    buckets.insert("GET".to_string(), text_bucket(&[("id", "fromGet")]));

    assert!(matches!(lookup_param(&buckets, "id", None, "GET"), Some(ParamValue::Text(v)) if v == "fromGet"));
}

#[test]
fn lookup_param_default_precedence_body_overrides_query_on_collision() {
    let mut buckets: HashMap<String, HashMap<String, ParamValue>> = HashMap::new();
    buckets.insert("GET".to_string(), text_bucket(&[("shared", "fromQuery"), ("onlyQuery", "q")]));
    buckets.insert("POST".to_string(), text_bucket(&[("shared", "fromBody"), ("onlyBody", "b")]));

    assert!(matches!(lookup_param(&buckets, "shared", None, "POST"), Some(ParamValue::Text(v)) if v == "fromBody"));
    // Clé présente uniquement côté query : repli sur GET.
    assert!(matches!(lookup_param(&buckets, "onlyQuery", None, "POST"), Some(ParamValue::Text(v)) if v == "q"));
    // Clé présente uniquement côté body : trouvée directement.
    assert!(matches!(lookup_param(&buckets, "onlyBody", None, "POST"), Some(ParamValue::Text(v)) if v == "b"));
}

#[test]
fn lookup_param_absent_key_is_none() {
    let buckets: HashMap<String, HashMap<String, ParamValue>> = METHOD_BUCKETS.iter()
        .map(|m| (m.to_string(), HashMap::new()))
        .collect();
    assert!(lookup_param(&buckets, "nope", None, "GET").is_none());
    assert!(lookup_param(&buckets, "nope", Some("POST"), "POST").is_none());
}
