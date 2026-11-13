### Task 19: The request editor and response pane

**Files:**
- Modify: `apps/gui/src/main.rs`, `src/events.rs`
- Create: `apps/gui/src/editor.rs`, `src/response.rs`, `src/tree.rs`
- Test: `#[cfg(test)]` in `editor.rs` and `response.rs`

**Interfaces:**
- Produces: `editor::RequestEditor { method: String, url: String, headers: Vec<Row>, body: String }` with `to_patch(&self) -> Patch`; `response::ResponseView { status: u16, status_message: String, elapsed: Duration, size: u64, tab: ResponseTab, body: String }` with `ResponseView::from_execution(&Execution) -> Self`; `tree::collections(&[Doc]) -> Vec<TreeNode>`.

**Layout (ADR 0013, design §16.2):** `Panel::top` toolbar, `Panel::left` collection tree, `Panel::right` response, `CentralPanel` request editor, per-region tab strips drawn with `ui.selectable_label`. No docking crate.

- [ ] **Step 1: Write the failing tests**, all on pure functions so they run headless: `a_disabled_header_row_is_dropped_from_the_patch`, `an_empty_url_produces_no_request`, `a_tree_nests_a_request_under_its_folder`, `a_2xx_response_selects_the_body_tab`, `a_204_shows_an_empty_body_without_panicking` (Review Focus 5), `a_megabyte_body_is_not_pretty_printed` (the 1 MiB guard).
- [ ] **Step 2–5:** replace the Phase 1 placeholder frames, wire Send to `CoreHandle::spawn(requests.send_and_wait(…))`, absorb `SendStarted`, `Timeline`, `SendFinished` and `SendCancelled` into the response view and console, and keep `describe` exhaustive so a new variant is a compile error. Verify `cargo test -p heric` and run the binary against a loopback server; commit `feat(gui): edit a request and show the response`.

