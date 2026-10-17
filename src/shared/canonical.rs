//! JSON Canonicalization Scheme (JCS, RFC 8785).
//!
//! The one canonical form for comparing or hashing JSON by value, for example
//! the `arguments` of an event subscription, whose identity must not depend
//! on how a client ordered keys or spelled numbers. Object members are sorted
//! by their UTF-16 code units, numbers take the ECMAScript shortest
//! round-trip form (`1.0`, `1` and `1e0` are all `1`), strings use the minimal
//! JSON escapes, and there is no whitespace.
//!
//! Integers beyond 2^53 lose precision, as RFC 8785 requires (it serializes
//! every number as an IEEE 754 double).

use std::fmt;

use serde::Serialize;
use serde_json::Value;

/// Why a value has no canonical form: its `Serialize` impl failed, or it holds
/// a map with non-string keys or a non-finite float.
#[derive(Debug)]
pub struct CanonicalJsonError(serde_json::Error);

impl fmt::Display for CanonicalJsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value has no canonical JSON form: {}", self.0)
    }
}

impl std::error::Error for CanonicalJsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// The canonical UTF-8 bytes of any serializable value.
pub fn to_canonical_vec<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalJsonError> {
    serde_json_canonicalizer::to_vec(value).map_err(CanonicalJsonError)
}

/// The canonical text of any serializable value.
pub fn to_canonical_string<T: Serialize>(value: &T) -> Result<String, CanonicalJsonError> {
    serde_json_canonicalizer::to_string(value).map_err(CanonicalJsonError)
}

/// The canonical text of a JSON document given as text.
pub fn canonicalize_str(json: &str) -> Result<String, CanonicalJsonError> {
    serde_json_canonicalizer::pipe(json).map_err(CanonicalJsonError)
}

/// The canonical text of a [`Value`]. Infallible: a `Value` has only string
/// keys and finite numbers.
pub fn canonical_value_string(value: &Value) -> String {
    to_canonical_string(value).expect("a serde_json::Value always has a canonical form")
}

