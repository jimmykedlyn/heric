### Task 5: Model defaults, `initModel` and the kind registry

**Files:**
- Create: `crates/heric-model/src/init.rs`, `crates/heric-model/src/registry.rs`
- Modify: `crates/heric-model/src/lib.rs`, `src/doc.rs`
- Test: `#[cfg(test)]` in both, plus `crates/heric-model/tests/v5_parser.rs`

**Interfaces:**
- Produces: `init::document(kind: DocKind, now: i64) -> Doc` (a fresh document with a generated id); `init::scratchpad() -> Vec<Doc>` (the three sentinel documents, fixed ids and names); `init::apply(doc: &mut Doc) -> Result<(), ModelError>`; `registry::{can_duplicate(DocKind) -> bool, rewrite_references(&mut Doc, &BTreeMap<Id, Id>), kinds_that_sync() -> &'static [DocKind]}`; constants `SCRATCHPAD_WORKSPACE`, `SCRATCHPAD_PROJECT`, `SCRATCHPAD_ORGANIZATION`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_new_request_gets_get_and_an_empty_body() -> Result<(), ModelError> {
    let doc = init::document(DocKind::Request, 1_000);
    assert_eq!(doc.body.get("method"), Some(&serde_json::json!("GET")));
    assert_eq!(doc.body.get("headers"), Some(&serde_json::json!([])));
    assert_eq!(doc.body.get("authentication"), Some(&serde_json::json!({})));
    assert_eq!(doc.meta.private, Some(false));
    assert_eq!(doc.body.get("metaSortKey"), Some(&serde_json::json!(-1_000.0)));
    assert!(doc.meta.id.as_str().starts_with("req_"));
    Ok(())
}

#[test]
fn pruning_drops_an_undeclared_key_and_keeps_a_declared_optional_one() -> Result<(), ModelError> {
    let mut doc = init::document(DocKind::Request, 0);
    doc.body.insert("notAModelField".into(), serde_json::json!(1));
    init::apply(&mut doc)?;
    assert!(!doc.body.contains_key("notAModelField"));
    Ok(())
}

#[test]
fn an_unmodelled_kind_keeps_every_key_it_was_given() -> Result<(), ModelError> {
    let mut doc = init::document(DocKind::MockRoute, 0);
    doc.body.insert("whatever".into(), serde_json::json!(1));
    init::apply(&mut doc)?;
    assert_eq!(doc.body.get("whatever"), Some(&serde_json::json!(1)));
    Ok(())
}

#[test]
fn the_scratchpad_uses_the_original_sentinel_ids() -> Result<(), ModelError> {
    let docs = init::scratchpad();
    let ids: Vec<&str> = docs.iter().map(|doc| doc.meta.id.as_str()).collect();
    assert_eq!(ids, [SCRATCHPAD_PROJECT, SCRATCHPAD_WORKSPACE, SCRATCHPAD_ORGANIZATION]);
    let workspace = docs.iter().find(|doc| doc.meta.kind == DocKind::Workspace).ok_or("no workspace")?;
    assert_eq!(workspace.meta.parent.as_ref().map(Id::as_str), Some(SCRATCHPAD_PROJECT));
    Ok(())
}

#[test]
fn a_fresh_workspace_gets_a_generated_id_not_the_scratchpad_one() {
    assert_ne!(init::document(DocKind::Workspace, 0).meta.id.as_str(), SCRATCHPAD_WORKSPACE);
}

#[test]
fn duplicating_a_request_rewrites_the_parent_reference() -> Result<(), ModelError> {
    // rewrite_references maps fld_old -> fld_new and the child's parent follows
    Ok(())
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

A `ModelSpec { defaults: &'static [(&'static str, Value)], optional: &'static [&'static str] }` table keyed by `DocKind`, transcribed from `docs/inventory/models.md` in the original's `init()` order (so key order matches). M1 fills the kinds it writes: Workspace, WorkspaceMeta, Project, Request, RequestMeta, RequestGroup, RequestGroupMeta, Environment, Response, CookieJar, Settings. Every other kind gets the empty spec, which prunes nothing — an unmodelled kind keeps whatever it was handed, which is the same property the `Doc` round trip already guarantees. `init::apply` merges defaults for absent keys and removes body keys outside `defaults ∪ optional`.

`FIX-006`: transcribe the 62 expectations from `packages/insomnia/src/common/__tests__/import-v5-parser.test.ts` into `tests/v5_parser.rs` as model-validation tests — one per expectation, asserting a default, an accepted and a rejected shape of a discriminated union, or a mock/MCP variant, through `Doc` round trips rather than a schema library.

- [ ] **Step 4: Run to verify passing** — `cargo test -p heric-model`.

- [ ] **Step 5: Commit** — `feat(model): add initModel defaults, pruning and the kind registry`

