### Task 21: The end-to-end smoke harness

**Files:**
- Create: `apps/gui/tests/e2e.rs`
- Test: `apps/gui/tests/e2e.rs`

**Interfaces:**
- Produces: `E2e { data_dir: TempDir, core: Core, handle: CoreHandle }` with `E2e::start()`, `create_workspace(&self, name) -> Result<Id, CoreError>`, `create_request(&self, workspace, method, url) -> Result<Id, CoreError>`, `send(&self, id) -> Result<SendOutcome, CoreError>`. `Core` is held, not the handle alone: dropping `Core` stops the runtime, so a harness that kept only the handle would hang on the first await.

- [ ] **Step 1: Write the failing test** — `a_request_created_stored_and_sent_round_trips`: start a core on a temporary data directory, create a workspace and a request, send it to a loopback listener, assert a `Response` document exists with a body blob, then reopen the store from the same directory and assert both survive.
- [ ] **Step 2–5:** implement. A fake engine is injected through `CoreBuilder::with_http_engine` for any suite that must not touch the network. `STR-002`'s critical and migration suites arrive with M2; this task delivers the smoke suite and the harness they will use. Verify `cargo test -p heric --test e2e`; commit `test(gui): add the M1 end-to-end smoke harness`.

