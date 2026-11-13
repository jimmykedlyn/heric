//! JavaScript truthiness and value printing, which is what the original
//! follows.
//!
//! Liquid's own rules are Ruby's, where only `nil` and `false` are falsy and
//! even `0` and `""` are truthy. The original configures `liquidjs` with
//! `jsTruthy: true` (`common/templating/liquid-engine.ts:35`), so a false value
//! is one of `""`, `0`, `null`, `false` and `undefined`, while `[]` and `{}` are
//! truthy. Getting this backwards silently flips every `{% if %}` in every
//! existing template, which is why it has its own tests.
//!
//! Printing follows the same engine. Its `stringify` joins an array's elements
//! with no separator and prints an object as `[object Object]`, which is not
//! what JSON would give; a template that interpolates a container gets that
//! spelling in the original, so it gets it here too.

use serde_json::Value;

/// Whether a value counts as true in a condition.
///
/// `NaN` is not representable in `serde_json`, so the JavaScript rule that
/// `NaN` is falsy has no case here.
#[must_use]
pub fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|float| float != 0.0),
        Value::String(text) => !text.is_empty(),
        // JavaScript treats every object and array as truthy, empty or not.
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// Renders a value the way a template would print it.
///
/// `null` becomes the empty string, which is the original's rule for every tag
/// result and every interpolated value (`TPL-014`). An array prints its
/// elements back to back — `[1, 2]` is `12` and `["a", "b"]` is `ab` — and an
/// object prints as `[object Object]`, both as `liquidjs` does.
#[must_use]
pub fn to_output(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number_output(number),
        Value::String(text) => text.clone(),
        Value::Array(items) => items.iter().map(to_output).collect(),
        Value::Object(_) => "[object Object]".to_owned(),
    }
}

/// A number the way JavaScript would print it.
///
/// An integer that fits in `i64` prints exactly, which keeps a large identifier
/// from being mangled the way JavaScript's double would mangle it. Anything else
/// prints through `f64`'s `Display`, which drops a trailing `.0` — so JSON's
/// `1.0` renders as `1`, as the original does.
///
/// One divergence remains and is deliberate: JavaScript switches to exponential
/// notation outside `1e-7 … 1e21`, and `f64`'s `Display` never does. A template
/// interpolating a number that large gets a long decimal instead of `1e+21`.
#[must_use]
fn number_output(number: &serde_json::Number) -> String {
    if let Some(integer) = number.as_i64() {
        return integer.to_string();
    }
    match number.as_f64() {
        Some(float) => float.to_string(),
        // Unreachable for a `serde_json::Number`, which is always one of these.
        None => number.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn an_empty_array_and_an_empty_object_are_truthy() {
        assert!(is_truthy(&json!([])));
        assert!(is_truthy(&json!({})));
    }

    #[test]
    fn the_javascript_falsy_values_are_falsy() {
        for falsy in [json!(""), json!(0), json!(0.0), Value::Null, json!(false)] {
            assert!(!is_truthy(&falsy), "{falsy} should be falsy");
        }
    }

    #[test]
    fn everything_else_is_truthy() {
        for truthy in [json!("x"), json!(1), json!(-1), json!(0.5), json!(true)] {
            assert!(is_truthy(&truthy), "{truthy} should be truthy");
        }
    }

    #[test]
    fn null_prints_as_the_empty_string() {
        assert_eq!(to_output(&Value::Null), "");
    }

    #[test]
    fn scalars_print_without_quotes() {
        assert_eq!(to_output(&json!("text")), "text");
        assert_eq!(to_output(&json!(true)), "true");
        assert_eq!(to_output(&json!(false)), "false");
        assert_eq!(to_output(&json!(12)), "12");
        assert_eq!(to_output(&json!(1.5)), "1.5");
        assert_eq!(to_output(&json!(-0.5)), "-0.5");
    }

    #[test]
    fn a_float_with_no_fraction_prints_as_an_integer() {
        // JavaScript has one number type, so JSON's `1.0` reaches a template as
        // `1`. Verified against `liquidjs` 10 with `jsTruthy: true`.
        assert_eq!(to_output(&json!(1.0)), "1");
        assert_eq!(to_output(&json!(0.0)), "0");
        assert_eq!(to_output(&json!(2.0)), "2");
    }

    #[test]
    fn a_large_integer_keeps_all_its_digits() {
        // JavaScript would print `9007199254740992` here, having lost the last
        // digit to a double. Heric keeps the value it was given.
        assert_eq!(
            to_output(&json!(9_007_199_254_740_993_u64)),
            "9007199254740993"
        );
    }

    #[test]
    fn an_array_joins_its_elements_with_no_separator() {
        assert_eq!(to_output(&json!(["a", "b"])), "ab");
        assert_eq!(to_output(&json!([1, 2])), "12");
        // Nested arrays flatten through the recursion.
        assert_eq!(to_output(&json!([[1, 2], [3]])), "123");
        assert_eq!(to_output(&json!([1, Value::Null, "b"])), "1b");
        assert_eq!(to_output(&json!([])), "");
    }

    #[test]
    fn an_object_prints_as_object_object() {
        assert_eq!(to_output(&json!({"a": 1})), "[object Object]");
        assert_eq!(to_output(&json!({})), "[object Object]");
        // Including one nested inside an array.
        assert_eq!(to_output(&json!([{"a": 1}])), "[object Object]");
    }
}
