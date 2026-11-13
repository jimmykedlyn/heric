### Task 11: Filters, the tag argument tokenizer and the built-in tags

**Files:**
- Create: `crates/heric-template/src/filters.rs`, `src/tags/mod.rs`
- Modify: `crates/heric-template/src/tag.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `filters::apply(name: &str, value: &Value, args: &[Value]) -> Result<Value, RenderError>`; `tags::run(name: &str, args: &[Value], context: &mut RenderContext<'_>, host: &dyn TagHost) -> Result<String, TagError>`; `pub(crate) fn tokenize_args(input: &str) -> Result<Vec<Expr>, RenderError>`.

- [ ] **Step 1: Write the failing tests** — one per filter the original registers, transcribed from `docs/inventory/templating.md` § Custom filters (do not invent a name or a behaviour); `a_missing_comma_merges_the_remaining_arguments`, which pins the original's `tokenize-args` behaviour and is what `TPL-006` means by "quoted strings with escapes, booleans, numbers, variables and expressions"; one test per entry in `tag::tags()`: `base64`, `cookie`, `faker`, `file`, `hash`, `jsonpath`, `now`, `os`, `prompt`, `request`, `response`, `timestamp`, `uuid`. `{% prompt %}` returns `TagError::NotAvailableInThisPurpose` when `!context.purpose.allows_blocking_tags()`, and every tag stringifies with `null` becoming `""` (`TPL-014`).
- [ ] **Step 2–5:** verify failure, implement, verify `cargo test -p heric-template`, commit `feat(template): add filters, tag arguments and the built-in tags`.

