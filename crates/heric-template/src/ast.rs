//! The template syntax tree.
//!
//! One node per template construct, with no evaluation logic: Task 10 walks
//! this tree and Task 11 evaluates the tags. Keeping the two apart is what lets
//! the parser be tested without a context or a host.

use serde_json::Value;

/// A step in a variable path: `a.b-c[0].d` is
/// `[Key("a"), Key("b-c"), Index(0), Key("d")]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    /// A dotted or bracketed string key.
    Key(String),
    /// A numeric bracket index.
    Index(usize),
}

/// A value-producing expression.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Expr {
    /// A string, number, boolean or `nil` written in the template.
    Literal(Value),
    /// A variable lookup: a root name plus an optional path.
    Variable {
        /// The bare name before the first `.` or `[`.
        root: String,
        /// The path walked from the root, empty for a bare name.
        path: Vec<Segment>,
    },
    /// A filter application, `input | name: arg, arg`.
    Filter {
        /// The value the filter is applied to.
        input: Box<Self>,
        /// The filter's name.
        name: String,
        /// The filter's positional arguments.
        args: Vec<Self>,
    },
    /// An inclusive range, `from..to`.
    Range {
        /// The first bound.
        from: Box<Self>,
        /// The last bound.
        to: Box<Self>,
    },
    /// A prefix operator application.
    Unary {
        /// Which operator.
        op: UnaryOp,
        /// The operand.
        operand: Box<Self>,
    },
    /// An infix operator application.
    Binary {
        /// Which operator.
        op: BinaryOp,
        /// The left operand.
        lhs: Box<Self>,
        /// The right operand.
        rhs: Box<Self>,
    },
}

/// A prefix operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnaryOp {
    /// `not <expr>`.
    Not,
    /// `-<expr>`.
    Negate,
}

/// An infix operator, loosest binding first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinaryOp {
    /// `==`.
    Eq,
    /// `!=`.
    Ne,
    /// `<`.
    Lt,
    /// `<=`.
    Le,
    /// `>`.
    Gt,
    /// `>=`.
    Ge,
    /// `and`.
    And,
    /// `or`.
    Or,
    /// `contains`.
    Contains,
}

/// One node of a parsed template.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Node {
    /// Literal text, emitted as-is.
    Text(String),
    /// A `{{ … }}` interpolation. The position is that of the opening `{{`,
    /// because Task 10 reports undefined variables against it.
    Output {
        /// What to render.
        expr: Expr,
        /// Line of the opening `{{`, 1-based.
        line: u32,
        /// Column of the opening `{{`, 1-based.
        column: u32,
    },
    /// A `{% … %}` tag with no block structure. Task 11 dispatches on `name`.
    Tag {
        /// The tag's name, the first word inside the delimiters.
        name: String,
        /// The tag's positional arguments.
        args: Vec<Expr>,
        /// Line of the opening `{%`, 1-based.
        line: u32,
        /// Column of the opening `{%`, 1-based.
        column: u32,
    },
    /// `{% if %}` with any number of `{% elsif %}` and an optional `{% else %}`.
    If {
        /// The `if` condition followed by each `elsif` condition, in source
        /// order, each with its body.
        branches: Vec<(Expr, Vec<Self>)>,
        /// The `{% else %}` body, empty when absent.
        otherwise: Vec<Self>,
        /// Line of the opening `{%`, 1-based.
        line: u32,
        /// Column of the opening `{%`, 1-based.
        column: u32,
    },
    /// `{% for %}` with an optional `{% else %}`.
    For {
        /// The loop variable's name.
        variable: String,
        /// What to iterate.
        iterable: Expr,
        /// The loop body.
        body: Vec<Self>,
        /// The `{% else %}` body, run when the iterable is empty.
        otherwise: Vec<Self>,
        /// Line of the opening `{%`, 1-based.
        line: u32,
        /// Column of the opening `{%`, 1-based.
        column: u32,
    },
    /// `{% unless %}` with an optional `{% else %}`.
    Unless {
        /// The condition that must be false for the body to run.
        condition: Expr,
        /// The body.
        body: Vec<Self>,
        /// The `{% else %}` body.
        otherwise: Vec<Self>,
        /// Line of the opening `{%`, 1-based.
        line: u32,
        /// Column of the opening `{%`, 1-based.
        column: u32,
    },
    /// `{% case %}` with `{% when %}` arms and an optional `{% else %}`.
    Case {
        /// The value compared against each `when`.
        scrutinee: Expr,
        /// The `when` value list and body for each arm, in source order.
        whens: Vec<(Vec<Expr>, Vec<Self>)>,
        /// The `{% else %}` body.
        otherwise: Vec<Self>,
        /// Line of the opening `{%`, 1-based.
        line: u32,
        /// Column of the opening `{%`, 1-based.
        column: u32,
    },
    /// `{% assign name = expr %}`.
    Assign {
        /// The variable being bound.
        target: String,
        /// The value bound to it.
        value: Expr,
        /// Line of the opening `{%`, 1-based.
        line: u32,
        /// Column of the opening `{%`, 1-based.
        column: u32,
    },
    /// `{% capture name %}…{% endcapture %}`.
    Capture {
        /// The variable that receives the rendered body.
        target: String,
        /// The body whose rendering is captured.
        body: Vec<Self>,
        /// Line of the opening `{%`, 1-based.
        line: u32,
        /// Column of the opening `{%`, 1-based.
        column: u32,
    },
    /// The verbatim body of a `{% raw %}` block. It is never re-lexed, so any
    /// delimiters it contains stay literal.
    Raw(String),
}
