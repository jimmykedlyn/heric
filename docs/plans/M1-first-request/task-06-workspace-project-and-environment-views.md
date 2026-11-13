### Task 6: Workspace, project and environment views

**Files:**
- Create: `crates/heric-model/src/views.rs`, `src/workspace.rs`, `src/environment.rs`
- Modify: `crates/heric-model/src/lib.rs`
- Test: `#[cfg(test)]` in each

**Interfaces:**
- Produces: `Doc::as_group()`, `as_environment()`, `as_project()`, `as_response() -> Result<…, ModelError>`; `workspace::{Scope, CollectionScope, scope_of(&Doc) -> Result<Scope, ModelError>, Projects::sorted(&[Doc]) -> Vec<Doc>, storage_label(&Doc) -> &'static str, assert_parent_is_project(&Doc, &dyn Fn(&Id) -> Option<Doc>) -> Result<(), ModelError>}`; `environment::{kv_from_json, json_from_kv, base_for(parent: Id) -> Doc, BASE_NAME}`; `views::{midpoint(a: f64, b: f64) -> f64, copy_name(&str) -> String, inherited_headers(request: &Doc, ancestors: &[Doc]) -> Vec<Value>, inherited_authentication(request: &Doc, ancestors: &[Doc]) -> Option<Value>}`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_disabled_header_row_is_skipped_by_inheritance() -> Result<(), ModelError> {
    // folder headers: [{"name":"A","value":"1"},{"name":"B","value":"2","disabled":true}]
    // inherited_headers(request, &[folder]) keeps A and drops B
    Ok(())
}

#[test]
fn a_folder_auth_block_is_inherited_only_when_the_request_has_none() -> Result<(), ModelError> {
    // request authentication {} + folder {"type":"bearer"} -> inherited; request's own wins otherwise
    Ok(())
}

#[test]
fn a_workspace_parent_must_be_a_project() { /* a Request parent, a Project parent, an absent parent */ }

#[test]
fn projects_sort_by_organization_then_name() -> Result<(), ModelError> {
    // three projects: default org "zzz", another org "aaa", default org "bbb"
    Ok(())
}

#[test]
fn an_empty_url_produces_no_request() {
    // RequestEditor { url: String::new(), .. } -> to_patch() leaves `url` empty and
    // Send stays disabled; no PreparedRequest is built.
}

#[test]
fn the_scope_discriminator_reads_collection_design_and_mock_server() { /* MOD-005 */ }

#[test]
fn the_storage_label_is_human_readable() { /* Local, Remote, Git */ }

#[test]
fn the_middle_sort_key_lands_between_its_neighbours() {
    assert_eq!(views::midpoint(-10.0, 0.0), -5.0);
    assert_eq!(views::midpoint(0.0, 10.0), 5.0);
}

#[test]
fn a_copy_is_named_with_the_suffix_once() {
    assert_eq!(views::copy_name("Foo"), "Foo (Copy)");
    assert_eq!(views::copy_name("Foo (Copy)"), "Foo (Copy) (Copy)");
}

#[test]
fn the_base_environment_id_is_deterministic_for_a_parent() {
    assert_eq!(environment::base_for(Id::new("wrk_1")), environment::base_for(Id::new("wrk_1")));
    assert_ne!(environment::base_for(Id::new("wrk_1")), environment::base_for(Id::new("wrk_2")));
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** the views as reads over `Doc::body` that never mutate. `scope_of` distinguishes `collection`, `design` and `mock-server` by the workspace's `scope` field, defaulting to `collection` when absent. `assert_parent_is_project` returns `ModelError::WrongKind { expected: DocKind::Project, found }`, the variant the crate already has, and is what the document service calls on create, update and upsert (`MOD-007`). `base_for` derives the id from a stable hash of `<parent>` prefixed `env_`, so a base environment is created lazily once per parent and never duplicated (`MOD-037`); `BASE_NAME` is `"Base Environment"`.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(model): add workspace, project and environment views`

