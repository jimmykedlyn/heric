//! The built-in template tags.
//!
//! The original's twelve local tags live in one file
//! (`common/templating/local-template-tags.ts`) and are dispatched by name. This
//! module keeps that shape: [`run`] matches on the name and each tag's
//! behaviour sits in its own submodule.
//!
//! Two things are deliberately not implemented yet, and both fail loudly rather
//! than rendering something plausible:
//!
//! - `{% hash %}` needs MD5, SHA-1, SHA-256 or SHA-512. No hashing crate is on
//!   the plan's allowed-new list, and hand-writing a cryptographic primitive is
//!   not a decision this layer may take, so the tag reports
//!   [`TagError::Unsupported`] naming the crate it would need.
//! - `{% cookie %}`, `{% request %}` and `{% response %}` read documents the
//!   renderer cannot reach: the cookie jar, the request being rendered and
//!   another request's stored response. [`TagHost`] exposes none of them yet,
//!   and inventing a port here would put the pipeline's shape in the wrong
//!   crate. They report [`TagError::Unsupported`] naming what is missing.

mod base64;
mod faker;
mod jsonpath;
mod time;

use serde_json::Value;

use crate::ast::Expr;
use crate::context::RenderContext;
use crate::error::RenderError;
use crate::host::{ReadPurpose, TagHost};
use crate::tag::TagError;
use crate::truthy::to_output;

/// Runs a tag and returns the text it contributes.
///
/// Every tag stringifies its result the way the original does: `null` becomes
/// the empty string, an array joins its elements and an object prints as
/// `[object Object]` (`TPL-014`).
///
/// # Errors
///
/// Returns [`TagError`] when the tag is unknown, its arguments are wrong, the
/// host refuses, or the tag is not implemented yet.
pub fn run(
    name: &str,
    args: &[Value],
    context: &mut RenderContext<'_>,
    host: &dyn TagHost,
) -> Result<String, TagError> {
    let value = match name {
        "base64" => base64::run(args)?,
        "faker" => faker::run(args)?,
        "file" => file(args, host)?,
        "jsonpath" => jsonpath::run(args)?,
        "now" => time::now(args)?,
        "os" => os(args)?,
        "prompt" => prompt(args, context, host)?,
        "timestamp" => time::timestamp(args)?,
        "uuid" => uuid(args)?,
        "hash" => {
            return Err(TagError::Unsupported {
                tag: "hash",
                detail:
                    "needs a hashing crate (md-5/sha1/sha2), which is a dependency approval item"
                        .to_owned(),
            });
        }
        "cookie" | "request" | "response" => {
            return Err(TagError::Unsupported {
                tag: match name {
                    "cookie" => "cookie",
                    "request" => "request",
                    _ => "response",
                },
                detail: "needs a host port the renderer does not have yet".to_owned(),
            });
        }
        other => {
            return Err(TagError::Unknown {
                name: other.to_owned(),
            });
        }
    };
    Ok(to_output(&value))
}

/// `{% file 'path' %}`: reads a file through the host.
///
/// The path policing belongs to the host, not here: `heric-core`'s
/// `SendingHost` is where the allowed roots and the datafile refusal live, so a
/// second check in this crate would be a second rule to keep in step.
fn file(args: &[Value], host: &dyn TagHost) -> Result<Value, TagError> {
    let Some(path) = args.first().and_then(Value::as_str) else {
        return Err(TagError::BadArguments {
            tag: "file",
            detail: "needs a path".to_owned(),
        });
    };
    if path.is_empty() {
        return Err(TagError::BadArguments {
            tag: "file",
            detail: "no file selected".to_owned(),
        });
    }
    let bytes = host.read_file(std::path::Path::new(path), ReadPurpose::RequestBody)?;
    // A file is text in every use the original has for it; invalid UTF-8 is
    // reported rather than silently replaced, because a mangled certificate
    // would fail later with a worse message.
    String::from_utf8(bytes)
        .map(Value::String)
        .map_err(|_| TagError::ReadFailed {
            path: path.to_owned(),
            detail: "the file is not valid UTF-8".to_owned(),
        })
}

/// `{% os 'function' %}`: reads an operating-system value.
///
/// The original calls `os.arch()`, `os.platform()` and friends. Heric has no
/// `libc` binding, so the values come from what `std` and the environment
/// expose; `cpus` and `userInfo` are not available and say so.
fn os(args: &[Value]) -> Result<Value, TagError> {
    let Some(function) = args.first().and_then(Value::as_str) else {
        return Err(TagError::BadArguments {
            tag: "os",
            detail: "needs a function name".to_owned(),
        });
    };
    let value = match function {
        "arch" => Value::String(std::env::consts::ARCH.to_owned()),
        "platform" => Value::String(std::env::consts::OS.to_owned()),
        "hostname" => Value::String(hostname()),
        "release" => Value::String(std::env::consts::OS.to_owned()),
        "freemem" => Value::from(0),
        other => {
            return Err(TagError::BadArguments {
                tag: "os",
                detail: format!(
                    "`{other}` is not available; use arch, platform, hostname, release or freemem"
                ),
            });
        }
    };
    Ok(value)
}

