### Task 3: Descendant traversal and the change broadcast

**Files:**
- Modify: `crates/heric-storage/src/sqlite.rs`
- Test: `#[cfg(test)]` in `sqlite.rs`

**Interfaces:**
- Produces: `Store::{insert, upsert, update, remove}` each emit one `Change` on a `broadcast::Sender<Change>`; `Store::changes()` returns a fresh receiver (already declared).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn removing_a_workspace_returns_every_descendant_id() -> Result<(), StorageError> {
    let store = ephemeral()?;
    store.insert(&workspace_doc("wrk_1"))?;
    store.insert(&folder_doc("fld_1", "wrk_1"))?;
    store.insert(&request_doc_at("req_1", "fld_1"))?;
    store.insert(&response_doc("res_1", "req_1"))?;
    let mut removed = store.remove(&Id::new("wrk_1"))?;
    removed.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(
        removed.iter().map(Id::as_str).collect::<Vec<_>>(),
        ["fld_1", "req_1", "res_1", "wrk_1"]
    );
    assert!(store.get(&Id::new("req_1"))?.is_none());
    Ok(())
}

#[test]
fn a_removal_emits_one_change_listing_every_id() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let mut rx = store.changes();
    store.insert(&workspace_doc("wrk_1"))?;
    let _ = rx.try_recv()?;
    let removed = store.remove(&Id::new("wrk_1"))?;
    let Change::Removed(ids) = rx.try_recv()? else { return Err("expected Removed".into()) };
    assert_eq!(ids.len(), removed.len());
    Ok(())
}

#[test]
fn a_lagging_subscriber_never_blocks_a_writer() -> Result<(), StorageError> {
    let store = ephemeral()?;
    let _rx = store.changes();          // never drained
    for index in 0..2000 {
        store.insert(&workspace_doc(&format!("wrk_{index}")))?;
    }
    Ok(())
}
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement**

Descendant collection walks `SELECT id FROM documents WHERE parent = ?` breadth-first inside the delete transaction, then deletes by id, so `remove` returns the full set in one pass. The sender has capacity `1024` and `send` discards its result (`let _ = self.sender.send(change);`), because a lagging GUI must drop events rather than stall a writer.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(storage): cascade deletes and broadcast document changes`

