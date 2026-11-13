//! `{% jsonpath '{"a":1}', '$.a' %}`: extracts a value from JSON.
//!
//! The original uses `jsonpath-plus` and returns only the **first** match
//! (`common/templating/local-template-tags.ts:288-328`). Heric implements the
//! subset of JSONPath a request template actually writes: a `$` root, dotted
//! names, bracketed names and indices, and a `[*]` wildcard. A filter
//! expression (`?(@.x > 1)`), a recursive descent (`..`) and a slice are
//! reported as unsupported rather than silently matching nothing, because a
//! template that gets an empty string back cannot tell a typo from a miss.

use serde_json::Value;

use crate::tag::TagError;
use crate::truthy::to_output;

/// Runs the tag.
///
/// # Errors
///
/// Returns [`TagError::BadArguments`] when the JSON does not parse, the query is
/// malformed or unsupported, or nothing matches.
pub(super) fn run(args: &[Value]) -> Result<Value, TagError> {
    let Some(json) = args.first().map(to_output) else {
        return Err(TagError::BadArguments {
            tag: "jsonpath",
            detail: "needs a JSON string".to_owned(),
        });
    };
    let Some(query) = args.get(1).map(to_output) else {
        return Err(TagError::BadArguments {
            tag: "jsonpath",
            detail: "needs a JSONPath query".to_owned(),
        });
    };
    let document: Value = serde_json::from_str(&json).map_err(|error| TagError::BadArguments {
        tag: "jsonpath",
        detail: format!("invalid JSON: {error}"),
    })?;
    let steps = parse(&query).map_err(|detail| TagError::BadArguments {
        tag: "jsonpath",
        detail: format!("invalid JSONPath query: {detail}"),
    })?;
    let mut current = vec![&document];
    for step in &steps {
        let mut next = Vec::new();
        for value in current {
            step.apply(value, &mut next);
        }
        current = next;
    }
    current
        .first()
        .map(|value| (*value).clone())
        .ok_or_else(|| TagError::BadArguments {
            tag: "jsonpath",
            detail: format!("JSONPath query returned no results: {query}"),
        })
}

/// One step of a query.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Step {
    /// A named member.
    Key(String),
    /// An array index.
    Index(usize),
    /// Every member of an array or object.
    Wildcard,
}

impl Step {
    /// Applies the step to one value, pushing every match.
    fn apply<'a>(&self, value: &'a Value, out: &mut Vec<&'a Value>) {
        match self {
            Self::Key(key) => {
                if let Some(found) = value.get(key) {
                    out.push(found);
                }
            }
            Self::Index(index) => {
                if let Some(found) = value.get(*index) {
                    out.push(found);
                }
            }
            Self::Wildcard => match value {
                Value::Array(items) => out.extend(items.iter()),
                Value::Object(map) => out.extend(map.values()),
                _ => {}
            },
        }
    }
}

/// Parses a query into steps.
///
/// # Errors
///
/// Returns a description of what is wrong, for the tag's error message.
fn parse(query: &str) -> Result<Vec<Step>, String> {
    let mut characters = query.chars().peekable();
    match characters.next() {
        Some('$') => {}
        Some(other) => return Err(format!("a query starts with `$`, not `{other}`")),
        None => return Err("the query is empty".to_owned()),
    }
    let mut steps = Vec::new();
    while let Some(character) = characters.next() {
        match character {
            '.' => {
                if characters.peek() == Some(&'.') {
                    return Err("recursive descent (`..`) is not supported".to_owned());
                }
                let mut name = String::new();
                while let Some(next) = characters.peek() {
                    if *next == '.' || *next == '[' {
                        break;
                    }
                    name.push(*next);
                    characters.next();
                }
                if name.is_empty() {
                    return Err("a `.` needs a name after it".to_owned());
                }
                steps.push(Step::Key(name));
            }
            '[' => {
                let mut inner = String::new();
                let mut closed = false;
                for next in characters.by_ref() {
                    if next == ']' {
                        closed = true;
                        break;
                    }
                    inner.push(next);
                }
                if !closed {
                    return Err("a `[` needs a `]`".to_owned());
                }
                steps.push(bracket(&inner)?);
            }
            other => return Err(format!("unexpected `{other}`")),
        }
    }
    Ok(steps)
}

/// Parses the inside of a bracket.
fn bracket(inner: &str) -> Result<Step, String> {
    let inner = inner.trim();
    if inner == "*" {
        return Ok(Step::Wildcard);
    }
    if let Some(quoted) = inner
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .or_else(|| {
            inner
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
        })
    {
        return Ok(Step::Key(quoted.to_owned()));
    }
    if inner.contains(':') {
        return Err("a slice is not supported".to_owned());
    }
    if inner.starts_with('?') {
        return Err("a filter expression is not supported".to_owned());
    }
    inner
        .parse::<usize>()
        .map(Step::Index)
        .map_err(|_| format!("`{inner}` is not an index or a quoted name"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn query(document: &str, path: &str) -> Result<String, TagError> {
        run(&[json!(document), json!(path)]).map(|value| to_output(&value))
    }

    #[test]
    fn a_dotted_path_reaches_a_member() -> Result<(), TagError> {
        assert_eq!(query(r#"{"a":{"b":"c"}}"#, "$.a.b")?, "c");
        Ok(())
    }

    #[test]
    fn a_bracketed_name_reaches_a_hyphenated_member() -> Result<(), TagError> {
        assert_eq!(query(r#"{"a-b":1}"#, "$['a-b']")?, "1");
        assert_eq!(query(r#"{"a-b":1}"#, r#"$["a-b"]"#)?, "1");
        Ok(())
    }

    #[test]
    fn an_index_reaches_an_array_element() -> Result<(), TagError> {
        assert_eq!(query(r#"{"xs":[10,20,30]}"#, "$.xs[1]")?, "20");
        Ok(())
    }

    #[test]
    fn a_wildcard_returns_the_first_match() -> Result<(), TagError> {
        // The original returns only the first result.
        assert_eq!(query(r#"{"xs":[10,20]}"#, "$.xs[*]")?, "10");
        Ok(())
    }

    #[test]
    fn a_non_string_result_prints_as_json() -> Result<(), TagError> {
        assert_eq!(query(r#"{"a":{"b":1}}"#, "$.a")?, "[object Object]");
        assert_eq!(query(r#"{"a":[1,2]}"#, "$.a")?, "12");
        assert_eq!(query(r#"{"a":true}"#, "$.a")?, "true");
        Ok(())
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(matches!(
            query("{", "$.a"),
            Err(TagError::BadArguments { detail, .. }) if detail.contains("invalid JSON")
        ));
    }

    #[test]
    fn a_miss_is_an_error() {
        assert!(matches!(
            query(r#"{"a":1}"#, "$.b"),
            Err(TagError::BadArguments { detail, .. }) if detail.contains("no results")
        ));
    }

    #[test]
    fn an_unsupported_construct_is_named() {
        for path in ["$..a", "$.a[?(@.b)]", "$.a[0:2]"] {
            assert!(
                matches!(
                    query(r#"{"a":1}"#, path),
                    Err(TagError::BadArguments { detail, .. }) if detail.contains("not supported")
                ),
                "{path} should be refused"
            );
        }
    }

    #[test]
    fn a_query_without_a_root_is_an_error() {
        assert!(query(r#"{"a":1}"#, "a.b").is_err());
    }
}
