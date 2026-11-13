//! Recursive-descent parser for the Liquid dialect the original uses.
//!
//! No parser-combinator crate: the grammar is small and the original's
//! behaviour is the specification, so a hand-written parser is easier to keep
//! honest than a combinator tree.
//!
//! Comments are stripped before lexing, exactly as the original does
//! (`stripLiquidComments`, non-greedy and quote-blind). Every reported position
//! therefore refers to the *stripped* text — the original reports the same way,
//! so do not try to compensate for the shift.

use std::borrow::Cow;

use serde_json::{Number, Value};

use crate::ast::{BinaryOp, Expr, Node, Segment, UnaryOp};
use crate::error::RenderError;
use crate::lexer::{Token, TokenKind, lex};

/// Parses `input` into a node list.
///
/// The `path` on any [`RenderError::Syntax`] this returns is empty: a parser
/// does not know which request field it is looking at. Task 13 rewrites it to
/// the field path (`_request.headers[0].value`) before showing it to a user.
///
/// # Errors
///
/// Returns [`RenderError::Syntax`] for an unterminated tag, an unclosed block,
/// malformed expression syntax, or an unexpected block-closing tag.
pub(crate) fn parse(input: &str) -> Result<Vec<Node>, RenderError> {
    let stripped = strip_comments(input);
    let tokens = lex(&stripped)?;
    let mut parser = Parser {
        tokens: &tokens,
        position: 0,
    };
    let nodes = parser.parse_body(&[])?;
    match parser.peek() {
        None => Ok(nodes),
        Some(token) => Err(syntax_at(token, "unexpected trailing block tag")),
    }
}

/// Removes `{# … #}` comments.
///
/// The original's regex is `/\{#[\s\S]*?#\}/g`: non-greedy, newline-matching and
/// blind to quotes, so the first `#}` after a `{#` closes the comment even when
/// it sits inside a string. Reproduced by hand to avoid a `regex` dependency.
/// An unclosed `{#` leaves the rest of the input alone — the regex would not
/// match either.
pub(crate) fn strip_comments(input: &str) -> Cow<'_, str> {
    let Some(first) = input.find("{#") else {
        return Cow::Borrowed(input);
    };
    let mut stripped = String::with_capacity(input.len());
    let mut rest = input;
    // `first` is the offset of the next `{#` within `rest`; any text before it
    // is kept verbatim.
    let mut next = Some(first);
    while let Some(at) = next {
        let Some((before, from_comment)) = rest.split_at_checked(at) else {
            break;
        };
        stripped.push_str(before);
        let Some(close) = from_comment.find("#}") else {
            // No closing delimiter: the original's regex does not match, so the
            // remainder survives untouched.
            stripped.push_str(from_comment);
            return Cow::Owned(stripped);
        };
        rest = from_comment.get(close + 2..).unwrap_or_default();
        next = rest.find("{#");
    }
    stripped.push_str(rest);
    Cow::Owned(stripped)
}

/// Reports whether `input` contains template syntax.
///
/// This is the original's short-circuit test, verbatim
/// (`common/render.ts:632-634`): each pair of delimiters must both be present.
/// A lone `{%` with no `%}` is text, which is why the check is a conjunction.
pub(crate) fn has_tags(input: &str) -> bool {
    (input.contains("{{") && input.contains("}}"))
        || (input.contains("{%") && input.contains("%}"))
        || (input.contains("{#") && input.contains("#}"))
}

/// Builds a syntax error with an empty path.
fn syntax_at(token: &Token, message: &str) -> RenderError {
    RenderError::Syntax {
        path: String::new(),
        line: token.line,
        column: token.column,
        message: message.to_owned(),
    }
}

