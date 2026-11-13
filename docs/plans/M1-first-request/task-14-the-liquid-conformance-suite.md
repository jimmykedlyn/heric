### Task 14: The Liquid conformance suite

**Files:**
- Create: `crates/heric-template/tests/liquid_compat.rs`
- Modify: `crates/heric-template/src/*` only where a case fails.

**Interfaces:** none new. This task is `FIX-010`'s definition of done and the acceptance for `TPL-001`.

- [ ] **Step 1:** Transcribe all **99** cases from `packages/insomnia/src/templating/__tests__/liquid-compat.test.ts` (26 `describe` blocks) into `tests/liquid_compat.rs`, one `#[test]` per original case, each carrying a comment naming its `describe`/`it` pair. Keep the Nunjucks-breaking-change group and the security group as their own modules, because the row names them separately.
- [ ] **Step 2:** Run `cargo test -p heric-template --test liquid_compat`. Record every failure in the PR description before fixing anything.
- [ ] **Step 3:** Fix in `heric-template` until green. A case that cannot pass exactly is reported to the owner as a scope question before the branch finishes; an expectation is never weakened to make a test green.
- [ ] **Step 4:** Verify `cargo test -p heric-template` is green across all three test binaries.
- [ ] **Step 5: Commit** — `test(template): port the 99-case Liquid conformance suite`

