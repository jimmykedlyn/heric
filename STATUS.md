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
  `deny.toml`, `rustfmt.toml`, `clippy.toml`.
- Lint policy: `unsafe_code = "forbid"`, `clippy::unwrap_used` and
  `clippy::expect_used` denied workspace-wide.
- GitHub Actions: `ci.yml` (lint, test matrix, MSRV, docs, gitflow check),
  `release.yml`, `docs.yml`, dependabot, PR and issue templates, CODEOWNERS.
- mdBook sites for `en`, `pt-BR` and `es` with identical structure, plus
  `docs/book/GLOSSARY.md`.
- Scripts: docs structure parity check and gitflow branch check.

## In progress

- Quality gates on the local Windows machine: formatting, clippy, tests and
  `cargo deny`.
- Phase 0.3 inventory has not started yet.

## Blocked

- **GitHub access.** `gh auth status` reports an invalid token in the
  keyring for account `jimmykedlyn`. Push, branch protection, labels,
  milestones, branch default and repository topics all need a
  `gh auth refresh -h github.com`.
- **Branch protection rules** cannot be applied until the token works.
  Until then, protect by discipline as described in `CONTRIBUTING.md`.
- **Release rehearsal** (`release/0.0.1` → tag `v0.0.1-alpha`) needs the
  remote configured and CI running.

## Decisions taken

- Package and binary naming: crate `heric` (GUI binary `heric`), crate
  `heric-cli` (binary `heric-cli`).
- MSRV 1.95, the floor imposed by `eframe`/`egui` 0.37 and `egui_dock` 0.21.
- The workspace declares candidate dependencies pinned in
  `[workspace.dependencies]`; each crate only depends on what it uses.
- Packaging tool: see `docs/adr/0001-packaging.md`.
- Documentation i18n: see `docs/adr/0002-docs-i18n.md`.

## Next

1. Run the full quality gate suite and fix any failures.
2. `git init`, initial commit on `main`, create `develop`.
3. Refresh `gh` auth, push `main` and `develop`, set `develop` as default,
   then apply topics, labels, milestones and branch protection.
4. Phase 0.3 inventory with parallel Explore subagents.
