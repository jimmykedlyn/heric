### Task 12: Layered context, three-pass render and confidentiality

**Files:**
- Create: `crates/heric-template/src/render_context.rs`, `src/render_tree.rs`
- Modify: `crates/heric-template/src/context.rs`, `src/lib.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `Layer { scope: Scope, source: LayerSource }` with `LayerSource::{Global, GlobalSub, Collection, CollectionSub, Folder, Iteration, Transient}`; `RenderContext::build(layers: &[Layer]) -> OwnedContext`; `render_tree::render_value(value: &Value, context: &OwnedContext, host: &dyn TagHost, path: &str) -> Result<Value, RenderError>`; `render_tree::second_pass(original: &str, rendered: &str, …) -> Result<String, RenderError>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_later_layer_wins_over_an_earlier_one() { /* Global then Collection, same key */ }

#[test]
fn a_dangerous_key_is_skipped() {
    for key in ["__proto__", "constructor", "prototype"] {
        // merged map must not contain key even when a layer supplies it
    }
}

#[test]
fn a_self_recursive_variable_resolves_once() -> Result<(), RenderError> {
    // base_url = "{{ base_url }}/foo" over base_url = "google.com"
    // expected rendered base_url == "google.com/foo", with no further expansion
    Ok(())
}

#[test]
fn a_template_valued_key_sorts_after_plain_keys() { /* TPL-009 ordering */ }

#[test]
fn an_unchanged_value_is_not_re_rendered() { /* skipNextTime */ }

#[test]
fn a_second_pass_runs_only_when_the_input_had_no_tags() -> Result<(), RenderError> {
    // original "{{ body }}" with a body containing "{{ x }}" must NOT expand x
    let out = render("{{ body }}", &context_with(&[("body", serde_json::json!("{{ x }}")), ("x", serde_json::json!("LEAK"))]), &NoHost)?;
    assert_eq!(out, "{{ x }}");
    Ok(())
}

#[test]
fn a_disabled_row_is_skipped_entirely() { /* {"name":"A","value":"1","disabled":true} */ }
#[test]
fn a_blacklisted_path_is_left_verbatim() { /* TPL-010's blacklist regex */ }
#[test]
fn a_masked_value_is_never_re_materialised() { /* ConfidentialPolicy::Mask before substitution */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** the seven-layer merge in increasing precedence with JSON property order preserved, the `__proto__`/`constructor`/`prototype` skip, template-last key ordering, and the three-pass loop with `skipNextTime` (`TPL-009`). `second_pass` runs only when the first output gained `{%` **and** the original input carried no tags (`TPL-011`) — this is Review Focus 3's guard, and the test above is its pin. Masking happens at merge time, before any substitution, using `ConfidentialPolicy::for_purpose` (already implemented in Phase 1 — use it, do not re-derive it), with the mask `••••••` (`TPL-015`).

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(template): layer environments and render in three passes`

