### Task 8: The settings singleton

**Files:**
- Create: `crates/heric-model/src/settings.rs`
- Modify: `crates/heric-model/src/lib.rs`
- Test: `#[cfg(test)]` in `settings.rs`

**Interfaces:**
- Produces: `settings::defaults() -> serde_json::Map<String, Value>`, `settings::singleton(now: i64) -> Doc`, `settings::repair(doc: &mut Doc) -> Vec<&'static str>` (names of the keys it had to fix), `settings::migrate_hotkeys(map: serde_json::Map<String, Value>) -> serde_json::Map<String, Value>`.

- [ ] **Step 1: Write the failing tests** — `every_default_key_is_present`, `a_missing_key_is_restored_and_reported`, `a_hotkey_bound_twice_is_reported_and_left_to_the_user` (repair reports, never silently rebinds — the original lets the user choose), `the_settings_id_is_the_singletons`. Transcribe the 60+ defaults from `docs/inventory/models.md` § Settings; do not invent a key. `MOD-054` is P2, so the accepted gap is: defaults and repair only, with hotkey *conflict* resolution deferred and named in the PR description.
- [ ] **Step 2–5:** verify failure, implement, verify `cargo test -p heric-model settings`, commit `feat(model): add the settings singleton and its defaults`.

