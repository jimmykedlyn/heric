## Summary

<!-- One or two sentences: what changes and why. -->

Closes #

## Checklist

- [ ] Tests cover the new behavior, and they failed before the change.
- [ ] Behavior matches the original client where parity applies; the source
      file and line are cited in the code or below.
- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
      `cargo test --workspace` and `cargo deny check` pass locally.
- [ ] CI is green on Windows, Linux and macOS.
- [ ] User-facing documentation was updated in `en`, and the `pt-BR` and `es`
      translations are in this PR.
- [ ] Target branch is correct: `feature/*` → `develop`, `release/*` or
      `hotfix/*` → `main`, `main` → `develop` only for back-merges.
- [ ] `STATUS.md` and the parity matrix were updated, when the change affects
      them.

## Notes for the reviewer

<!-- Edge cases, decisions, anything surprising. -->
