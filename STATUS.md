# Heric — Status

Updated at the end of every work session and committed to `develop`.

Last update: 2026-11-10.

## Done

- Cargo workspace with 12 library crates and 2 binaries, all compiling with
  stub implementations and unit tests.
- Root files: `Cargo.toml` (workspace, edition 2024, MSRV 1.95, MIT),
  `rust-toolchain.toml`, `.gitignore`, `.gitattributes`, `LICENSE`, `NOTICE`,
  `licenses/APACHE-2.0-insomnia.txt`, `README.md` in three languages,
  `CONTRIBUTING.md` in three languages, `CHANGELOG.md`, `cliff.toml`,
  `deny.toml`, `rustfmt.toml`, `clippy.toml`, `PLAN.md`, `STATUS.md`.
- Lint policy: `unsafe_code = "forbid"`, `clippy::unwrap_used` and
  `clippy::expect_used` denied workspace-wide.
- Git history started: `main` holds the bootstrap commit and `develop`
  branches from it. Work happens on `feature/ci-bootstrap`.
- GitHub Actions written: `ci.yml` (lint, test matrix across the three
  platforms plus an Intel Mac cross-build, MSRV, docs, gitflow branch check),
  `release.yml`, `docs.yml`; dependabot, PR template, four issue templates and
  CODEOWNERS.
- mdBook sites for `en`, `pt-BR` and `es` with identical structure and
  `SUMMARY.md` shape, plus `docs/book/GLOSSARY.md`.
- Scripts: documentation structure parity check (bash and PowerShell) and the
  Gitflow branch pair check.
- ADRs 0001–0008: packaging, docs i18n, template engine, HTTP client,
  scripting sandbox, storage format, Git backend, code editor.
- Verified locally on Windows: `cargo fmt --all --check` clean,
  `cargo clippy --workspace --all-targets -- -D warnings` clean,
  `cargo test --workspace --locked` green (12 unit tests),
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` clean, all three mdBook
  sites build, and the documentation structure check passes.

## In progress

- Phase 0.3 inventory: 12 agents dispatched over the original repository.
  Reports land in `docs/inventory/`; consolidation into `FEATURES.md` comes
  next.

## Blocked

- **Network egress to github.com.** Connections to `github.com:443` time out
  from this machine, so `git push`, `gh` API calls and the `cargo deny`
  advisory database fetch all fail. `cargo-deny` cannot be validated locally
  until the advisory database is reachable or is vendored.
- **GitHub authentication.** Even once the network works, `gh auth status`
  reports an invalid keyring token for `jimmykedlyn`; a
  `gh auth refresh -h github.com` is required.
- **Branch protection, labels, milestones, topics, default branch.** All need
  the `gh` API. Until then the Gitflow rules are enforced by discipline, as
  documented in `CONTRIBUTING.md`.
- **Release rehearsal** (`release/0.0.1` → PR to `main` → tag `v0.0.1-alpha`
  → artifacts → back-merge) needs the remote and CI.

## Decisions taken

- Package and binary naming: crate `heric` (GUI binary `heric`), crate
  `heric-cli` (binary `heric-cli`).
- MSRV 1.95, the floor imposed by `eframe`/`egui` 0.37 and `egui_dock` 0.21.
- The workspace declares candidate dependencies pinned in
  `[workspace.dependencies]`; each crate depends only on what it uses.
- Clippy's `all` group runs at priority -1 so the deny-level lints win.
- `cargo-deny` scopes the `unmaintained` advisory to workspace crates,
  because the flagged transitive crates come from egui/accesskit upstream.
- Packaging tool: `docs/adr/0001-packaging.md`.
- Documentation i18n: `docs/adr/0002-docs-i18n.md`.

## Next

1. Restore network and `gh` auth, then push `main` and `develop` and open the
   `feature/ci-bootstrap` PR.
2. Apply repository settings through `gh`: topics, labels, milestones M1–M7,
   merge methods, delete-branch-on-merge and branch protection for `main` and
   `develop`.
3. Wait for CI green on the three platforms, then run the release rehearsal.
4. Consolidate the inventory into `docs/inventory/FEATURES.md` and open one
   issue per P0/P1 item.
5. Report Phase 0 results in Portuguese and wait for approval before Phase 1.