/// The canonical bytes of a [`Value`]. Infallible, as for
/// [`canonical_value_string`].
pub fn canonical_value_bytes(value: &Value) -> Vec<u8> {
    canonical_value_string(value).into_bytes()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// RFC 8785 section 3.2.2, with the input's line breaks.
    #[test]
    fn rfc_8785_section_3_2_2_example() {
        let input = "{\r\n  \"numbers\": [333333333.33333329, 1E30, 4.50,\r\n              2e-3, 0.000000000000000000000000001],\r\n  \"string\": \"\\u20ac$\\u000F\\u000aA'\\u0042\\u0022\\u005c\\\\\\\"\\/\",\r\n  \"literals\": [null, true, false]\r\n}";
        let expected = "{\"literals\":[null,true,false],\"numbers\":[333333333.3333333,1e+30,4.5,0.002,1e-27],\"string\":\"\u{20ac}$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"}";
        assert_eq!(canonicalize_str(input).unwrap(), expected);
    }

    /// The same example as the bytes RFC 8785 section 3.2.4 lists.
    #[test]
    fn rfc_8785_section_3_2_4_utf8_bytes() {
        let input = r#"{"numbers":[333333333.33333329,1E30,4.50,2e-3,0.000000000000000000000000001],"string":"\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/","literals":[null,true,false]}"#;
        let expected: Vec<u8> = "7b 22 6c 69 74 65 72 61 6c 73 22 3a 5b 6e 75 6c 6c 2c 74 72 \
             75 65 2c 66 61 6c 73 65 5d 2c 22 6e 75 6d 62 65 72 73 22 3a \
             5b 33 33 33 33 33 33 33 33 33 2e 33 33 33 33 33 33 33 2c 31 \
             65 2b 33 30 2c 34 2e 35 2c 30 2e 30 30 32 2c 31 65 2d 32 37 \
             5d 2c 22 73 74 72 69 6e 67 22 3a 22 e2 82 ac 24 5c 75 30 30 \
             30 66 5c 6e 41 27 42 5c 22 5c 5c 5c 5c 5c 22 2f 22 7d"
            .split_whitespace()
            .map(|h| u8::from_str_radix(h, 16).unwrap())
            .collect();
        let value: Value = serde_json::from_str(input).unwrap();
        assert_eq!(canonical_value_bytes(&value), expected);
    }

    /// RFC 8785 section 3.2.3: members sort by UTF-16 code units, so the
    /// emoji (a surrogate pair starting 0xD83D) sorts before U+FB33.
    #[test]
    fn rfc_8785_section_3_2_3_sorting() {
        let input = "{\"\\u20ac\": \"Euro Sign\", \"\\r\": \"Carriage Return\", \"\\ufb33\": \"Hebrew Letter Dalet With Dagesh\", \"1\": \"One\", \"\\ud83d\\ude00\": \"Emoji: Grinning Face\", \"\\u0080\": \"Control\", \"\\u00f6\": \"Latin Small Letter O With Diaeresis\"}";
        let expected = "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\"\u{f6}\":\"Latin Small Letter O With Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\"\u{1f600}\":\"Emoji: Grinning Face\",\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}";
        assert_eq!(canonicalize_str(input).unwrap(), expected);
    }

    /// RFC 8785 appendix B: IEEE 754 doubles and their canonical text.
    #[test]
    fn rfc_8785_appendix_b_numbers() {
        let cases: &[(u64, &str)] = &[
            (0x0000000000000000, "0"),
            (0x8000000000000000, "0"),
            (0x0000000000000001, "5e-324"),
            (0x8000000000000001, "-5e-324"),
            (0x7fefffffffffffff, "1.7976931348623157e+308"),
            (0xffefffffffffffff, "-1.7976931348623157e+308"),
            (0x4340000000000000, "9007199254740992"),
            (0xc340000000000000, "-9007199254740992"),
            (0x4430000000000000, "295147905179352830000"),
            (0x44b52d02c7e14af5, "9.999999999999997e+22"),
            (0x44b52d02c7e14af6, "1e+23"),
            (0x44b52d02c7e14af7, "1.0000000000000001e+23"),
            (0x444b1ae4d6e2ef4e, "999999999999999700000"),
            (0x444b1ae4d6e2ef4f, "999999999999999900000"),
            (0x444b1ae4d6e2ef50, "1e+21"),
            (0x3eb0c6f7a0b5ed8c, "9.999999999999997e-7"),
            (0x3eb0c6f7a0b5ed8d, "0.000001"),
            (0x41b3de4355555553, "333333333.3333332"),
            (0x41b3de4355555554, "333333333.33333325"),
            (0x41b3de4355555555, "333333333.3333333"),
            (0x41b3de4355555556, "333333333.3333334"),
            (0x41b3de4355555557, "333333333.33333343"),
            (0xbecbf647612f3696, "-0.0000033333333333333333"),
            (0x43143ff3c1cb0959, "1424953923781206.2"),
        ];
        for (bits, text) in cases {
            let n = serde_json::Number::from_f64(f64::from_bits(*bits)).unwrap();
            assert_eq!(
                canonical_value_string(&Value::Number(n)),
                *text,
                "{bits:016x}"
            );
        }
    }

    #[test]
    fn non_finite_numbers_have_no_json_value() {
        for bits in [
            0x7fffffffffffffff_u64,
            0x7ff0000000000000,
            0xfff0000000000000,
        ] {
            assert!(serde_json::Number::from_f64(f64::from_bits(bits)).is_none());
        }
        assert!(to_canonical_string(&f64::NAN).is_err());
    }

    #[test]
    fn key_order_and_number_spelling_do_not_change_the_form() {
        let a: Value = serde_json::from_str(r#"{"a":1.0,"b":2}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        let c: Value = serde_json::from_str(r#"{ "b" : 2e0 , "a" : 10E-1 }"#).unwrap();
        assert_eq!(canonical_value_string(&a), r#"{"a":1,"b":2}"#);
        assert_eq!(canonical_value_string(&a), canonical_value_string(&b));
        assert_eq!(canonical_value_string(&a), canonical_value_string(&c));
    }

    #[test]
    fn every_insertion_order_of_nested_keys_gives_one_form() {
        let keys = [
            "zeta",
            "alpha",
            "Mid",
            "\u{e9}t\u{e9}",
            "10",
            "1",
            "\u{1f600}",
            "\u{fb33}",
        ];
        let mut canonical = std::collections::BTreeSet::new();
        let mut plain = std::collections::BTreeSet::new();
        for rotation in 0..keys.len() {
            for reverse in [false, true] {
                let mut order: Vec<&str> = keys.to_vec();
                order.rotate_left(rotation);
                if reverse {
                    order.reverse();
                }
                let mut inner = serde_json::Map::new();
                for k in &order {
                    inner.insert((*k).to_owned(), json!({"k": k, "n": k.len()}));
                }
                let mut outer = serde_json::Map::new();
                for k in order.iter().rev() {
                    outer.insert((*k).to_owned(), Value::Object(inner.clone()));
                }
                let value = Value::Object(outer);
                plain.insert(serde_json::to_string(&value).unwrap());
                canonical.insert(canonical_value_string(&value));
            }
        }
        assert!(
            plain.len() > 1,
            "insertion order must be visible without JCS"
        );
        assert_eq!(canonical.len(), 1, "{canonical:?}");
    }

    #[test]
    fn arrays_keep_their_order() {
        assert_ne!(
            canonical_value_string(&json!([1, 2])),
            canonical_value_string(&json!([2, 1]))
        );
    }

    #[test]
    fn typed_values_canonicalize_like_their_json() {
        #[derive(Serialize)]
        struct Args {
            region: &'static str,
            limit: u32,
        }
        assert_eq!(
            to_canonical_string(&Args {
                region: "eu",
                limit: 10,
            })
            .unwrap(),
            r#"{"limit":10,"region":"eu"}"#
        );
        assert_eq!(
            to_canonical_vec(&json!({"b": [], "a": {}})).unwrap(),
            br#"{"a":{},"b":[]}"#
        );
    }

    #[test]
    fn malformed_text_is_an_error() {
        assert!(canonicalize_str("{").is_err());
        assert!(canonicalize_str(r#"{"a":1,}"#).is_err());
    }
}
