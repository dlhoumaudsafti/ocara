// `JSON::encode`/`JSON::decode` sans arbre `serde_json::Value` : même texte
// et mêmes valeurs qu'avec lui (json_codec.rs).

use crate::json_codec::{decode, encode};
use crate::*;

/// Texte décodé puis réencodé (`None` : décodage invalide, `null`).
fn roundtrip(text: &str) -> Option<String> {
    let val = decode(text);
    if val == 0 {
        return None;
    }
    let out = encode(val, 0);
    let result = unsafe { ptr_to_str(out) }.to_string();
    unsafe {
        rc::release(out);
        rc::release(val);
    }
    Some(result)
}

/// Même texte que `serde_json::Value` (clés d'objet triées).
fn by_value(text: &str) -> String {
    serde_json::to_string(&serde_json::from_str::<serde_json::Value>(text).unwrap()).unwrap()
}

#[test]
fn decode_then_encode_matches_serde_json_value() {
    for text in [
        r#"{"b": 1, "a": 2, "b": 3}"#,
        r#"{"10": 1, "2": 2, "1": 3, "x": {"z": [], "y": {}}}"#,
        r#"[9223372036854775807, -9223372036854775808, -5, 1e2, 0.5, -0.0]"#,
        r#"[true, false, null, "é\n\"\\", "😀", 0, 65536, 1152921504606846976]"#,
        r#"{"a": {"b": {"c": [1, {"d": "e"}]}}}"#,
        "  [ ]  ",
        "42",
        r#""texte""#,
    ] {
        assert_eq!(roundtrip(text).as_deref(), Some(by_value(text).as_str()), "{}", text);
    }
}

/// Au-delà de `i64::MAX`, un flottant (comme l'ancienne conversion depuis
/// `Value` : `as_i64` échoue, `as_f64` réussit).
#[test]
fn integers_beyond_i64_become_floats() {
    assert_eq!(roundtrip("[18446744073709551615]").as_deref(), Some("[1.8446744073709552e19]"));
}

#[test]
fn invalid_text_decodes_to_null() {
    for text in ["", "{", r#"{"a": [1, {"b": "x"}, 3], "c": tru}"#, "[1, 2] x", r#"{"a" 1}"#, "[1,]"] {
        assert_eq!(roundtrip(text), None, "{}", text);
    }
}
