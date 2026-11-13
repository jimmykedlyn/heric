//! Turns template text into tokens.
//!
//! The lexer is deliberately dumb between delimiters: it hands out the raw
//! inner text of each `{{ … }}` and `{% … %}` and lets the parser decide what
//! it means. The one exception is `{% raw %}`, whose body must never be
//! tokenised at all — hence a dedicated token kind rather than a flag.

use crate::error::RenderError;

/// What a token is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// Literal text between tags.
    Text(String),
    /// The trimmed inner text of a `{{ … }}`.
    Output(String),
    /// The trimmed inner text of a `{% … %}`.
    Tag(String),
    /// The verbatim body of a `{% raw %}` block, already unwrapped.
    Raw(String),
}

/// A token and where it started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    /// What the token is.
    pub(crate) kind: TokenKind,
    /// Line of the opening delimiter, 1-based. For text, the line of its first
    /// character.
    pub(crate) line: u32,
    /// Column of the opening delimiter, 1-based.
    pub(crate) column: u32,
}

/// The opening of a `{% raw %}` block.
const RAW_OPEN: &str = "raw";
/// The closing of a `{% raw %}` block.
const RAW_CLOSE: &str = "{% endraw %}";

/// Splits `input` into tokens.
///
/// `input` must already be comment-free: `parser::parse` calls
/// `strip_comments` first. A `{#` that reaches the lexer anyway is treated as
/// text rather than being an error, so the lexer never has to know whether the
/// caller stripped.
///
/// # Errors
///
/// Returns [`RenderError::Syntax`] for an unterminated `{{` or `{%`, or for a
/// `{% raw %}` with no matching `{% endraw %}`. Every error points at the
/// opening delimiter, not at the end of input.
pub(crate) fn lex(input: &str) -> Result<Vec<Token>, RenderError> {
    let mut tokens = Vec::new();
    let mut rest = input;
    // Position of `rest`'s first byte, tracked so an error can name the line and
    // column of a delimiter that may be far into the input.
    let mut line = 1_u32;
    let mut column = 1_u32;

    while !rest.is_empty() {
        let Some(at) = find_delimiter(rest) else {
            tokens.push(Token {
                kind: TokenKind::Text(rest.to_owned()),
                line,
                column,
            });
            break;
        };

        // Anything before the delimiter is text, at the current position.
        if at > 0 {
            let (text, after) = rest.split_at(at);
            tokens.push(Token {
                kind: TokenKind::Text(text.to_owned()),
                line,
                column,
            });
            let (next_line, next_column) = advance(line, column, text);
            line = next_line;
            column = next_column;
            rest = after;
        }

        let (opener, closer) = if rest.starts_with("{{") {
            ("{{", "}}")
        } else {
            ("{%", "%}")
        };
        let inner_start = opener.len();
        let Some(close_at) = rest[inner_start..].find(closer) else {
            return Err(syntax(line, column, format!("unterminated `{opener}` tag")));
        };
        let inner = rest[inner_start..inner_start + close_at].trim().to_owned();
        let consumed = inner_start + close_at + closer.len();
        let whole = &rest[..consumed];
        let (next_line, next_column) = advance(line, column, whole);

        if opener == "{%" && inner == RAW_OPEN {
            let Some(close_seq) = find_endraw(rest) else {
                return Err(syntax(
                    line,
                    column,
                    "`{% raw %}` has no matching `{% endraw %}`".to_owned(),
                ));
            };
            let body_start = consumed;
            let body = rest
                .get(body_start..close_seq)
                .unwrap_or_default()
                .to_owned();
            // The lexer must consume exactly what it reports, so recompute the
            // consumed length from the body it found.
            let total = body_start + body.len() + RAW_CLOSE.len();
            let whole = rest.get(..total).unwrap_or(rest);
            let (next_line, next_column) = advance(line, column, whole);
            tokens.push(Token {
                kind: TokenKind::Raw(body),
                line,
                column,
            });
            line = next_line;
            column = next_column;
            rest = rest.get(total..).unwrap_or_default();
            continue;
        }

        let kind = if opener == "{{" {
            TokenKind::Output(inner)
        } else {
            TokenKind::Tag(inner)
        };
        tokens.push(Token { kind, line, column });
        line = next_line;
        column = next_column;
        rest = rest.get(consumed..).unwrap_or_default();
    }

    Ok(tokens)
}