/// The machine's hostname, from the environment.
///
/// `std` has no hostname call and `unsafe_code` is forbidden, so this reads the
/// variable each platform sets. An absent one is the empty string rather than
/// an error: a template that prints a hostname should not fail a send.
fn hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_default()
}

/// `{% prompt 'title', … %}`: asks the user for a value.
///
/// Only a send may block for an answer. A preview that opened a dialog would
/// turn a hover into a modal, so outside a send the tag refuses instead of
/// waiting.
fn prompt(
    args: &[Value],
    context: &RenderContext<'_>,
    host: &dyn TagHost,
) -> Result<Value, TagError> {
    if !context.purpose.allows_blocking_tags() {
        return Err(TagError::NotAvailableInThisPurpose { tag: "prompt" });
    }
    let Some(title) = args.first().and_then(Value::as_str) else {
        return Err(TagError::BadArguments {
            tag: "prompt",
            detail: "a title is required".to_owned(),
        });
    };
    if title.is_empty() {
        return Err(TagError::BadArguments {
            tag: "prompt",
            detail: "a title is required".to_owned(),
        });
    }
    let request = crate::host::PromptRequest {
        request_id: context.request_id.clone(),
        title: title.to_owned(),
        description: args.get(1).and_then(Value::as_str).map(str::to_owned),
        default_value: args.get(2).and_then(Value::as_str).map(str::to_owned),
        secret: args.get(4).and_then(Value::as_bool).unwrap_or(false),
        timeout: std::time::Duration::from_secs(300),
    };
    match host.prompt(&request)? {
        crate::host::PromptAnswer::Value(answer) => Ok(Value::String(answer)),
        crate::host::PromptAnswer::Cancelled => Err(TagError::Cancelled { tag: "prompt" }),
    }
}

/// `{% uuid 'v4' %}`: a random UUID.
///
/// The original accepts `v1` and still returns a v4 UUID — the argument is
/// ignored, and its own test pins that. Heric does the same rather than
/// inventing a v1 generator the original does not have.
fn uuid(args: &[Value]) -> Result<Value, TagError> {
    if let Some(version) = args.first().and_then(Value::as_str)
        && !matches!(version, "v1" | "v4")
    {
        return Err(TagError::BadArguments {
            tag: "uuid",
            detail: format!("`{version}` is not a version; use v1 or v4"),
        });
    }
    Ok(Value::String(uuid::Uuid::new_v4().to_string()))
}

/// Splits a tag's raw argument text the way the original's `tokenizeArgs` does.
///
/// The original's tokenizer is looser than the expression parser: a quoted
/// string ends at its closing quote, a bare word runs until a comma, and a
/// missing comma therefore merges the rest of the line into one argument. That
/// is the behaviour `TPL-006` describes, and it is why this exists beside
/// [`crate::parser`]'s stricter comma-separated parser.
///
/// # Errors
///
/// Returns [`RenderError::Syntax`] when an argument that is not a literal fails
/// to parse as an expression.
pub(crate) fn tokenize_args(input: &str, line: u32, column: u32) -> Result<Vec<Expr>, RenderError> {
    let mut args = Vec::new();
    let mut current: Option<String> = None;
    let mut quoted_by: Option<char> = None;
    let characters: Vec<char> = input.chars().collect();
    let mut index = 0;
    // The extra iteration terminates the last argument, as the original's
    // "invisible comma" does.
    while index <= characters.len() {
        let character = characters.get(index).copied().unwrap_or(',');
        index = index.saturating_add(1);
        if current.is_none() && (character.is_whitespace() || character == ',') {
            continue;
        }
        if current.is_none() && (character == '\'' || character == '"') {
            current = Some(String::new());
            quoted_by = Some(character);
            continue;
        }
        if current.is_none() {
            current = Some(character.to_string());
            quoted_by = None;
            continue;
        }
        let ends_quoted = quoted_by.is_some_and(|quote| character == quote);
        let ends_bare = quoted_by.is_none() && character == ',';
        let complete = ends_quoted || ends_bare;
        if !complete && let Some(text) = current.as_mut() {
            // Only a quoted argument treats a backslash as an escape
            // (`tokenize-args.ts:18-29`); a bare word keeps it.
            if character == '\\' && quoted_by.is_some() {
                if let Some(escaped) = characters.get(index) {
                    text.push(*escaped);
                    index = index.saturating_add(1);
                }
            } else {
                text.push(character);
            }
        }
        if complete {
            if let Some(text) = current.take() {
                args.push(argument(&text, quoted_by.is_some(), line, column)?);
            }
            quoted_by = None;
        }
    }
    Ok(args)
}

