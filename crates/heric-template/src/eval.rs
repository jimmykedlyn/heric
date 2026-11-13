//! Evaluation: walks a parsed template and produces text.
//!
//! Strict variables and strict filters are both on, matching the original's
//! configuration: an undefined variable is an error the UI can offer to fix,
//! and an unknown filter is an error rather than a passthrough. Two limits
//! bound a render so a runaway template cannot hang the editor — a step budget
//! and an output budget.
//!
//! Filters are Task 12 and tags are Task 11. Until they land, a filter or a
//! named tag fails with an [`RenderError::Evaluation`] naming what is missing,
//! rather than silently rendering nothing.

use serde_json::{Map, Value};

use crate::ast::{BinaryOp, Expr, Node, Segment, UnaryOp};
use crate::context::RenderContext;
use crate::error::RenderError;
use crate::host::TagHost;
use crate::truthy::{is_truthy, to_output};

/// How many evaluation steps one render may take.
///
/// A step is charged for every node, every expression node, and every loop
/// iteration. Counting nodes alone would let a `{% for %}` over a long array
/// pass unnoticed, so each iteration pays too; the budget is meant to catch
/// runaway templates, not to bill work precisely.
pub(crate) const RENDER_STEP_LIMIT: u64 = 10_000;

/// How many bytes of output one render may produce.
pub(crate) const OUTPUT_BYTE_LIMIT: usize = 10_000_000;

/// The alias the original spreads the whole context under.
const ALIAS: &str = "_";

/// Evaluates `nodes` into a string.
///
/// # Errors
///
/// Returns [`RenderError::UndefinedVariable`] for a missing variable (strict
/// mode), [`RenderError::Evaluation`] for a type error or an unimplemented
/// construct, and [`RenderError::Limit`] when either budget is exceeded.
pub(crate) fn eval(
    nodes: &[Node],
    context: &RenderContext<'_>,
    host: &dyn TagHost,
) -> Result<String, RenderError> {
    let mut state = State {
        scope: context.merged(),
        _host: host,
        steps: 0,
        output: String::new(),
        locals: Vec::new(),
        position: (0, 0),
    };
    state.eval_nodes(nodes)?;
    Ok(state.output)
}

/// The mutable state of one render.
struct State<'a> {
    scope: Map<String, Value>,
    /// The host a tag will call. Task 11 reads it; the core evaluator does not.
    _host: &'a dyn TagHost,
    steps: u64,
    output: String,
    /// Loop variables and `assign`/`capture` bindings, innermost last.
    locals: Vec<Map<String, Value>>,
    /// Position of the construct being evaluated, so an error raised deep in an
    /// expression can name the tag it came from.
    position: (u32, u32),
}