/// Byte offset of the next `{{` or `{%`, whichever comes first.
fn find_delimiter(input: &str) -> Option<usize> {
    let output = input.find("{{");
    let tag = input.find("{%");
    match (output, tag) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Byte offset of the `{% endraw %}` that closes the block opened inside
/// `input`.
///
/// Liquid's `endraw` must match exactly, so a plain `find` is right: the closer
/// has no arguments and no whitespace freedom beyond the one that is written.
fn find_endraw(input: &str) -> Option<usize> {
    input.find(RAW_CLOSE)
}

/// Advances a 1-based line/column pair over `text`.
fn advance(line: u32, column: u32, text: &str) -> (u32, u32) {
    let mut line = line;
    let mut column = column;
    for character in text.chars() {
        if character == '\n' {
            line = line.saturating_add(1);
            column = 1;
        } else {
            column = column.saturating_add(1);
        }
    }
    (line, column)
}

/// Builds a syntax error with an empty path, since a lexer does not know which
/// request field it is looking at.
fn syntax(line: u32, column: u32, message: String) -> RenderError {
    RenderError::Syntax {
        path: String::new(),
        line,
        column,
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), RenderError>;

    #[test]
    fn plain_text_is_one_text_token() -> TestResult {
        let tokens = lex("plain text")?;
        assert!(
            matches!(tokens.as_slice(), [Token { kind: TokenKind::Text(t), .. }] if t == "plain text")
        );
        Ok(())
    }

    #[test]
    fn an_output_token_carries_its_trimmed_inner_text() -> TestResult {
        let tokens = lex("{{ name }}")?;
        assert!(
            matches!(tokens.as_slice(), [Token { kind: TokenKind::Output(t), line: 1, column: 1 }] if t == "name")
        );
        Ok(())
    }

    #[test]
    fn a_tag_token_keeps_its_arguments() -> TestResult {
        let tokens = lex("{% if x == 1 %}")?;
        assert!(
            matches!(tokens.as_slice(), [Token { kind: TokenKind::Tag(t), .. }] if t == "if x == 1")
        );
        Ok(())
    }

    #[test]
    fn text_and_tags_alternate_with_positions() -> TestResult {
        let tokens = lex("a{{ b }}c")?;
        let kinds: Vec<&TokenKind> = tokens.iter().map(|token| &token.kind).collect();
        assert!(matches!(
            kinds.as_slice(),
            [TokenKind::Text(a), TokenKind::Output(_), TokenKind::Text(c)]
                if a == "a" && c == "c"
        ));
        assert_eq!((tokens[1].line, tokens[1].column), (1, 2));
        assert_eq!((tokens[2].line, tokens[2].column), (1, 9));
        Ok(())
    }

    #[test]
    fn a_newline_moves_the_next_token_to_the_next_line() -> TestResult {
        let tokens = lex("line one\n{{ nope }}")?;
        assert_eq!((tokens[1].line, tokens[1].column), (2, 1));
        Ok(())
    }

    #[test]
    fn an_unterminated_output_tag_errors_at_its_opening() {
        assert!(
            matches!(
                lex("{{ name"),
                Err(RenderError::Syntax {
                    line: 1,
                    column: 1,
                    ..
                })
            ),
            "expected a syntax error at 1:1"
        );
    }

    #[test]
    fn an_unterminated_tag_errors_at_its_opening() {
        assert!(
            matches!(
                lex("ok
  {% if x"),
                Err(RenderError::Syntax {
                    line: 2,
                    column: 3,
                    ..
                })
            ),
            "expected a syntax error at 2:3"
        );
    }

    #[test]
    fn a_raw_block_is_one_token_with_a_verbatim_body() -> TestResult {
        let tokens = lex("{% raw %}{{ x }}{% endraw %}")?;
        assert!(
            matches!(tokens.as_slice(), [Token { kind: TokenKind::Raw(body), .. }] if body == "{{ x }}")
        );
        Ok(())
    }

    #[test]
    fn a_raw_block_can_contain_a_line_break() -> TestResult {
        let tokens = lex("{% raw %}a\nb{% endraw %}")?;
        assert!(
            matches!(tokens.as_slice(), [Token { kind: TokenKind::Raw(body), .. }] if body == "a\nb")
        );
        Ok(())
    }

    #[test]
    fn an_unclosed_raw_block_errors_at_its_opening() {
        assert!(
            matches!(
                lex("x {% raw %}body"),
                Err(RenderError::Syntax {
                    line: 1,
                    column: 3,
                    ..
                })
            ),
            "expected a syntax error at 1:3"
        );
    }

    #[test]
    fn text_after_a_raw_block_is_lexed_again() -> TestResult {
        let tokens = lex("{% raw %}{{ x }}{% endraw %}{{ y }}")?;
        assert!(matches!(
            tokens.as_slice(),
            [Token { kind: TokenKind::Raw(_), .. }, Token { kind: TokenKind::Output(y), .. }] if y == "y"
        ));
        Ok(())
    }
}
