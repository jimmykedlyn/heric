### Task 7: Responses, snapshots and delete history

**Files:**
- Create: `crates/heric-model/src/response.rs`, `src/request.rs`
- Modify: `crates/heric-model/src/lib.rs`, `crates/heric-model/Cargo.toml`, `Cargo.toml`
- Test: `#[cfg(test)]` in both

**Interfaces:**
- Produces: `response::{create(request: &Doc, execution: &Execution, body_path: &str, timeline_path: &str, now: i64) -> Doc, read_body(bytes: &[u8], compressed: bool) -> Result<Vec<u8>, ModelError>, prune(docs: Vec<Doc>, max_responses: usize) -> Vec<Doc>, latest(docs: &[Doc], environment: Option<&Id>) -> Option<&Doc>}`; `request::{path_parameters(url: &str, parameters: &[Value]) -> Result<(String, Vec<(String, String)>), ModelError>, merge_parameters(saved: &[Value], unsaved: &[Value]) -> Vec<Value>, snapshot(&Doc, kind: DocKind, now: i64) -> Doc}`; one new `ModelError` variant, `CorruptBody { detail: String }`, with its `Display` text.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_response_snapshots_the_request_version_and_paths() -> Result<(), ModelError> {
    let response = response::create(&request_doc()?, &Execution::ok(204), "b/res_1", "t/res_1", 10);
    assert_eq!(response.body.get("statusCode"), Some(&serde_json::json!(204)));
    assert_eq!(response.body.get("bodyPath"), Some(&serde_json::json!("b/res_1")));
    assert_eq!(response.body.get("requestVersionId"), Some(&serde_json::json!("rvr_1")));
    Ok(())
}

#[test]
fn an_empty_body_is_read_as_empty_not_as_corrupt() {
    assert_eq!(response::read_body(&[], false), Ok(Vec::new()));
}

#[test]
fn a_gzip_body_is_decompressed_and_a_plain_one_passes_through() -> Result<(), ModelError> {
    // flate2 GzEncoder over b"hello" -> read_body(.., true) == b"hello"
    assert_eq!(response::read_body(b"plain", false)?, b"plain");
    Ok(())
}

#[test]
fn pruning_keeps_the_newest_twenty() { /* 25 docs, assert 20 kept and the oldest five dropped */ }

#[test]
fn the_latest_response_is_filtered_by_environment_and_sorted_desc() { /* MOD-023 */ }

#[test]
fn a_saved_path_parameter_is_merged_over_the_url_template() -> Result<(), ModelError> {
    let (url, _) = request::path_parameters("https://x.test/users/:id", &[serde_json::json!({"name": "id", "value": "7"})])?;
    assert_eq!(url, "https://x.test/users/7");
    Ok(())
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** with `flate2 = "1.1.11"` in `[workspace.dependencies]` and the crate. `read_body` returns `Ok(Vec::new())` for empty input and gunzips when `compressed`. A decode failure returns a new variant added to `ModelError` in this task, `ModelError::CorruptBody { detail: String }`, because the existing three variants (`UnknownKind`, `WrongKind`, `MissingField`) all describe a document's shape and none describes bytes that would not decompress. `prune` sorts by `modified` descending and truncates at `max_responses`, which the caller supplies as 20 by default (`MOD-022`). `create` writes `statusCode`, `statusMessage`, `httpVersion`, `headers`, `bodyPath`, `timelinePath`, `requestVersionId`, `environmentId`, `bytesContent` and `elapsed`; the snapshot id `rvr_1` comes from `request::snapshot`, which copies the request and preserves `_id`, `created` and `parentId` (`MOD-026`). Deleting response history (`MOD-025`) is the store's cascade in Task 3 plus `BlobStore::remove` in Task 4; Task 17 carries the test that both the document and both blobs disappear.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(model): add responses, snapshots and delete history helpers`