impl State<'_> {
    /// Charges one step against the budget.
    fn step(&mut self) -> Result<(), RenderError> {
        self.steps = self.steps.saturating_add(1);
        if self.steps > RENDER_STEP_LIMIT {
            return Err(RenderError::Limit { what: "render" });
        }
        Ok(())
    }

    /// Appends to the output, enforcing the byte budget.
    fn push(&mut self, text: &str) -> Result<(), RenderError> {
        if self.output.len().saturating_add(text.len()) > OUTPUT_BYTE_LIMIT {
            return Err(RenderError::Limit { what: "memory" });
        }
        self.output.push_str(text);
        Ok(())
    }

    /// An evaluation error at the current position.
    fn evaluation(&self, message: String) -> RenderError {
        RenderError::Evaluation {
            path: String::new(),
            line: self.position.0,
            column: self.position.1,
            message,
        }
    }

    /// An undefined-variable error at the current position.
    fn undefined(&self, name: &str) -> RenderError {
        RenderError::UndefinedVariable {
            path: String::new(),
            line: self.position.0,
            column: self.position.1,
            name: name.to_owned(),
        }
    }

    fn eval_nodes(&mut self, nodes: &[Node]) -> Result<(), RenderError> {
        for node in nodes {
            self.eval_node(node)?;
        }
        Ok(())
    }

    fn eval_node(&mut self, node: &Node) -> Result<(), RenderError> {
        self.step()?;
        match node {
            Node::Text(text) | Node::Raw(text) => self.push(text),
            Node::Output { expr, line, column } => {
                self.position = (*line, *column);
                let value = self.eval_expr(expr)?;
                let text = to_output(&value);
                self.push(&text)
            }
            Node::Assign { target, value } => {
                let value = self.eval_expr(value)?;
                self.bind(target, value);
                Ok(())
            }
            Node::Capture { target, body } => {
                // Render the body into a scratch buffer, then restore the outer
                // output, so a `capture` never swallows the text around it.
                let saved = std::mem::take(&mut self.output);
                let result = self.eval_nodes(body);
                let captured = std::mem::replace(&mut self.output, saved);
                result?;
                self.bind(target, Value::String(captured));
                Ok(())
            }
            Node::If {
                branches,
                otherwise,
            } => {
                for (condition, body) in branches {
                    if is_truthy(&self.eval_expr(condition)?) {
                        return self.eval_nodes(body);
                    }
                }
                self.eval_nodes(otherwise)
            }
            Node::Unless {
                condition,
                body,
                otherwise,
            } => {
                if is_truthy(&self.eval_expr(condition)?) {
                    self.eval_nodes(otherwise)
                } else {
                    self.eval_nodes(body)
                }
            }
            Node::For {
                variable,
                iterable,
                body,
                otherwise,
            } => self.eval_for(variable, iterable, body, otherwise),
            Node::Case {
                scrutinee,
                whens,
                otherwise,
            } => {
                let value = self.eval_expr(scrutinee)?;
                for (candidates, body) in whens {
                    for candidate in candidates {
                        if self.eval_expr(candidate)? == value {
                            return self.eval_nodes(body);
                        }
                    }
                }
                self.eval_nodes(otherwise)
            }
            Node::Tag {
                name, line, column, ..
            } => {
                self.position = (*line, *column);
                Err(self.evaluation(format!("`{{% {name} %}}` is not implemented yet")))
            }
        }
    }

    fn eval_for(
        &mut self,
        variable: &str,
        iterable: &Expr,
        body: &[Node],
        otherwise: &[Node],
    ) -> Result<(), RenderError> {
        let value = self.eval_expr(iterable)?;
        // `for` walks an array's elements, an object's values and a range's
        // numbers. A string is not iterable in the original.
        let items: Vec<Value> = match value {
            Value::Array(items) => items,
            Value::Object(map) => map.into_values().collect(),
            other => {
                return Err(self.evaluation(format!("`{{% for %}}` cannot iterate over {other}")));
            }
        };
        if items.is_empty() {
            return self.eval_nodes(otherwise);
        }
        for item in items {
            self.step()?;
            let mut frame = Map::new();
            frame.insert(variable.to_owned(), item);
            self.locals.push(frame);
            let result = self.eval_nodes(body);
            self.locals.pop();
            result?;
        }
        Ok(())
    }

    /// Binds a name for the rest of the render.
    ///
    /// The innermost frame receives it, so an `assign` inside a `for` does not
    /// leak past the loop.
    fn bind(&mut self, target: &str, value: Value) {
        match self.locals.last_mut() {
            Some(frame) => {
                frame.insert(target.to_owned(), value);
            }
            None => {
                let mut frame = Map::new();
                frame.insert(target.to_owned(), value);
                self.locals.push(frame);
            }
        }
    }

    /// Looks a root name up, innermost scope first.
    fn lookup(&self, name: &str) -> Option<&Value> {
        for frame in self.locals.iter().rev() {
            if let Some(value) = frame.get(name) {
                return Some(value);
            }
        }
        if name == ALIAS {
            // `_` is the whole merged context, not a key inside it.
            return None;
        }
        self.scope.get(name)
    }

    fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RenderError> {
        self.step()?;
        match expr {
            Expr::Literal(value) => Ok(value.clone()),
            Expr::Variable { root, path } => self.resolve(root, path),
            Expr::Range { from, to } => {
                let from_value = self.eval_expr(from)?;
                let from = self.numeric(&from_value, "range start")?;
                let to_value = self.eval_expr(to)?;
                let to = self.numeric(&to_value, "range end")?;
                // An inclusive range, as Liquid documents it. A descending
                // range is empty rather than reversed.
                let mut values = Vec::new();
                let mut current = from;
                while current <= to {
                    values.push(Value::from(current));
                    current = current.saturating_add(1);
                }
                Ok(Value::Array(values))
            }
            Expr::Unary { op, operand } => {
                let value = self.eval_expr(operand)?;
                match op {
                    UnaryOp::Not => Ok(Value::Bool(!is_truthy(&value))),
                    UnaryOp::Negate => {
                        let number = self.numeric(&value, "negation operand")?;
                        Ok(integer_value(-number))
                    }
                }
            }
            Expr::Binary { op, lhs, rhs } => self.eval_binary(*op, lhs, rhs),
            Expr::Filter { name, .. } => {
                Err(self.evaluation(format!("filter `{name}` is not implemented yet")))
            }
        }
    }

    /// Resolves a variable reference to a value, or fails in strict mode.
    fn resolve(&self, root: &str, path: &[Segment]) -> Result<Value, RenderError> {
        // `_.name` has root `_`, so the alias collapses to "look in the merged
        // scope", exactly as the original's double spread does.
        let (mut current, remaining) = if root == ALIAS {
            // `_` is the merged scope itself, so every segment of the path
            // indexes into it: both `_.name` and the bracket form for a
            // hyphenated key work, and a bare `_` renders the whole map.
            (Value::Object(self.scope.clone()), path)
        } else {
            let Some(value) = self.lookup(root) else {
                return Err(self.undefined(root));
            };
            (value.clone(), path)
        };
        for segment in remaining {
            // Own properties only. JSON has no prototype chain, but a data key
            // named `__proto__` or `constructor` must stay unreachable, which
            // is what the original's `hasOwnProperty` guard achieves.
            if is_forbidden(key_of(segment)) {
                return Err(self.undefined(&path_name(root, path)));
            }
            let next = match segment {
                Segment::Key(key) => match &current {
                    Value::Object(map) => map.get(key),
                    _ => None,
                },
                Segment::Index(index) => match &current {
                    Value::Array(items) => items.get(*index),
                    _ => None,
                },
            };
            let Some(value) = next else {
                return Err(self.undefined(&path_name(root, path)));
            };
            current = value.clone();
        }
        Ok(current)
    }

    fn eval_binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr) -> Result<Value, RenderError> {
        match op {
            // Short-circuit before evaluating the right operand, which is what
            // keeps `x and x.y` from failing when `x` is absent.
            BinaryOp::And => {
                let left = self.eval_expr(lhs)?;
                if !is_truthy(&left) {
                    return Ok(Value::Bool(false));
                }
                let right = self.eval_expr(rhs)?;
                Ok(Value::Bool(is_truthy(&right)))
            }
            BinaryOp::Or => {
                let left = self.eval_expr(lhs)?;
                if is_truthy(&left) {
                    return Ok(Value::Bool(true));
                }
                let right = self.eval_expr(rhs)?;
                Ok(Value::Bool(is_truthy(&right)))
            }
            _ => {
                let left = self.eval_expr(lhs)?;
                let right = self.eval_expr(rhs)?;
                match op {
                    BinaryOp::Eq => Ok(Value::Bool(left == right)),
                    BinaryOp::Ne => Ok(Value::Bool(left != right)),
                    BinaryOp::Lt => self.compare(&left, &right).map(|o| Value::Bool(o.is_lt())),
                    BinaryOp::Le => self.compare(&left, &right).map(|o| Value::Bool(o.is_le())),
                    BinaryOp::Gt => self.compare(&left, &right).map(|o| Value::Bool(o.is_gt())),
                    BinaryOp::Ge => self.compare(&left, &right).map(|o| Value::Bool(o.is_ge())),
                    BinaryOp::Contains => Ok(Value::Bool(contains(&left, &right))),
                    BinaryOp::And | BinaryOp::Or => Ok(Value::Null),
                }
            }
        }
    }

    /// Orders two values, which Liquid only defines over numbers.
    fn compare(&self, left: &Value, right: &Value) -> Result<std::cmp::Ordering, RenderError> {
        match (left.as_f64(), right.as_f64()) {
            (Some(left), Some(right)) => left
                .partial_cmp(&right)
                .ok_or_else(|| self.evaluation("cannot compare incomparable numbers".to_owned())),
            _ => Err(self.evaluation(format!("cannot compare {left} with {right}"))),
        }
    }

    /// A value as a number, or an evaluation error naming the context.
    fn numeric(&self, value: &Value, what: &str) -> Result<i64, RenderError> {
        let Some(number) = value.as_f64() else {
            return Err(self.evaluation(format!("{what} must be a number, got {value}")));
        };
        // A range bound and a negation are integers in every real template, so
        // a fractional bound is reported rather than truncated silently.
        if number.fract() != 0.0 {
            return Err(self.evaluation(format!("{what} must be a whole number, got {number}")));
        }
        format!("{number:.0}").parse::<i64>().map_err(|_| {
            self.evaluation(format!(
                "{what} is not an integer this engine can represent"
            ))
        })
    }
}