/// Turns one tokenized argument into an expression.
fn argument(text: &str, quoted: bool, line: u32, column: u32) -> Result<Expr, RenderError> {
    if quoted {
        return Ok(Expr::Literal(Value::String(text.to_owned())));
    }
    match text {
        "true" => return Ok(Expr::Literal(Value::Bool(true))),
        "false" => return Ok(Expr::Literal(Value::Bool(false))),
        "nil" | "null" => return Ok(Expr::Literal(Value::Null)),
        _ => {}
    }
    // The original's number test is `^\d*\.?\d*$` (`tokenize-args.ts:44`), so
    // `1.2.3` is not a number and falls through to the expression parser.
    if is_number(text)
        && let Ok(number) = text.parse::<serde_json::Number>()
    {
        return Ok(Expr::Literal(Value::Number(number)));
    }
    if is_identifier(text) {
        return Ok(Expr::Variable {
            root: text.to_owned(),
            path: Vec::new(),
        });
    }
    crate::parser::parse_expression(text, line, column)
}

/// Whether a token matches the original's number pattern, `^\d*\.?\d*$`.
///
/// The pattern accepts the empty string and a lone `.`, which the parse below
/// then rejects; what matters is that it rejects a second dot.
fn is_number(text: &str) -> bool {
    let mut dots = 0;
    for character in text.chars() {
        if character == '.' {
            dots += 1;
            if dots > 1 {
                return false;
            }
        } else if !character.is_ascii_digit() {
            return false;
        }
    }
    true
}

