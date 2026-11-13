//! JavaScript truthiness, which is what the original follows.
//!
//! Liquid's own rules are Ruby's, where only `nil` and `false` are falsy and
//! even `0` and `""` are truthy. The original configures `liquidjs` to follow
//! JavaScript instead, so `""`, `0`, `null`, `false` and `undefined` are falsy
//! while `[]` and `{}` are truthy. Getting this backwards silently flips every
//! `{% if %}` in every existing template, which is why it is pinned by its own
//! tests.

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
/// result and every interpolated value (`TPL-014`). Objects and arrays print as
/// JSON, since there is no JavaScript `String()` to defer to.
#[must_use]
pub fn to_output(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
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
        assert_eq!(to_output(&json!(12)), "12");
        assert_eq!(to_output(&json!(1.5)), "1.5");
    }

    #[test]
    fn containers_print_as_json() {
        assert_eq!(to_output(&json!([1, 2])), "[1,2]");
        assert_eq!(to_output(&json!({"a": 1})), r#"{"a":1}"#);
    }
}
