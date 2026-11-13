### Task 13: Request-level render and error paths

**Files:**
- Create: `crates/heric-template/src/render_request.rs`
- Modify: `crates/heric-template/src/lib.rs`
- Test: `#[cfg(test)]` in `render_request.rs`, plus `crates/heric-template/tests/render_golden.rs`

**Interfaces:**
- Produces: `heric_template::render_request(request: &mut Doc, context: &RenderContext<'_>, host: &dyn TagHost) -> Result<(), RenderError>`; `render_request::FIELDS: &[&str]`, the rendered fields in order.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn the_url_headers_and_body_are_rendered() -> Result<(), RenderError> { /* … */ }

#[test]
fn a_header_with_an_undefined_variable_fails_the_whole_request() -> Result<(), RenderError> {
    let mut doc = request_doc()?;
    let before = doc.clone();
    let error = render_request(&mut doc, &context(&Scope::new()), &NoHost);
    assert!(matches!(error, Err(RenderError::UndefinedVariable { .. })));
    assert_eq!(doc, before, "a failed render must leave the document unmodified");
    Ok(())
}

#[test]
fn an_error_path_names_the_field() -> Result<(), RenderError> {
    // TPL-030: the path reads `_request.headers[0].value`
    Ok(())
}

#[test]
fn only_a_manual_cookie_is_rendered() -> Result<(), RenderError> {
    // a {"source":"server"} cookie keeps its literal "{{"
    Ok(())
}

#[test]
fn a_description_is_rendered_at_lower_priority() -> Result<(), RenderError> { /* TPL-012 */ }
```

- [ ] **Step 2: Golden tests.** `tests/render_golden.rs` transcribes `FIX-011`'s 61 cases from `packages/insomnia/src/common/__tests__/render.test.ts`, including the gRPC variants, as a table of `(template, context, expected)`, keeping the original's `describe` names as module names. Transcribe the cases; never copy files at build time.

- [ ] **Step 3: Implement** `render_request` as all-or-nothing: render into a scratch copy, and write it back only when every field succeeded, so a partial render can never reach the wire. Render in `FIELDS` order — URL, path parameters, query parameters, headers, body text, authentication — and set `RenderError`'s `path` to the field path, so `TPL-030`'s `_request.headers[0].value` form comes out of the error the renderer already produces.

- [ ] **Step 4: Log a breadcrumb after five seconds.** `TPL-040` is a `tracing::warn!` emitted from a timer started around the render when it is still running at 5 s; it never aborts. The test asserts the event through a `tracing_subscriber` capture, not a wall-clock wait.

- [ ] **Step 5: Verify and commit** — `cargo test -p heric-template`, then `feat(template): render a request's fields atomically`