/// Builds a syntax error at an arbitrary position with an empty path.
fn syntax(line: u32, column: u32, message: String) -> RenderError {
    RenderError::Syntax {
        path: String::new(),
        line,
        column,
        message,
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl<'a> Parser<'a> {
    /// Parses nodes until end of input or a closer in `stops`.
    ///
    /// A closer that is not in `stops` is left unconsumed for the caller to
    /// reject, so an `endif` with no `if` becomes a clear error instead of
    /// being silently swallowed.
    fn parse_body(&mut self, stops: &[&str]) -> Result<Vec<Node>, RenderError> {
        let mut nodes = Vec::new();
        while let Some(token) = self.peek() {
            match &token.kind {
                TokenKind::Text(text) => {
                    let text = text.clone();
                    self.position = self.position.saturating_add(1);
                    nodes.push(Node::Text(text));
                }
                TokenKind::Raw(body) => {
                    let body = body.clone();
                    self.position = self.position.saturating_add(1);
                    nodes.push(Node::Raw(body));
                }
                TokenKind::Output(inner) => {
                    let (inner, line, column) = (inner.clone(), token.line, token.column);
                    self.position = self.position.saturating_add(1);
                    let expr = parse_expression(&inner, line, column)?;
                    nodes.push(Node::Output { expr, line, column });
                }
                TokenKind::Tag(inner) => {
                    let (inner, line, column) = (inner.clone(), token.line, token.column);
                    let name = first_word(&inner).to_owned();
                    if stops.contains(&name.as_str()) {
                        return Ok(nodes);
                    }
                    self.position = self.position.saturating_add(1);
                    nodes.push(self.parse_tag(&name, &inner, line, column)?);
                }
            }
        }
        Ok(nodes)
    }

    /// Parses one `{% %}` construct whose name is already known.
    fn parse_tag(
        &mut self,
        name: &str,
        inner: &str,
        line: u32,
        column: u32,
    ) -> Result<Node, RenderError> {
        let rest = inner.get(name.len()..).unwrap_or_default().trim();
        match name {
            "if" => self.parse_if(rest, line, column),
            "unless" => self.parse_unless(rest, line, column),
            "for" => self.parse_for(rest, line, column),
            "case" => self.parse_case(rest, line, column),
            "assign" => self.parse_assign(rest, line, column),
            "capture" => self.parse_capture(rest, line, column),
            "" => Err(syntax(line, column, "empty tag".to_owned())),
            "endif" | "endunless" | "endfor" | "endcase" | "endcapture" | "elsif" | "else"
            | "when" => Err(syntax(
                line,
                column,
                format!("`{{% {name} %}}` has no matching opening tag"),
            )),
            _ => {
                // A tag's arguments follow the original's looser tokenizer
                // rather than the expression parser: `{% uuid 'v4' %}` and
                // `{% file '/tmp/x y' %}` are legal, and a missing comma merges
                // the rest of the line into one argument (TPL-006).
                let args = crate::tags::tokenize_args(rest, line, column)?;
                Ok(Node::Tag {
                    name: name.to_owned(),
                    args,
                    line,
                    column,
                })
            }
        }
    }

    fn parse_if(&mut self, rest: &str, line: u32, column: u32) -> Result<Node, RenderError> {
        let mut branches = Vec::new();
        let mut condition = parse_expression(rest, line, column)?;
        loop {
            let body = self.parse_body(&["elsif", "else", "endif"])?;
            branches.push((condition, body));
            let Some(token) = self.peek() else {
                return Err(syntax(
                    line,
                    column,
                    "`{% if %}` has no `{% endif %}`".to_owned(),
                ));
            };
            let TokenKind::Tag(inner) = &token.kind else {
                return Err(syntax_at(
                    token,
                    "expected `{% elsif %}`, `{% else %}` or `{% endif %}`",
                ));
            };
            let inner = inner.clone();
            let word = first_word(&inner).to_owned();
            let (tag_line, tag_column) = (token.line, token.column);
            self.position = self.position.saturating_add(1);
            match word.as_str() {
                "elsif" => {
                    let args = rest_after_word(&inner);
                    condition = parse_expression(args, tag_line, tag_column)?;
                }
                "else" => {
                    let otherwise = self.parse_body(&["endif"])?;
                    self.expect_tag("endif", line, column, "`{% if %}`")?;
                    return Ok(Node::If {
                        branches,
                        otherwise,
                        line,
                        column,
                    });
                }
                "endif" => {
                    return Ok(Node::If {
                        branches,
                        otherwise: Vec::new(),
                        line,
                        column,
                    });
                }
                _ => {
                    return Err(syntax_at(
                        token,
                        "expected `{% elsif %}`, `{% else %}` or `{% endif %}`",
                    ));
                }
            }
        }
    }

    fn parse_unless(&mut self, rest: &str, line: u32, column: u32) -> Result<Node, RenderError> {
        let condition = parse_expression(rest, line, column)?;
        let body = self.parse_body(&["else", "endunless"])?;
        let Some(token) = self.peek() else {
            return Err(syntax(
                line,
                column,
                "`{% unless %}` has no `{% endunless %}`".to_owned(),
            ));
        };
        let TokenKind::Tag(inner) = &token.kind else {
            return Err(syntax_at(
                token,
                "expected `{% else %}` or `{% endunless %}`",
            ));
        };
        if first_word(inner) == "else" {
            self.position = self.position.saturating_add(1);
            let otherwise = self.parse_body(&["endunless"])?;
            self.expect_tag("endunless", line, column, "`{% unless %}`")?;
            return Ok(Node::Unless {
                condition,
                body,
                otherwise,
                line,
                column,
            });
        }
        self.position = self.position.saturating_add(1);
        Ok(Node::Unless {
            condition,
            body,
            otherwise: Vec::new(),
            line,
            column,
        })
    }

    fn parse_for(&mut self, rest: &str, line: u32, column: u32) -> Result<Node, RenderError> {
        let (variable, iterable) = parse_loop_header(rest, line, column)?;
        let body = self.parse_body(&["else", "endfor"])?;
        let Some(token) = self.peek() else {
            return Err(syntax(
                line,
                column,
                "`{% for %}` has no `{% endfor %}`".to_owned(),
            ));
        };
        let TokenKind::Tag(inner) = &token.kind else {
            return Err(syntax_at(token, "expected `{% else %}` or `{% endfor %}`"));
        };
        if first_word(inner) == "else" {
            self.position = self.position.saturating_add(1);
            let otherwise = self.parse_body(&["endfor"])?;
            self.expect_tag("endfor", line, column, "`{% for %}`")?;
            return Ok(Node::For {
                variable,
                iterable,
                body,
                otherwise,
                line,
                column,
            });
        }
        self.position = self.position.saturating_add(1);
        Ok(Node::For {
            variable,
            iterable,
            body,
            otherwise: Vec::new(),
            line,
            column,
        })
    }

    fn parse_case(&mut self, rest: &str, line: u32, column: u32) -> Result<Node, RenderError> {
        let scrutinee = parse_expression(rest, line, column)?;
        let mut whens: Vec<(Vec<Expr>, Vec<Node>)> = Vec::new();
        loop {
            let Some(token) = self.peek() else {
                return Err(syntax(
                    line,
                    column,
                    "`{% case %}` has no `{% endcase %}`".to_owned(),
                ));
            };
            let TokenKind::Tag(inner) = &token.kind else {
                return Err(syntax_at(
                    token,
                    "expected `{% when %}`, `{% else %}` or `{% endcase %}`",
                ));
            };
            let inner = inner.clone();
            let word = first_word(&inner).to_owned();
            let (tag_line, tag_column) = (token.line, token.column);
            self.position = self.position.saturating_add(1);
            match word.as_str() {
                "when" => {
                    let args = rest_after_word(&inner);
                    let values = parse_argument_list(args, tag_line, tag_column)?;
                    let body = self.parse_body(&["when", "else", "endcase"])?;
                    whens.push((values, body));
                }
                "else" => {
                    let otherwise = self.parse_body(&["endcase"])?;
                    self.expect_tag("endcase", line, column, "`{% case %}`")?;
                    return Ok(Node::Case {
                        scrutinee,
                        whens,
                        otherwise,
                        line,
                        column,
                    });
                }
                "endcase" => {
                    return Ok(Node::Case {
                        scrutinee,
                        whens,
                        otherwise: Vec::new(),
                        line,
                        column,
                    });
                }
                _ => {
                    return Err(syntax_at(
                        token,
                        "expected `{% when %}`, `{% else %}` or `{% endcase %}`",
                    ));
                }
            }
        }
    }

    fn parse_assign(&mut self, rest: &str, line: u32, column: u32) -> Result<Node, RenderError> {
        let Some((target, value)) = rest.split_once('=') else {
            return Err(syntax(
                line,
                column,
                "`{% assign %}` needs `name = value`".to_owned(),
            ));
        };
        let target = require_identifier(target.trim(), line, column, "`{% assign %}`")?;
        let value = parse_expression(value.trim(), line, column)?;
        Ok(Node::Assign {
            target,
            value,
            line,
            column,
        })
    }

    fn parse_capture(&mut self, rest: &str, line: u32, column: u32) -> Result<Node, RenderError> {
        let target = require_identifier(rest, line, column, "`{% capture %}`")?;
        let body = self.parse_body(&["endcapture"])?;
        self.expect_tag("endcapture", line, column, "`{% capture %}`")?;
        Ok(Node::Capture {
            target,
            body,
            line,
            column,
        })
    }

    /// Consumes the closer `expected` and errors if the next token is anything
    /// else, reporting the opening tag's position so the message points at the
    /// block a reader is looking for.
    fn expect_tag(
        &mut self,
        expected: &str,
        line: u32,
        column: u32,
        what: &str,
    ) -> Result<(), RenderError> {
        let Some(token) = self.peek() else {
            return Err(syntax(
                line,
                column,
                format!("{what} has no `{{% {expected} %}}`"),
            ));
        };
        let TokenKind::Tag(inner) = &token.kind else {
            return Err(syntax_at(token, "expected a block-closing tag"));
        };
        if first_word(inner) == expected {
            self.position = self.position.saturating_add(1);
            return Ok(());
        }
        Err(syntax_at(token, "expected a different block-closing tag"))
    }

    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.position)
    }
}