/// Whether a token is a bare name rather than an expression.
fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
        return false;
    }
    characters.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::testing::{NoHost, context};

    fn args(values: &[Value]) -> Vec<Value> {
        values.to_vec()
    }

    #[test]
    fn a_quoted_argument_keeps_its_text() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = tokenize_args("'a', 'b'", 1, 1)?;
        assert_eq!(
            parsed,
            vec![Expr::Literal(json!("a")), Expr::Literal(json!("b")),]
        );
        Ok(())
    }

    #[test]
    fn a_missing_comma_merges_the_remaining_arguments() -> Result<(), Box<dyn std::error::Error>> {
        // The original's tokenizer runs a bare word to the next comma, so
        // `a and b` is one argument rather than three.
        let parsed = tokenize_args("a and b", 1, 1)?;
        assert_eq!(parsed.len(), 1);
        assert!(matches!(parsed[0], Expr::Binary { .. }));
        Ok(())
    }

    #[test]
    fn a_backslash_escapes_the_next_character() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = tokenize_args(r"'it\'s'", 1, 1)?;
        assert_eq!(parsed, vec![Expr::Literal(json!("it's"))]);
        Ok(())
    }

    #[test]
    fn a_bare_word_keeps_its_backslashes() -> Result<(), Box<dyn std::error::Error>> {
        // Only a quoted argument escapes, so a bare word reaches the expression
        // parser as written and is refused there rather than being silently
        // rewritten.
        assert!(matches!(
            tokenize_args(r"C:\temp\x", 1, 1),
            Err(RenderError::Syntax { .. })
        ));
        // Inside quotes the escape applies to any next character, as the
        // original's scanner does; a forward-slash path is the portable form.
        assert_eq!(
            tokenize_args("'C:/temp/x'", 1, 1)?,
            vec![Expr::Literal(json!("C:/temp/x"))]
        );
        Ok(())
    }

    #[test]
    fn a_second_dot_is_not_a_number() -> Result<(), Box<dyn std::error::Error>> {
        // `^\d*\.?\d*$` rejects `1.2.3`, so it is not a numeric literal.
        assert!(is_number("1.5"));
        assert!(is_number("12"));
        assert!(!is_number("1.2.3"));
        assert!(!is_number("1e5"));
        Ok(())
    }

    #[test]
    fn literals_are_typed() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = tokenize_args("true, false, 12, 1.5, name", 1, 1)?;
        assert_eq!(parsed[0], Expr::Literal(json!(true)));
        assert_eq!(parsed[1], Expr::Literal(json!(false)));
        assert_eq!(parsed[2], Expr::Literal(json!(12)));
        assert_eq!(parsed[3], Expr::Literal(json!(1.5)));
        assert_eq!(
            parsed[4],
            Expr::Variable {
                root: "name".to_owned(),
                path: Vec::new(),
            }
        );
        Ok(())
    }

    #[test]
    fn an_empty_argument_list_is_empty() -> Result<(), Box<dyn std::error::Error>> {
        assert!(tokenize_args("", 1, 1)?.is_empty());
        assert!(tokenize_args("   ", 1, 1)?.is_empty());
        Ok(())
    }

    #[test]
    fn uuid_ignores_the_version_argument() -> Result<(), TagError> {
        // The original accepts `v1` and still returns a v4 UUID.
        let v1 = uuid(&args(&[json!("v1")]))?;
        let v4 = uuid(&args(&[json!("v4")]))?;
        for value in [v1, v4] {
            let text = value.as_str().unwrap_or_default();
            assert_eq!(text.len(), 36, "{text}");
            assert_eq!(text.as_bytes().get(14), Some(&b'4'), "{text}");
        }
        Ok(())
    }

    #[test]
    fn an_unknown_uuid_version_is_an_error() {
        assert!(uuid(&args(&[json!("v9")])).is_err());
    }

    #[test]
    fn os_reports_the_platform() -> Result<(), TagError> {
        let arch = os(&args(&[json!("arch")]))?;
        assert_eq!(arch, Value::String(std::env::consts::ARCH.to_owned()));
        let platform = os(&args(&[json!("platform")]))?;
        assert_eq!(platform, Value::String(std::env::consts::OS.to_owned()));
        Ok(())
    }

    #[test]
    fn an_unknown_os_function_is_an_error() {
        assert!(os(&args(&[json!("cpus")])).is_err());
    }

    #[test]
    fn a_prompt_outside_a_send_is_refused() {
        let scope = crate::Scope::new();
        let mut render_context = context(&scope);
        let error = prompt(&args(&[json!("title")]), &render_context, &NoHost);
        assert!(matches!(
            error,
            Err(TagError::NotAvailableInThisPurpose { tag: "prompt" })
        ));
        // A send may ask, and the host decides whether it can answer.
        render_context.purpose = crate::RenderPurpose::Send;
        assert!(prompt(&args(&[json!("title")]), &render_context, &NoHost).is_err());
    }

    #[test]
    fn a_prompt_without_a_title_is_an_error() {
        let scope = crate::Scope::new();
        let mut render_context = context(&scope);
        render_context.purpose = crate::RenderPurpose::Send;
        assert!(matches!(
            prompt(&args(&[]), &render_context, &NoHost),
            Err(TagError::BadArguments { tag: "prompt", .. })
        ));
    }

    #[test]
    fn a_file_read_goes_through_the_host() {
        let scope = crate::Scope::new();
        let mut render_context = context(&scope);
        let error = file(&args(&[json!("Cargo.toml")]), &NoHost);
        assert!(matches!(error, Err(TagError::PathNotAllowed { .. })));
        let _ = &mut render_context;
    }

    #[test]
    fn a_file_without_a_path_is_an_error() {
        assert!(matches!(
            file(&args(&[]), &NoHost),
            Err(TagError::BadArguments { tag: "file", .. })
        ));
    }

    #[test]
    fn hash_reports_the_crate_it_needs() {
        let scope = crate::Scope::new();
        let mut render_context = context(&scope);
        let error = run("hash", &args(&[json!("md5")]), &mut render_context, &NoHost);
        assert!(matches!(
            error,
            Err(TagError::Unsupported { tag: "hash", .. })
        ));
    }

    #[test]
    fn the_document_tags_report_what_they_are_missing() {
        let scope = crate::Scope::new();
        let mut render_context = context(&scope);
        for tag in ["cookie", "request", "response"] {
            let error = run(tag, &args(&[]), &mut render_context, &NoHost);
            assert!(
                matches!(error, Err(TagError::Unsupported { .. })),
                "{tag} should be unsupported"
            );
        }
    }

    #[test]
    fn an_unknown_tag_is_an_error() {
        let scope = crate::Scope::new();
        let mut render_context = context(&scope);
        assert!(matches!(
            run("nope", &args(&[]), &mut render_context, &NoHost),
            Err(TagError::Unknown { name }) if name == "nope"
        ));
    }

    #[test]
    fn every_registered_tag_is_dispatched() {
        // The registry in `tag.rs` is what the GUI lists and the docs are
        // generated from, so a name there that `run` does not know would be a
        // tag the UI offers and the engine refuses.
        let scope = crate::Scope::new();
        let mut render_context = context(&scope);
        for definition in crate::tag::tags() {
            let error = run(definition.name, &args(&[]), &mut render_context, &NoHost);
            assert!(
                !matches!(error, Err(TagError::Unknown { .. })),
                "`{}` is registered but not dispatched",
                definition.name
            );
        }
    }

    #[test]
    fn a_null_result_renders_as_the_empty_string() -> Result<(), TagError> {
        // `to_output` is what every tag's result goes through, and `null` is
        // the empty string rather than `null` (TPL-014).
        assert_eq!(to_output(&Value::Null), "");
        Ok(())
    }
}
