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
- Quality gates verified locally on Windows and all passing:
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace --locked` (12 unit tests),
  `cargo deny check` (`advisories ok, bans ok, licenses ok, sources ok`) and
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`.
- The three mdBook sites build and the documentation structure check passes.
- Remote published at https://github.com/jimmykedlyn/Heric (public) with
  `main`, `develop` and `feature/ci-bootstrap` pushed.
- Labels created: `crate:*` for all 12 crates and both apps, `milestone:M1` to
  `milestone:M7`, `parity`, `docs`, `i18n` and `platform:windows|linux|macos`.
- Pull request https://github.com/jimmykedlyn/heric/pull/6 open from
  `feature/ci-bootstrap` to `develop`.

## In progress

- Phase 0.3 inventory: 12 agents dispatched over the original repository.
  Reports land in `docs/inventory/`; consolidation into `FEATURES.md` comes
  next.
- First CI run on PR #6, which must be green on Windows, Linux and macOS
  before the merge.

## Blocked

- **Branch protection** for `main` and `develop` is applied only after the
  first CI run publishes the exact job names to require.
- **Release rehearsal** (`release/0.0.1` → PR to `main` → tag `v0.0.1-alpha`
  → artifacts → back-merge) waits for PR #6 to merge.
- **`FEATURES.md` and the per-item issues** wait for the inventory reports.

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

1. Wait for CI green on the three platforms on PR #6, then squash merge into
   `develop`.
2. Apply branch protection to `main` and `develop` with the published check
   names, block force-push and deletion, and require pull requests.
3. Run the release rehearsal through `release/0.0.1`, then remove the test
   release and tag after confirmation.
4. Consolidate the inventory into `docs/inventory/FEATURES.md` and open one
   issue per P0/P1 item.
5. Report Phase 0 results in Portuguese and wait for approval before Phase 1.
