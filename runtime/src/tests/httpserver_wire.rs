// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires — format sur le fil de ocara.HTTPServer (décodage URL,
// validation et fusion d'en-têtes, routage décodé) — voir
// docs/roadmap.d/stdlib-httpserver-robustesse.md.
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::httpserver::{match_route, parse_route_pattern, ParamValue};
use crate::httpserver_wire::{
    form_decode, insert_request_header, is_valid_header_name, is_valid_header_value, parse_query, percent_decode,
    split_target,
};

#[test]
fn percent_decode_reads_utf8_byte_sequences() {
    assert_eq!(percent_decode("h%C3%A9llo"), "héllo");
    assert_eq!(percent_decode("%41"), "A", "a sequence at the very end is decoded");
}

#[test]
fn percent_decode_keeps_plus_and_malformed_sequences() {
    assert_eq!(percent_decode("a+b"), "a+b");
    assert_eq!(percent_decode("100%"), "100%");
    assert_eq!(percent_decode("%zz%4"), "%zz%4");
}

#[test]
fn percent_decode_replaces_invalid_utf8() {
    assert_eq!(percent_decode("%FF"), "\u{FFFD}");
}

#[test]
fn form_decode_turns_plus_into_space() {
    assert_eq!(form_decode("caf%C3%A9+cr%C3%A8me"), "café crème");
    assert_eq!(form_decode("a%2Bb"), "a+b");
}

#[test]
fn parse_query_decodes_keys_and_values() {
    let q = parse_query("n%C3%A9=%C3%A9t%C3%A9&flag&x=1+2");
    assert_eq!(q.get("né").map(String::as_str), Some("été"));
    assert_eq!(q.get("flag").map(String::as_str), Some(""));
    assert_eq!(q.get("x").map(String::as_str), Some("1 2"));
}

#[test]
fn split_target_drops_fragment() {
    assert_eq!(split_target("/a?q=1#frag"), ("/a", "q=1"));
    assert_eq!(split_target("/a#frag?q=1"), ("/a", ""));
    assert_eq!(split_target("/a"), ("/a", ""));
}

#[test]
fn header_value_with_crlf_or_control_is_rejected() {
    assert!(is_valid_header_value("text/html; charset=utf-8"));
    assert!(is_valid_header_value("a\tb"));
    assert!(!is_valid_header_value("a\r\nX-Injected: yes"));
    assert!(!is_valid_header_value("a\nSet-Cookie: evil=1"));
    assert!(!is_valid_header_value("a\u{0}b"));
}

#[test]
fn header_name_must_be_a_token() {
    assert!(is_valid_header_name("X-Echo"));
    assert!(!is_valid_header_name(""));
    assert!(!is_valid_header_name("X Echo"));
    assert!(!is_valid_header_name("X:Echo"));
}

#[test]
fn repeated_headers_are_merged() {
    let mut headers = HashMap::new();
    insert_request_header(&mut headers, "Cookie", "a=1");
    insert_request_header(&mut headers, "cookie", "b=2");
    insert_request_header(&mut headers, "Accept", "text/html");
    insert_request_header(&mut headers, "Accept", "application/json");
    assert_eq!(headers.get("Cookie").map(String::as_str), Some("a=1; b=2"));
    assert_eq!(headers.get("Accept").map(String::as_str), Some("text/html, application/json"));
}

#[test]
fn route_segments_are_decoded_before_typing() {
    let ints = parse_route_pattern("/i/<id:int>");
    assert!(matches!(match_route(&ints, "/i/%31").unwrap().get("id"), Some(ParamValue::Int(1))));
    let strings = parse_route_pattern("/s/<x:string>");
    assert!(matches!(match_route(&strings, "/s/h%C3%A9llo").unwrap().get("x"), Some(ParamValue::Text(v)) if v == "héllo"));
    assert!(matches!(match_route(&strings, "/s/a+b").unwrap().get("x"), Some(ParamValue::Text(v)) if v == "a+b"));
    assert!(matches!(match_route(&strings, "/s/a%2Fb").unwrap().get("x"), Some(ParamValue::Text(v)) if v == "a/b"));
}

#[test]
fn float_route_param_rejects_nan_and_infinity() {
    let pattern = parse_route_pattern("/f/<x:float>");
    for raw in ["nan", "NaN", "inf", "-inf", "infinity"] {
        assert!(match_route(&pattern, &format!("/f/{}", raw)).is_none(), "{} must not match", raw);
    }
    assert!(match_route(&pattern, "/f/1.5").is_some());
}