/// Depth-independent name for an error message.
fn path_name(root: &str, path: &[Segment]) -> String {
    let mut name = root.to_owned();
    for segment in path {
        match segment {
            Segment::Key(key) => {
                name.push('.');
                name.push_str(key);
            }
            Segment::Index(index) => {
                name.push('[');
                name.push_str(&index.to_string());
                name.push(']');
            }
        }
    }
    name
}

/// Keys a template may never read.
fn is_forbidden(key: Option<&str>) -> bool {
    matches!(key, Some("__proto__" | "constructor" | "prototype"))
}

/// The key a segment names, when it names one.
fn key_of(segment: &Segment) -> Option<&str> {
    match segment {
        Segment::Key(key) => Some(key),
        Segment::Index(_) => None,
    }
}

/// A numeric result as a JSON value.
fn integer_value(value: i64) -> Value {
    Value::from(value)
}

/// Liquid's `contains`: a substring for strings, a membership test for arrays.
fn contains(haystack: &Value, needle: &Value) -> bool {
    match haystack {
        Value::String(text) => match needle {
            Value::String(fragment) => text.contains(fragment.as_str()),
            other => text.contains(to_output(other).as_str()),
        },
        Value::Array(items) => items.contains(needle),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::*;
    use crate::context::Scope;
    use crate::testing::{NoHost, context, context_with};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// Renders with a local scope.
    fn render_with(input: &str, scope: &Scope) -> Result<String, RenderError> {
        crate::render(input, &context(scope), &NoHost)
    }

    /// A scope from `(name, value)` pairs.
    fn scope_of(pairs: &[(&str, Value)]) -> Scope {
        let mut variables = Map::new();
        for (name, value) in pairs {
            variables.insert((*name).to_owned(), value.clone());
        }
        Scope::from_map(variables)
    }

    // -- the plan's tests --------------------------------------------------

    #[test]
    fn a_root_variable_and_its_underscore_form_resolve_alike() -> TestResult {
        let scope = scope_of(&[("name", json!("kyle"))]);
        assert_eq!(render_with("{{ name }}", &scope)?, "kyle");
        assert_eq!(render_with("{{ _.name }}", &scope)?, "kyle");
        Ok(())
    }

    #[test]
    fn a_bracket_key_reaches_a_hyphenated_name() -> TestResult {
        let scope = scope_of(&[("water-type", json!("salt"))]);
        // A hyphenated name cannot be written as a dotted path, so it needs the
        // bracket form. `_` is the merged scope, so both spellings reach it.
        assert_eq!(render_with("{{ _['water-type'] }}", &scope)?, "salt");
        // A hyphen is part of an identifier here, exactly as in the original,
        // so the dotted-name spelling is not a subtraction.
        assert_eq!(render_with("{{ water-type }}", &scope)?, "salt");
        Ok(())
    }

    #[test]
    fn an_undefined_variable_names_itself_and_its_line() {
        assert!(
            matches!(
                crate::render(
                    "line one\n{{ nope }}",
                    &context_with(&[("something", json!(1))]),
                    &NoHost
                ),
                Err(RenderError::UndefinedVariable { ref name, line: 2, .. }) if name == "nope"
            ),
            "expected an UndefinedVariable naming `nope` on line 2"
        );
    }

    #[test]
    fn an_empty_array_and_an_empty_object_are_truthy() {
        assert!(is_truthy(&json!([])));
        assert!(is_truthy(&json!({})));
        for falsy in [json!(""), json!(0), Value::Null, json!(false)] {
            assert!(!is_truthy(&falsy), "{falsy}");
        }
    }

    #[test]
    fn a_render_over_ten_thousand_steps_is_a_limit_error() {
        // A range of 10 001 numbers, each iterated, charged a step for the
        // iteration and another for the body's output node.
        let scope = scope_of(&[("many", (0..10_001).collect::<Vec<i64>>().into())]);
        assert!(
            matches!(
                render_with("{% for x in many %}{{ x }}{% endfor %}", &scope),
                Err(RenderError::Limit { what: "render" })
            ),
            "expected a render limit error"
        );
    }

    #[test]
    fn a_value_outside_the_context_fails_closed() -> TestResult {
        // `__proto__` is an own key here, and must still be unreachable: the
        // original's own-property guard is what keeps a template out of a
        // prototype chain, and a migrated document must not be able to reach it
        // either.
        let scope = scope_of(&[("obj", json!({"__proto__": "leaked"}))]);
        assert!(
            matches!(
                render_with("{{ obj.__proto__ }}", &scope),
                Err(RenderError::UndefinedVariable { .. })
            ),
            "expected an undefined variable"
        );
        Ok(())
    }

    // -- control flow ------------------------------------------------------

    #[test]
    fn an_if_picks_the_first_truthy_branch() -> TestResult {
        let scope = scope_of(&[("n", json!(2))]);
        assert_eq!(
            render_with(
                "{% if n == 1 %}one{% elsif n == 2 %}two{% else %}?{% endif %}",
                &scope
            )?,
            "two"
        );
        Ok(())
    }

    #[test]
    fn an_else_runs_when_no_branch_matches() -> TestResult {
        let scope = scope_of(&[("n", json!(9))]);
        assert_eq!(
            render_with("{% if n == 1 %}one{% else %}other{% endif %}", &scope)?,
            "other"
        );
        Ok(())
    }

    #[test]
    fn an_unless_inverts_its_condition() -> TestResult {
        let scope = scope_of(&[("off", json!(false))]);
        assert_eq!(
            render_with("{% unless off %}on{% endunless %}", &scope)?,
            "on"
        );
        Ok(())
    }

    #[test]
    fn a_for_concatenates_its_body() -> TestResult {
        let scope = scope_of(&[("xs", json!(["a", "b", "c"]))]);
        assert_eq!(
            render_with("{% for x in xs %}{{ x }}{% endfor %}", &scope)?,
            "abc"
        );
        Ok(())
    }

    #[test]
    fn a_for_over_an_empty_array_runs_its_else() -> TestResult {
        let scope = scope_of(&[("xs", json!([]))]);
        assert_eq!(
            render_with("{% for x in xs %}{{ x }}{% else %}none{% endfor %}", &scope)?,
            "none"
        );
        Ok(())
    }

    #[test]
    fn a_for_over_a_range_counts_inclusively() -> TestResult {
        assert_eq!(
            render_with("{% for i in (1..3) %}{{ i }}{% endfor %}", &Scope::new())?,
            "123"
        );
        Ok(())
    }

    #[test]
    fn a_case_picks_the_matching_when() -> TestResult {
        let scope = scope_of(&[("kind", json!(3))]);
        assert_eq!(
            render_with(
                "{% case kind %}{% when 1, 2 %}low{% when 3 %}high{% else %}?{% endcase %}",
                &scope
            )?,
            "high"
        );
        Ok(())
    }

    #[test]
    fn an_assign_binds_a_name_for_the_rest_of_the_render() -> TestResult {
        assert_eq!(
            render_with("{% assign who = 'kyle' %}{{ who }}", &Scope::new())?,
            "kyle"
        );
        Ok(())
    }

    #[test]
    fn a_capture_binds_the_rendered_body() -> TestResult {
        assert_eq!(
            render_with(
                "{% capture out %}a{{ 'b' }}c{% endcapture %}[{{ out }}]",
                &Scope::new()
            )?,
            "[abc]"
        );
        Ok(())
    }

    #[test]
    fn an_assign_inside_a_loop_does_not_escape_it() -> TestResult {
        let scope = scope_of(&[("xs", json!([1, 2]))]);
        assert!(
            matches!(
                render_with("{% for x in xs %}{% assign y = x %}{% endfor %}{{ y }}", &scope),
                Err(RenderError::UndefinedVariable { ref name, .. }) if name == "y"
            ),
            "`y` should not be visible after the loop"
        );
        Ok(())
    }

    // -- operators ---------------------------------------------------------

    #[test]
    fn comparisons_and_equality_work_on_numbers_and_strings() -> TestResult {
        let scope = scope_of(&[("n", json!(5)), ("s", json!("abc"))]);
        assert_eq!(render_with("{{ n > 3 }}", &scope)?, "true");
        assert_eq!(render_with("{{ n <= 5 }}", &scope)?, "true");
        assert_eq!(render_with("{{ s == 'abc' }}", &scope)?, "true");
        assert_eq!(render_with("{{ s != 'abc' }}", &scope)?, "false");
        Ok(())
    }

    #[test]
    fn and_and_or_short_circuit() -> TestResult {
        // The right operand is undefined; short-circuiting must keep it from
        // failing.
        let scope = scope_of(&[("nothing", Value::Null)]);
        assert_eq!(render_with("{{ nothing and nope }}", &scope)?, "false");
        assert_eq!(render_with("{{ 'x' or nope }}", &scope)?, "true");
        Ok(())
    }

    #[test]
    fn contains_tests_a_substring_and_an_array_member() -> TestResult {
        let scope = scope_of(&[("s", json!("hello")), ("xs", json!([1, 2]))]);
        assert_eq!(render_with("{{ s contains 'ell' }}", &scope)?, "true");
        assert_eq!(render_with("{{ xs contains 2 }}", &scope)?, "true");
        assert_eq!(render_with("{{ xs contains 3 }}", &scope)?, "false");
        Ok(())
    }

    #[test]
    fn a_negative_number_is_negated() -> TestResult {
        assert_eq!(render_with("{{ -3 }}", &Scope::new())?, "-3");
        Ok(())
    }

    // -- output ------------------------------------------------------------

    #[test]
    fn null_renders_as_nothing() -> TestResult {
        let scope = scope_of(&[("nothing", Value::Null)]);
        assert_eq!(render_with("[{{ nothing }}]", &scope)?, "[]");
        Ok(())
    }

    #[test]
    fn a_template_without_tags_is_returned_untouched() -> TestResult {
        assert_eq!(render_with("plain text", &Scope::new())?, "plain text");
        Ok(())
    }

    #[test]
    fn a_raw_block_renders_its_body_verbatim() -> TestResult {
        assert_eq!(
            render_with("{% raw %}{{ x }}{% endraw %}", &Scope::new())?,
            "{{ x }}"
        );
        Ok(())
    }

    #[test]
    fn the_output_budget_is_enforced() {
        // Ten million one-byte appends, each its own node: the memory limit
        // fires before the step limit can.
        let mut template = String::new();
        for _ in 0..10_000_001 {
            template.push('a');
        }
        // A single text node larger than the budget is the cheapest way to
        // reach the limit without a ten-million-node parse.
        assert!(
            matches!(
                render_with(
                    &format!("{{{{ x }}}}{template}"),
                    &scope_of(&[("x", json!(""))])
                ),
                Err(RenderError::Limit { what: "memory" })
            ),
            "expected a memory limit error"
        );
    }

    #[test]
    fn an_unknown_filter_is_an_evaluation_error() {
        assert!(
            matches!(
                render_with("{{ 'a' | nope }}", &Scope::new()),
                Err(RenderError::Evaluation { .. })
            ),
            "expected an evaluation error"
        );
    }

    #[test]
    fn an_unknown_tag_is_an_evaluation_error() {
        assert!(
            matches!(
                render_with("{% uuid 'v4' %}", &Scope::new()),
                Err(RenderError::Evaluation { .. })
            ),
            "expected an evaluation error"
        );
    }

    // -- layers ------------------------------------------------------------

    #[test]
    fn a_later_layer_wins_over_an_earlier_one() {
        let (global, collection, folder) = (
            scope_of(&[("winner", json!("global"))]),
            scope_of(&[("winner", json!("collection"))]),
            scope_of(&[("winner", json!("folder"))]),
        );
        let context = crate::context::RenderContext {
            global_root: &global,
            global_sub: None,
            collection_root: &collection,
            collection_sub: None,
            folder_ancestors: &[folder],
            iteration: None,
            transient: None,
            policy: crate::context::ConfidentialPolicy::Reveal,
            purpose: crate::purpose::RenderPurpose::Preview,
            request_id: None,
        };
        assert_eq!(
            crate::render("{{ winner }}", &context, &NoHost)
                .ok()
                .as_deref(),
            Some("folder")
        );
    }
}
