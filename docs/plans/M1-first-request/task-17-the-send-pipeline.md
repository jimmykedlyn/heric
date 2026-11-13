### Task 17: The send pipeline

**Files:**
- Modify: `crates/heric-core/src/service.rs`, `src/host.rs`, `src/runtime.rs`
- Test: `#[cfg(test)]` in `service.rs`, plus `crates/heric-core/tests/send.rs`

**Interfaces:**
- Produces: `RequestService::{prepare, send_and_wait}` implemented for the stages M1 runs; `SendingHost::read_file` consulting `is_datafile` before reading.

**Stages M1 runs:** `Inherit`, `Render`, `BuildBody`, `Authenticate`, `Execute`, `StoreResponse`, `RecordHistory`. `PreRequestScript`, `Plugins`, `AfterResponseScript` and `PatchResponse` arrive with M5, so `ExecutionPlan::stages` omits them — which is exactly what `ExecutionPlan::minimal` already encodes.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_prepared_request_carries_the_rendered_url_and_headers() -> Result<(), CoreError> { /* … */ }

#[test]
fn an_undefined_variable_stops_the_send_before_the_engine_runs() { /* fake engine records zero calls */ }

#[test]
fn a_pending_credential_stops_the_send_with_a_stop_reason() { /* StopReason::PendingCredential */ }

#[test]
fn a_response_is_stored_with_its_body_and_timeline_blobs() -> Result<(), CoreError> {
    // after the send: one Response doc, BlobRole::Body and BlobRole::Timeline both non-empty
    Ok(())
}

#[test]
fn a_finished_send_reports_send_finished_and_releases_the_flight() { /* is_sending() back to false */ }
#[test]
fn a_cancelled_send_reports_send_cancelled() { /* … */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** `prepare` as `Inherit` (through `views::inherited_headers` and `inherited_authentication` from Task 6) → `Render` (`render_request` with a `SendingHost` at `RenderPurpose::Send`) → `BuildBody` (`Body::Empty` for GET and HEAD, `Body::Bytes` for text, `Body::File` for a `{% file %}` body) → `Authenticate` (through the existing `apply_auth` helper). Implement `send_and_wait` to call `ports().engine.execute` inside `run_blocking`, then `response::create`, `blobs.put` for body and timeline, `store.insert` for the response document, and a `Patch` setting `RequestMeta.activeResponseId`. Every failure path still calls `end_flight`, which Phase 1 already guarantees.

In `host.rs`, `read_file` returns `TagError::PathNotAllowed` for a path where `is_datafile` is true, closing `STO-020`'s template and plugin route.

- [ ] **Step 4: Run to verify passing** — `cargo test -p heric-core`.

- [ ] **Step 5: Commit** — `feat(core): run the M1 send pipeline end to end`

