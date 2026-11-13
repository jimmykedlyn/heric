### Task 20: macOS window lifecycle and the Intel runner

**Files:**
- Create: `apps/gui/src/lifecycle.rs`
- Modify: `apps/gui/src/main.rs`, `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- Test: `#[cfg(test)]` in `lifecycle.rs`

**Interfaces:**
- Produces: `lifecycle::{CloseAction, ActivateAction, on_close(Platform) -> CloseAction, on_activate(Platform) -> ActivateAction}` with `CloseAction::{Hide, Exit}` and `ActivateAction::{Reveal, Ignore}`.

- [ ] **Step 1: Write the failing tests** — `a_close_on_macos_hides_the_window`, `a_close_elsewhere_exits`, `an_activate_on_macos_reveals_and_elsewhere_ignores`.
- [ ] **Step 2:** on macOS the app sends `ViewportCommand::CancelClose` then `Visible(false)`, and shows the viewport again when the window is activated (`PLT-015`). Elsewhere the close proceeds.
- [ ] **Step 3: CI.** Design §7 and §21 require the Intel Mac to run the **full** suite from M1. Replace the cross-build entry with a dedicated runner and remove `build_only` from both workflows:

```yaml
          - os: macos-15-intel
            target: x86_64-apple-darwin
```

Remove `if: ${{ !matrix.build_only }}` from the `cargo test` step, and make the same change in `release.yml` so the Intel archive is built on Intel hardware. If `macos-15-intel` is unavailable to this repository — it is a paid larger runner on some plans — stop and report the observed billing or availability message to the owner before substituting anything, because a silent fallback to a cross-build would reverse the owner's "dedicated Intel runner" decision.
- [ ] **Step 4: Run to verify passing** — `cargo test -p heric lifecycle`, and the workflow must be green on the PR.
- [ ] **Step 5: Commit** — `ci: run the full suite on an Intel macOS runner`

