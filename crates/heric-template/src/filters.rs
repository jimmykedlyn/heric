//! The filters a template may apply.
//!
//! The original registers exactly one filter of its own — `debug`, an identity
//! function kept for Nunjucks compatibility (`common/templating/liquid-engine.ts:58`)
//! — and everything else is a stock `liquidjs` filter. `strictFilters` is on, so
//! a name outside this table is an error rather than a passthrough: a typo in a
//! template should say so instead of silently rendering the input unchanged.
//!
//! Only the filters the original's own conformance suite pins are implemented
//! here (`templating/__tests__/liquid-compat.test.ts:74-102`). The rest of
//! `liquidjs`'s catalogue is not ported, and a template that uses one gets the
//! same "unknown filter" error a typo would.

use serde_json::Value;

use crate::error::RenderError;
use crate::truthy::{is_truthy, to_output};

/// Applies a filter to a value.
///
/// # Errors
///
/// Returns [`RenderError::Evaluation`] when the filter is unknown or its
/// arguments are wrong. The position is left at `0:0` because a filter does not
/// know where it was written; the evaluator fills it in.
pub fn apply(name: &str, value: &Value, args: &[Value]) -> Result<Value, RenderError> {
    match name {
        // Registered by the original for Nunjucks back-compat; it does nothing.
        "debug" => Ok(value.clone()),
        "upcase" => Ok(Value::String(to_output(value).to_uppercase())),
        "downcase" => Ok(Value::String(to_output(value).to_lowercase())),
        "size" => Ok(Value::from(size(value))),
        "default" => Ok(default(value, args.first())),
        "replace" => replace(value, args),
        other => Err(unknown(other)),
    }
}

/// The names [`apply`] accepts, for the docs and for a test that pins the list.
#[must_use]
pub fn names() -> &'static [&'static str] {
    &["debug", "default", "downcase", "replace", "size", "upcase"]
}

/// `liquidjs`'s `size`: the length of a string or array, and `0` for anything
/// else, including a number and an object.
///
/// A string's length is counted in UTF-16 code units, as JavaScript's
/// `String.length` does, so an astral character counts as two.
fn size(value: &Value) -> usize {
    match value {
        Value::String(text) => text.encode_utf16().count(),
        Value::Array(items) => items.len(),
        _ => 0,
    }
}

/// `liquidjs`'s `default`: the fallback when the value is empty or falsy.
///
/// An empty string and an empty array fall back even though an array is
/// otherwise truthy, because the filter checks `length` before truthiness. An
/// empty object does *not* fall back: it has no `length`, and JavaScript treats
/// it as truthy.
fn default(value: &Value, fallback: Option<&Value>) -> Value {
    let empty = match value {
        Value::String(text) => text.is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    };
    if empty || !is_truthy(value) {
        return fallback.cloned().unwrap_or(Value::Null);
    }
    value.clone()
}

/// `liquidjs`'s `replace`: every occurrence of the pattern, with both sides
/// stringified first.
fn replace(value: &Value, args: &[Value]) -> Result<Value, RenderError> {
    let (Some(pattern), Some(replacement)) = (args.first(), args.get(1)) else {
        return Err(RenderError::Evaluation {
            path: String::new(),
            line: 0,
            column: 0,
            message: "`replace` needs a pattern and a replacement".to_owned(),
        });
    };
    let text = to_output(value);
    let pattern = to_output(pattern);
    let replacement = to_output(replacement);
    // JavaScript's `split('')` splits into characters, so an empty pattern
    // inserts the replacement between every pair rather than at the ends.
    if pattern.is_empty() {
        let mut out = String::new();
        for (index, character) in text.chars().enumerate() {
            if index > 0 {
                out.push_str(&replacement);
            }
            out.push(character);
        }
        return Ok(Value::String(out));
    }
    Ok(Value::String(text.replace(&pattern, &replacement)))
}

