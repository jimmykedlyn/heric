### Task 10: Evaluation, truthiness, limits and structured errors

**Files:**
- Create: `crates/heric-template/src/eval.rs`, `src/truthy.rs`
- Modify: `crates/heric-template/src/lib.rs`, `src/context.rs`
- Test: `#[cfg(test)]` in both

**Interfaces:**
- Produces: `heric_template::render(input, context, host) -> Result<String, RenderError>` (signature declared in Phase 1); `RenderContext::merged(&self) -> serde_json::Map<String, Value>`; `truthy::is_truthy(&Value) -> bool`; `pub(crate) mod testing { pub struct NoHost; pub fn context(&Scope) -> RenderContext<'_>; pub fn context_with(&[(&str, Value)]) -> RenderContext<'_>; }`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_root_variable_and_its_underscore_form_resolve_alike() -> Result<(), RenderError> {
    let scope = Scope::from_map(serde_json::Map::from_iter([("name".into(), serde_json::json!("kyle"))]));
    assert_eq!(render("{{ name }}", &context(&scope), &NoHost)?, "kyle");
    assert_eq!(render("{{ _.name }}", &context(&scope), &NoHost)?, "kyle");
    Ok(())
}

#[test]
fn a_bracket_key_reaches_a_hyphenated_name() -> Result<(), RenderError> { /* {{ _['water-type'] }} */ }

#[test]
fn an_undefined_variable_names_itself_and_its_line() -> Result<(), RenderError> {
    match render("line one\n{{ nope }}", &context(&Scope::new()), &NoHost) {
        Err(RenderError::UndefinedVariable { name, line, .. }) => {
            assert_eq!((name.as_str(), line), ("nope", 2));
            Ok(())
        }
        other => Err(format!("expected UndefinedVariable, got {other:?}").into()),
    }
}

#[test]
fn an_empty_array_and_an_empty_object_are_truthy() {
    assert!(truthy::is_truthy(&serde_json::json!([])));
    assert!(truthy::is_truthy(&serde_json::json!({})));
    for falsy in [serde_json::json!(""), serde_json::json!(0), serde_json::Value::Null, serde_json::json!(false)] {
        assert!(!truthy::is_truthy(&falsy), "{falsy}");
    }
}

#[test]
fn a_render_over_ten_thousand_steps_is_a_limit_error() { /* RenderError::Limit { what: "render" } */ }
#[test]
fn a_value_outside_the_context_fails_closed() { /* own property only, never a prototype chain */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** evaluation over `Node`, with JavaScript truthiness (`""`, `0`, `null`, `false` falsy; `[]` and `{}` **truthy** — the original follows JS, not Liquid's Ruby rules, and the test above pins it), own-property-only lookups with no prototype traversal, `strictVariables` on by default and `strictFilters` likewise (`TPL-001`), a render limit of 10 000 steps and a memory limit of 10 000 000 bytes reported as `RenderError::Limit { what: "render" | "memory" }`. `TPL-005` is satisfied by `RenderError`'s existing shape: every variant carries `path`, `line` and `column`, and `UndefinedVariable` carries `name`.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(template): evaluate Liquid with strict variables and structured errors`

