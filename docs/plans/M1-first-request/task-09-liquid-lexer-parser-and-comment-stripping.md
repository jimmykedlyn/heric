### Task 9: Liquid lexer, parser and comment stripping

**Files:**
- Create: `crates/heric-template/src/lexer.rs`, `src/parser.rs`, `src/ast.rs`
- Modify: `crates/heric-template/src/lib.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `pub(crate) enum Node { Text(String), Output(Expr), Tag { name: String, args: Vec<Expr>, line: u32 }, If { … }, For { … }, Unless { … }, Case { … }, Assign { … }, Capture { … }, Raw(String) }`; `pub(crate) fn parse(input: &str) -> Result<Vec<Node>, RenderError>`; `pub(crate) fn strip_comments(input: &str) -> Cow<'_, str>`; `pub(crate) fn has_tags(input: &str) -> bool`.

**Parser choice, decided here (design §22 leaves it to the builder):** hand-written recursive descent, incrementally, driven by the conformance suite. No parser-combinator crate enters the tree until a case proves this approach cannot express a construct.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_multiline_comment_is_removed_before_parsing() {
    assert_eq!(strip_comments("a{# one\ntwo #}b"), "ab");
}

#[test]
fn a_hash_brace_inside_a_comment_ends_it() {
    // non-greedy, quote-blind, exactly as the original's regex behaves
    assert_eq!(strip_comments("{# 'a #}' tail"), " tail");
}

#[test]
fn input_without_delimiters_is_returned_untouched() {
    assert!(!has_tags("plain {%- not really -%}"));
    assert!(!has_tags("plain text"));
    assert!(has_tags("{{ name }}"));
    assert!(has_tags("{# #}"));
}

#[test]
fn text_without_delimiters_is_one_text_node() -> Result<(), RenderError> {
    assert!(matches!(parse("plain text")?.as_slice(), [Node::Text(t)] if t == "plain text"));
    Ok(())
}

#[test]
fn an_unclosed_output_tag_is_a_syntax_error() -> Result<(), RenderError> {
    match parse("{{ name") {
        Err(RenderError::Syntax { line, column, .. }) => {
            assert_eq!((line, column), (1, 1));
            Ok(())
        }
        other => Err(format!("expected a syntax error, got {other:?}").into()),
    }
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** the lexer (`{{ }}`, `{% %}`, `{# #}`, `{% raw %}`), the recursive-descent parser with 1-based line and column threaded into every node and every error, and `strip_comments`, which applies the original's non-greedy, quote-blind `{#…#}` removal (`TPL-002`) before tokenising. `has_tags` implements `TPL-003`'s short-circuit test and is what `render` consults first.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(template): add the Liquid lexer, parser and comment stripping`