/// The error an unknown filter produces.
fn unknown(name: &str) -> RenderError {
    RenderError::Evaluation {
        path: String::new(),
        line: 0,
        column: 0,
        message: format!("unknown filter `{name}`"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(name: &str, value: Value, args: &[Value]) -> Result<String, RenderError> {
        apply(name, &value, args).map(|value| to_output(&value))
    }

    #[test]
    fn the_filter_list_is_sorted_and_unique() {
        let mut names = names().to_vec();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn debug_passes_the_value_through() -> Result<(), RenderError> {
        assert_eq!(run("debug", json!("abc"), &[])?, "abc");
        assert_eq!(run("debug", json!([1, 2]), &[])?, "12");
        Ok(())
    }

    #[test]
    fn upcase_and_downcase_stringify_first() -> Result<(), RenderError> {
        assert_eq!(run("upcase", json!("hello"), &[])?, "HELLO");
        assert_eq!(run("downcase", json!("HELLO"), &[])?, "hello");
        // A number stringifies, and an object prints as `[object Object]`.
        assert_eq!(run("upcase", json!(5), &[])?, "5");
        assert_eq!(run("upcase", json!({"a": 1}), &[])?, "[OBJECT OBJECT]");
        assert_eq!(run("upcase", Value::Null, &[])?, "");
        Ok(())
    }

    #[test]
    fn size_counts_a_string_or_an_array_and_zeroes_the_rest() -> Result<(), RenderError> {
        assert_eq!(run("size", json!("hello"), &[])?, "5");
        assert_eq!(run("size", json!([1, 2, 3]), &[])?, "3");
        // A number and an object have no `length` in JavaScript.
        assert_eq!(run("size", json!(5), &[])?, "0");
        assert_eq!(run("size", json!({"a": 1}), &[])?, "0");
        assert_eq!(run("size", Value::Null, &[])?, "0");
        // An astral character is two UTF-16 code units, as in JavaScript.
        assert_eq!(run("size", json!("😀"), &[])?, "2");
        Ok(())
    }

    #[test]
    fn default_falls_back_for_an_empty_or_falsy_value() -> Result<(), RenderError> {
        let fallback = json!("fb");
        for empty in [json!(""), json!([]), json!(0), json!(false), Value::Null] {
            assert_eq!(
                run("default", empty.clone(), std::slice::from_ref(&fallback))?,
                "fb",
                "{empty}"
            );
        }
        // An empty object is truthy and has no `length`, so it survives.
        assert_eq!(
            run("default", json!({}), std::slice::from_ref(&fallback))?,
            "[object Object]"
        );
        assert_eq!(run("default", json!("v"), &[fallback])?, "v");
        Ok(())
    }

    #[test]
    fn default_without_a_fallback_renders_nothing() -> Result<(), RenderError> {
        assert_eq!(run("default", json!(""), &[])?, "");
        Ok(())
    }

    #[test]
    fn replace_substitutes_every_occurrence() -> Result<(), RenderError> {
        assert_eq!(
            run("replace", json!("abc"), &[json!("a"), json!("b")])?,
            "bbc"
        );
        assert_eq!(
            run("replace", json!("aaa"), &[json!("a"), json!("b")])?,
            "bbb"
        );
        // Both sides stringify, so a numeric pattern matches the digits.
        assert_eq!(run("replace", json!(111), &[json!(1), json!(2)])?, "222");
        // An empty pattern inserts between characters, as JavaScript's
        // `split('')` does.
        assert_eq!(
            run("replace", json!("ab"), &[json!(""), json!("-")])?,
            "a-b"
        );
        Ok(())
    }

    #[test]
    fn replace_without_both_arguments_is_an_error() {
        assert!(apply("replace", &json!("a"), &[json!("a")]).is_err());
        assert!(apply("replace", &json!("a"), &[]).is_err());
    }

    #[test]
    fn an_unknown_filter_is_an_error() {
        let error = apply("nope", &json!("a"), &[]);
        assert!(matches!(
            error,
            Err(RenderError::Evaluation { message, .. }) if message.contains("unknown filter")
        ));
    }
}