/// The first whitespace-delimited word of a tag's inner text.
fn first_word(inner: &str) -> &str {
    inner.split_whitespace().next().unwrap_or_default()
}

/// The text of a tag after its first word, trimmed.
fn rest_after_word(inner: &str) -> &str {
    inner
        .split_once(char::is_whitespace)
        .map_or("", |(_, rest)| rest)
        .trim()
}

/// Errors unless `text` is a bare identifier (letters, digits, `_`, not
/// starting with a digit). Dotted targets are rejected because M1's `assign`
/// and `capture` only bind top-level names.
fn require_identifier(
    text: &str,
    line: u32,
    column: u32,
    what: &str,
) -> Result<String, RenderError> {
    if text.is_empty() {
        return Err(syntax(
            line,
            column,
            format!("{what} needs a variable name"),
        ));
    }
    if is_identifier(text) {
        return Ok(text.to_owned());
    }
    Err(syntax(
        line,
        column,
        format!("{what} needs a plain variable name, got `{text}`"),
    ))
}

/// Whether `text` is a bare identifier.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first.is_ascii_digit()) {
        return false;
    }
    if first.is_ascii_digit() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

/// Parses a whole expression, rejecting anything left over.
pub(crate) fn parse_expression(text: &str, line: u32, column: u32) -> Result<Expr, RenderError> {
    let pieces = scan(text, line, column)?;
    let mut parser = ExprParser {
        tokens: &pieces,
        position: 0,
        line,
        column,
    };
    let expr = parser.parse_or()?;
    parser.reject_keyword_argument()?;
    match parser.peek() {
        None => Ok(expr),
        Some(left) => {
            Err(parser.error(format!("unexpected `{}` after the expression", left.text())))
        }
    }
}

/// Parses a comma-separated argument list. An empty string is an empty list.
pub(crate) fn parse_arguments(
    text: &str,
    line: u32,
    column: u32,
) -> Result<Vec<Expr>, RenderError> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let pieces = scan(text, line, column)?;
    let mut parser = ExprParser {
        tokens: &pieces,
        position: 0,
        line,
        column,
    };
    parser.parse_argument_list()
}

/// Splits a `{% for %}` header into its variable and iterable.
fn parse_loop_header(text: &str, line: u32, column: u32) -> Result<(String, Expr), RenderError> {
    let pieces = scan(text, line, column)?;
    let mut parser = ExprParser {
        tokens: &pieces,
        position: 0,
        line,
        column,
    };
    let Some(variable) = parser.next_identifier() else {
        return Err(parser.error("`{% for %}` needs a loop variable".to_owned()));
    };
    match parser.next() {
        Some(Piece::Ident(word)) if word == "in" => {}
        _ => {
            return Err(parser.error("`{% for %}` needs `in` after the loop variable".to_owned()));
        }
    }
    let iterable = parser.parse_or()?;
    match parser.peek() {
        None => Ok((variable, iterable)),
        Some(left) => Err(parser.error(format!(
            "`{{% for %}}` does not accept `{}`; `limit`, `offset` and `reversed` are not implemented",
            left.text()
        ))),
    }
}

/// Splits a `{% when %}` argument list on top-level commas.
pub(crate) fn parse_argument_list(
    text: &str,
    line: u32,
    column: u32,
) -> Result<Vec<Expr>, RenderError> {
    parse_arguments(text, line, column)
}

/// One token of an expression.
#[derive(Debug, Clone, PartialEq)]
enum Piece {
    /// A bare word: a variable name, `and`, `or`, `contains`, `true`, `false`,
    /// `nil`.
    Ident(String),
    /// A quoted string, already unescaped.
    Str(String),
    /// A numeric literal.
    Number(Number),
    /// A punctuation or operator sequence.
    Punct(String),
}

impl Piece {
    /// How the piece is spelled, for error messages.
    fn text(&self) -> String {
        match self {
            Self::Ident(word) => word.clone(),
            Self::Str(text) => format!("'{text}'"),
            Self::Number(number) => number.to_string(),
            Self::Punct(text) => text.clone(),
        }
    }
}

/// Scans an expression's inner text into tokens.
fn scan(text: &str, line: u32, column: u32) -> Result<Vec<Piece>, RenderError> {
    let error = |message: String| RenderError::Syntax {
        path: String::new(),
        line,
        column,
        message,
    };
    let mut pieces = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((at, character)) = chars.next() {
        if character.is_whitespace() {
            continue;
        }
        if character == '\'' || character == '"' {
            let quote = character;
            let mut literal = String::new();
            let mut closed = false;
            while let Some((_, next)) = chars.next() {
                if next == '\\' {
                    let Some((_, escaped)) = chars.next() else {
                        break;
                    };
                    literal.push(match escaped {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        other => other,
                    });
                } else if next == quote {
                    closed = true;
                    break;
                } else {
                    literal.push(next);
                }
            }
            if !closed {
                return Err(error(format!("unterminated string starting at `{quote}`")));
            }
            pieces.push(Piece::Str(literal));
            continue;
        }
        if character.is_ascii_digit() {
            let end = scan_number(text, at);
            let literal = text.get(at..end).unwrap_or_default();
            let number = literal.parse::<Number>().map_err(|_| {
                error(format!(
                    "`{literal}` is not a number this engine can represent"
                ))
            })?;
            pieces.push(Piece::Number(number));
            // `scan_number` returns a byte offset. The outer `chars.next()`
            // already consumed the first digit, so skip the rest.
            let consumed = text.get(at..end).unwrap_or_default();
            let mut skipped = 1;
            while skipped < consumed.chars().count() {
                chars.next();
                skipped += 1;
            }
            continue;
        }
        if character.is_ascii_alphabetic() || character == '_' {
            let mut word = String::from(character);
            while let Some((_, next)) = chars.peek() {
                if next.is_ascii_alphanumeric() || *next == '_' || *next == '-' {
                    word.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }
            pieces.push(Piece::Ident(word));
            continue;
        }
        // Punctuation. Two-character operators are recognised before the
        // single-character forms so `..` never reads as two path separators.
        let rest = text.get(at..).unwrap_or_default();
        let punct = ["==", "!=", "<=", ">=", ".."]
            .iter()
            .find(|candidate| rest.starts_with(**candidate))
            .copied()
            .unwrap_or_else(|| rest.get(..1).unwrap_or_default());
        if !matches!(
            punct,
            "==" | "!="
                | "<="
                | ">="
                | ".."
                | "|"
                | ":"
                | ","
                | "("
                | ")"
                | "["
                | "]"
                | "."
                | "<"
                | ">"
                | "-"
        ) {
            return Err(error(format!("unexpected `{punct}` in an expression")));
        }
        pieces.push(Piece::Punct(punct.to_owned()));
        let mut skipped = 1;
        while skipped < punct.chars().count() {
            chars.next();
            skipped += 1;
        }
    }
    Ok(pieces)
}

/// Byte offset just past the number that starts at `at`.
fn scan_number(text: &str, at: usize) -> usize {
    let bytes = text.as_bytes();
    let mut at = at;
    while at < bytes.len() && bytes.get(at).is_some_and(u8::is_ascii_digit) {
        at = at.saturating_add(1);
    }
    // A `.` only continues the number when a digit follows, so `1..2` stays a
    // range rather than becoming `1.` and a stray `.`.
    if bytes.get(at) == Some(&b'.')
        && bytes
            .get(at.saturating_add(1))
            .is_some_and(u8::is_ascii_digit)
    {
        at = at.saturating_add(1);
        while at < bytes.len() && bytes.get(at).is_some_and(u8::is_ascii_digit) {
            at = at.saturating_add(1);
        }
    }
    at
}

struct ExprParser<'a> {
    tokens: &'a [Piece],
    position: usize,
    line: u32,
    column: u32,
}

impl<'a> ExprParser<'a> {
    fn error(&self, message: String) -> RenderError {
        RenderError::Syntax {
            path: String::new(),
            line: self.line,
            column: self.column,
            message,
        }
    }

    fn peek(&self) -> Option<&'a Piece> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<&'a Piece> {
        let token = self.tokens.get(self.position);
        if token.is_some() {
            self.position = self.position.saturating_add(1);
        }
        token
    }

    fn next_identifier(&mut self) -> Option<String> {
        match self.peek() {
            Some(Piece::Ident(word)) => {
                let word = word.clone();
                self.position = self.position.saturating_add(1);
                Some(word)
            }
            _ => None,
        }
    }

    /// Consumes `punct` when it is next.
    fn eat(&mut self, punct: &str) -> bool {
        if matches!(self.peek(), Some(Piece::Punct(found)) if found == punct) {
            self.position = self.position.saturating_add(1);
            return true;
        }
        false
    }

    /// Consumes `word` when it is the next identifier.
    fn eat_word(&mut self, word: &str) -> bool {
        if matches!(self.peek(), Some(Piece::Ident(found)) if found == word) {
            self.position = self.position.saturating_add(1);
            return true;
        }
        false
    }

    fn parse_or(&mut self) -> Result<Expr, RenderError> {
        let mut lhs = self.parse_and()?;
        while self.eat_word("or") {
            let rhs = self.parse_and()?;
            lhs = binary(BinaryOp::Or, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, RenderError> {
        let mut lhs = self.parse_contains()?;
        while self.eat_word("and") {
            let rhs = self.parse_contains()?;
            lhs = binary(BinaryOp::And, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_contains(&mut self) -> Result<Expr, RenderError> {
        let mut lhs = self.parse_equality()?;
        while self.eat_word("contains") {
            let rhs = self.parse_equality()?;
            lhs = binary(BinaryOp::Contains, lhs, rhs);
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> Result<Expr, RenderError> {
        let mut lhs = self.parse_comparison()?;
        loop {
            let op = if self.eat("==") {
                BinaryOp::Eq
            } else if self.eat("!=") {
                BinaryOp::Ne
            } else {
                return Ok(lhs);
            };
            let rhs = self.parse_comparison()?;
            lhs = binary(op, lhs, rhs);
        }
    }

    fn parse_comparison(&mut self) -> Result<Expr, RenderError> {
        let mut lhs = self.parse_range()?;
        loop {
            let op = if self.eat("<=") {
                BinaryOp::Le
            } else if self.eat(">=") {
                BinaryOp::Ge
            } else if self.eat("<") {
                BinaryOp::Lt
            } else if self.eat(">") {
                BinaryOp::Gt
            } else {
                return Ok(lhs);
            };
            let rhs = self.parse_range()?;
            lhs = binary(op, lhs, rhs);
        }
    }

    fn parse_range(&mut self) -> Result<Expr, RenderError> {
        let from = self.parse_unary()?;
        if !self.eat("..") {
            return Ok(from);
        }
        let to = self.parse_unary()?;
        Ok(Expr::Range {
            from: Box::new(from),
            to: Box::new(to),
        })
    }

    fn parse_unary(&mut self) -> Result<Expr, RenderError> {
        if self.eat_word("not") {
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            });
        }
        if self.eat("-") {
            let operand = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Negate,
                operand: Box::new(operand),
            });
        }
        self.parse_filtered()
    }

    fn parse_filtered(&mut self) -> Result<Expr, RenderError> {
        let mut expr = self.parse_primary()?;
        while self.eat("|") {
            let Some(name) = self.next_identifier() else {
                return Err(self.error("a filter needs a name".to_owned()));
            };
            let mut args = Vec::new();
            if self.eat(":") {
                args.push(self.parse_or()?);
                while self.eat(",") {
                    args.push(self.parse_or()?);
                }
            }
            expr = Expr::Filter {
                input: Box::new(expr),
                name,
                args,
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, RenderError> {
        let Some(token) = self.next() else {
            return Err(self.error("expected a value".to_owned()));
        };
        match token {
            Piece::Str(text) => Ok(Expr::Literal(Value::String(text.clone()))),
            Piece::Number(number) => Ok(Expr::Literal(Value::Number(number.clone()))),
            Piece::Punct(punct) if punct == "(" => {
                let inner = self.parse_or()?;
                if !self.eat(")") {
                    return Err(self.error("expected `)`".to_owned()));
                }
                Ok(inner)
            }
            Piece::Ident(word) => match word.as_str() {
                "true" => Ok(Expr::Literal(Value::Bool(true))),
                "false" => Ok(Expr::Literal(Value::Bool(false))),
                "nil" | "null" => Ok(Expr::Literal(Value::Null)),
                _ => self.parse_path(word.clone()),
            },
            Piece::Punct(other) => Err(self.error(format!("expected a value, found `{other}`"))),
        }
    }

    /// Continues a variable reference after its root name.
    fn parse_path(&mut self, root: String) -> Result<Expr, RenderError> {
        let mut path = Vec::new();
        loop {
            if self.eat(".") {
                match self.next() {
                    Some(Piece::Ident(key)) => path.push(Segment::Key(key.clone())),
                    Some(Piece::Number(number)) => {
                        let index = number.as_u64().ok_or_else(|| {
                            self.error("a path index must be a non-negative integer".to_owned())
                        })?;
                        let index = usize::try_from(index).map_err(|_| {
                            self.error("a path index is too large for this platform".to_owned())
                        })?;
                        path.push(Segment::Index(index));
                    }
                    _ => return Err(self.error("expected a name after `.`".to_owned())),
                }
                continue;
            }
            if self.eat("[") {
                let segment = match self.next() {
                    Some(Piece::Str(key)) => Segment::Key(key.clone()),
                    Some(Piece::Number(number)) => {
                        let index = number.as_u64().ok_or_else(|| {
                            self.error("a path index must be a non-negative integer".to_owned())
                        })?;
                        let index = usize::try_from(index).map_err(|_| {
                            self.error("a path index is too large for this platform".to_owned())
                        })?;
                        Segment::Index(index)
                    }
                    Some(Piece::Ident(name)) => Segment::Key(format!("[{name}]")),
                    _ => return Err(self.error("expected a key or index inside `[`".to_owned())),
                };
                if !self.eat("]") {
                    return Err(self.error("expected `]`".to_owned()));
                }
                path.push(segment);
                continue;
            }
            return Ok(Expr::Variable { root, path });
        }
    }

    /// Parses `expr, expr, …` with no surrounding brackets.
    fn parse_argument_list(&mut self) -> Result<Vec<Expr>, RenderError> {
        let mut args = vec![self.parse_or()?];
        self.reject_keyword_argument()?;
        while self.eat(",") {
            // A keyword argument would spell `name: value`; Task 14 owns that
            // shape and Task 10's evaluator cannot consume it yet, so name it
            // rather than inventing a representation.
            if matches!(
                (self.peek(), self.tokens.get(self.position.saturating_add(1))),
                (Some(Piece::Ident(_)), Some(Piece::Punct(punct))) if punct == ":"
            ) {
                return Err(self.error(
                    "keyword arguments are not supported; pass the value positionally".to_owned(),
                ));
            }
            args.push(self.parse_or()?);
            self.reject_keyword_argument()?;
        }
        Ok(args)
    }

    /// Rejects `name: value`, which Task 14 owns and this evaluator cannot
    /// represent. Task 10's `Expr` has no variant for it, so failing here beats
    /// silently dropping the keyword.
    fn reject_keyword_argument(&self) -> Result<(), RenderError> {
        if matches!(self.peek(), Some(Piece::Punct(punct)) if punct == ":") {
            return Err(self.error(
                "keyword arguments are not supported; pass the value positionally".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Builds a binary expression.
fn binary(op: BinaryOp, lhs: Expr, rhs: Expr) -> Expr {
    Expr::Binary {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestResult = Result<(), RenderError>;

    /// Parses `input` and requires exactly one node.
    fn one(input: &str) -> Result<Node, RenderError> {
        let mut nodes = parse(input)?;
        assert_eq!(nodes.len(), 1, "expected one node, got {nodes:?}");
        match nodes.pop() {
            Some(node) => Ok(node),
            None => Err(syntax(0, 0, "expected one node".to_owned())),
        }
    }

    // -- comments ----------------------------------------------------------

    #[test]
    fn a_multiline_comment_is_removed_before_parsing() {
        assert_eq!(strip_comments("a{# one\ntwo #}b"), "ab");
    }

    #[test]
    fn a_hash_brace_inside_a_comment_ends_it() {
        // Non-greedy and quote-blind: the first `#}` closes the comment even
        // inside quotes, so the stray quote survives.
        assert_eq!(strip_comments("{# 'a #}' tail"), "' tail");
    }

    #[test]
    fn an_unclosed_comment_is_left_alone() {
        assert_eq!(strip_comments("a{# never closed"), "a{# never closed");
    }

    #[test]
    fn several_comments_are_all_removed() {
        assert_eq!(strip_comments("{#a#}x{#b#}y"), "xy");
    }

    #[test]
    fn text_without_comments_borrows() {
        assert!(matches!(strip_comments("no comments"), Cow::Borrowed(_)));
    }

    // -- has_tags ----------------------------------------------------------

    #[test]
    fn a_lone_opening_delimiter_is_not_template_syntax() {
        assert!(has_tags("plain {%- not really -%}"));
        assert!(!has_tags("plain text"));
        assert!(!has_tags("a {{ b"));
    }

    #[test]
    fn both_delimiters_of_any_pair_are_template_syntax() {
        assert!(has_tags("{{ name }}"));
        assert!(has_tags("{% uuid 'v4' %}"));
        assert!(has_tags("{# #}"));
    }

    // -- text --------------------------------------------------------------

    #[test]
    fn text_without_delimiters_is_one_text_node() -> TestResult {
        assert!(matches!(one("plain text")?, Node::Text(t) if t == "plain text"));
        Ok(())
    }

    #[test]
    fn text_around_a_tag_stays_separate() -> TestResult {
        let nodes = parse("a{{ b }}c")?;
        assert_eq!(nodes.len(), 3);
        assert!(matches!(nodes.first(), Some(Node::Text(t)) if t == "a"));
        assert!(matches!(nodes.get(1), Some(Node::Output { .. })));
        assert!(matches!(nodes.get(2), Some(Node::Text(t)) if t == "c"));
        Ok(())
    }

    // -- errors ------------------------------------------------------------

    #[test]
    fn an_unclosed_output_tag_is_a_syntax_error() {
        assert!(
            matches!(
                parse("{{ name"),
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
    fn an_unclosed_block_tag_is_a_syntax_error() {
        assert!(
            matches!(
                parse("{% if x %}body"),
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
    fn a_stray_closer_is_a_syntax_error() {
        assert!(matches!(
            parse("{% endif %}"),
            Err(RenderError::Syntax { .. })
        ));
    }

    #[test]
    fn an_empty_tag_is_a_syntax_error() {
        assert!(matches!(parse("{%  %}"), Err(RenderError::Syntax { .. })));
    }

    // -- output and expressions -------------------------------------------

    #[test]
    fn an_output_tag_yields_an_output_node() -> TestResult {
        match one("{{ name }}")? {
            Node::Output { expr, line, column } => {
                assert_eq!((line, column), (1, 1));
                assert!(
                    matches!(expr, Expr::Variable { root, path } if root == "name" && path.is_empty())
                );
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected an Output, got {other:?}"))),
        }
    }

    #[test]
    fn a_dotted_and_bracketed_path_becomes_segments() -> TestResult {
        match one("{{ a.b-c['x'][0] }}")? {
            Node::Output { expr, .. } => {
                assert!(matches!(
                    expr,
                    Expr::Variable { root, path }
                        if root == "a"
                            && path
                                == vec![
                                    Segment::Key("b-c".to_owned()),
                                    Segment::Key("x".to_owned()),
                                    Segment::Index(0),
                                ]
                ));
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected an Output, got {other:?}"))),
        }
    }

    #[test]
    fn a_filter_pipeline_is_left_to_right() -> TestResult {
        match one("{{ a | upper | append: 'x' }}")? {
            Node::Output {
                expr: Expr::Filter { input, name, args },
                ..
            } => {
                assert_eq!(name, "append");
                assert!(matches!(args.as_slice(), [Expr::Literal(Value::String(s))] if s == "x"));
                assert!(
                    matches!(
                        input.as_ref(),
                        Expr::Filter { name: inner, args, .. }
                            if inner == "upper" && args.is_empty()
                    ),
                    "the inner filter should be `upper` with no arguments"
                );
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected a filter, got {other:?}"))),
        }
    }

    #[test]
    fn a_string_literal_keeps_its_escapes() -> TestResult {
        match one(r"{{ 'a\'b' }}")? {
            Node::Output {
                expr: Expr::Literal(Value::String(text)),
                ..
            } => {
                assert_eq!(text, "a'b");
                Ok(())
            }
            other => Err(syntax(
                0,
                0,
                format!("expected a string literal, got {other:?}"),
            )),
        }
    }

    #[test]
    fn the_literal_spellings_are_recognised() -> TestResult {
        for (input, expected) in [
            ("{{ true }}", Value::Bool(true)),
            ("{{ false }}", Value::Bool(false)),
            ("{{ nil }}", Value::Null),
        ] {
            match one(input)? {
                Node::Output {
                    expr: Expr::Literal(value),
                    ..
                } => assert_eq!(value, expected),
                other => return Err(syntax(0, 0, format!("expected a literal, got {other:?}"))),
            }
        }
        Ok(())
    }

    #[test]
    fn a_range_is_a_range_not_a_decimal() -> TestResult {
        match one("{{ (1..3) }}")? {
            Node::Output {
                expr: Expr::Range { from, to },
                ..
            } => {
                assert!(matches!(from.as_ref(), Expr::Literal(Value::Number(_))));
                assert!(matches!(to.as_ref(), Expr::Literal(Value::Number(_))));
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected a range, got {other:?}"))),
        }
    }

    #[test]
    fn and_binds_tighter_than_or() -> TestResult {
        // `a or b and c` must read as `a or (b and c)`.
        match one("{{ a or b and c }}")? {
            Node::Output {
                expr: Expr::Binary { op, rhs, .. },
                ..
            } => {
                assert_eq!(op, BinaryOp::Or);
                assert!(matches!(
                    rhs.as_ref(),
                    Expr::Binary {
                        op: BinaryOp::And,
                        ..
                    }
                ));
                Ok(())
            }
            other => Err(syntax(
                0,
                0,
                format!("expected a binary expression, got {other:?}"),
            )),
        }
    }

    #[test]
    fn a_unary_not_wraps_its_operand() -> TestResult {
        assert!(matches!(
            one("{{ not x }}")?,
            Node::Output {
                expr: Expr::Unary {
                    op: UnaryOp::Not,
                    ..
                },
                ..
            }
        ));
        Ok(())
    }

    // -- blocks ------------------------------------------------------------

    #[test]
    fn an_if_with_elsif_keeps_its_branches() -> TestResult {
        match one("{% if a %}1{% elsif b %}2{% elsif c %}3{% endif %}")? {
            Node::If {
                branches,
                otherwise,
                ..
            } => {
                assert_eq!(branches.len(), 3);
                assert!(otherwise.is_empty());
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected an If, got {other:?}"))),
        }
    }

    #[test]
    fn an_if_with_an_else_keeps_the_else_body() -> TestResult {
        match one("{% if a %}1{% else %}2{% endif %}")? {
            Node::If {
                branches,
                otherwise,
                ..
            } => {
                assert_eq!(branches.len(), 1);
                assert!(matches!(otherwise.as_slice(), [Node::Text(t)] if t == "2"));
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected an If, got {other:?}"))),
        }
    }

    #[test]
    fn a_for_with_an_else_keeps_the_else_body() -> TestResult {
        match one("{% for x in xs %}{{ x }}{% else %}none{% endfor %}")? {
            Node::For {
                variable,
                body,
                otherwise,
                ..
            } => {
                assert_eq!(variable, "x");
                assert_eq!(body.len(), 1);
                assert!(matches!(otherwise.as_slice(), [Node::Text(t)] if t == "none"));
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected a For, got {other:?}"))),
        }
    }

    #[test]
    fn a_for_needs_an_in_keyword() -> TestResult {
        assert!(matches!(
            parse("{% for x xs %}{% endfor %}"),
            Err(RenderError::Syntax { .. })
        ));
        Ok(())
    }

    #[test]
    fn an_unless_with_an_else_is_parsed() -> TestResult {
        match one("{% unless a %}1{% else %}2{% endunless %}")? {
            Node::Unless {
                body, otherwise, ..
            } => {
                assert_eq!(body.len(), 1);
                assert_eq!(otherwise.len(), 1);
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected an Unless, got {other:?}"))),
        }
    }

    #[test]
    fn a_case_with_two_whens_and_an_else() -> TestResult {
        match one("{% case x %}{% when 1, 2 %}low{% when 3 %}high{% else %}other{% endcase %}")? {
            Node::Case {
                whens, otherwise, ..
            } => {
                assert_eq!(whens.len(), 2);
                assert_eq!(otherwise.len(), 1);
                assert!(matches!(whens.first(), Some((values, _)) if values.len() == 2));
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected a Case, got {other:?}"))),
        }
    }

    #[test]
    fn an_assign_binds_a_plain_name() -> TestResult {
        match one("{% assign path = 'x' %}")? {
            Node::Assign { target, value, .. } => {
                assert_eq!(target, "path");
                assert!(matches!(value, Expr::Literal(Value::String(s)) if s == "x"));
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected an Assign, got {other:?}"))),
        }
    }

    #[test]
    fn an_assign_to_a_dotted_name_is_a_syntax_error() {
        assert!(matches!(
            parse("{% assign a.b = 'x' %}"),
            Err(RenderError::Syntax { .. })
        ));
    }

    #[test]
    fn a_capture_keeps_its_body() -> TestResult {
        match one("{% capture out %}{{ a }}{% endcapture %}")? {
            Node::Capture { target, body, .. } => {
                assert_eq!(target, "out");
                assert_eq!(body.len(), 1);
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected a Capture, got {other:?}"))),
        }
    }

    #[test]
    fn an_unknown_tag_becomes_a_tag_node() -> TestResult {
        match one("{% uuid 'v4' %}")? {
            Node::Tag {
                name,
                args,
                line,
                column,
            } => {
                assert_eq!(name, "uuid");
                assert_eq!((line, column), (1, 1));
                assert!(matches!(args.as_slice(), [Expr::Literal(Value::String(s))] if s == "v4"));
                Ok(())
            }
            other => Err(syntax(0, 0, format!("expected a Tag, got {other:?}"))),
        }
    }

    // -- raw ---------------------------------------------------------------

    #[test]
    fn a_raw_block_is_not_tokenised() -> TestResult {
        assert!(
            matches!(one("{% raw %}{{ x }}{% endraw %}")?, Node::Raw(body) if body == "{{ x }}")
        );
        Ok(())
    }

    #[test]
    fn a_raw_block_survives_comment_stripping() {
        // The original strips comments from the whole input first, so a `{#`
        // inside a raw block is eaten. This pins that behaviour rather than
        // trying to be cleverer than it.
        assert_eq!(
            strip_comments("{% raw %}{# x #}{% endraw %}"),
            "{% raw %}{% endraw %}"
        );
    }

    // -- expressions in isolation -----------------------------------------

    #[test]
    fn an_expression_with_trailing_junk_is_rejected() {
        assert!(matches!(
            parse("{{ a b }}"),
            Err(RenderError::Syntax { .. })
        ));
    }

    #[test]
    fn an_unterminated_string_is_a_syntax_error() {
        assert!(matches!(parse("{{ 'a }}"), Err(RenderError::Syntax { .. })));
    }

    #[test]
    fn a_keyword_argument_is_named_as_unsupported() -> TestResult {
        match parse_arguments("key: value", 1, 1) {
            Ok(args) => Err(syntax(0, 0, format!("expected a rejection, got {args:?}"))),
            Err(RenderError::Syntax { message, .. }) => {
                assert!(message.contains("keyword arguments"), "got `{message}`");
                Ok(())
            }
            Err(other) => Err(other),
        }
    }
}
