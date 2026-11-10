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
- GitHub Actions: `ci.yml` (lint, test matrix across the three platforms plus
  an Intel Mac cross-build, MSRV, docs, gitflow branch check), `release.yml`,
  `docs.yml`; dependabot, PR template, four issue templates and CODEOWNERS.
- mdBook sites for `en`, `pt-BR` and `es` with identical structure and
  `SUMMARY.md` shape, plus `docs/book/GLOSSARY.md`.
- Scripts: documentation structure parity check (bash and PowerShell) and the
  Gitflow branch pair check, which now also accepts `dependabot/*` branches
  targeting `develop`.
- ADRs 0001–0008: packaging, docs i18n, template engine, HTTP client,
  scripting sandbox, storage format, Git backend, code editor.
- Quality gates verified locally on Windows and all passing:
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
  warnings`, `cargo test --workspace --locked` (12 unit tests),
  `cargo deny check` (`advisories ok, bans ok, licenses ok, sources ok`) and
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`.
- The three mdBook sites build and the documentation structure check passes.
- CI green on all eight jobs (Windows, Linux, both macOS targets, MSRV, lint,
  docs, gitflow) on `develop` at `0f1a665` and on every merge since.
- Remote published at https://github.com/jimmykedlyn/Heric (public). Merged by
  PR: `#6` bootstrap, `#7` gitflow gate fix, `#8` inventory, `#9` status
  rewrite, `#12` parity matrix, `#93` docs site root links, and the five
  Dependabot actions bumps (`checkout` 4 to 7, `upload-artifact` 4 to 7,
  `configure-pages` 5 to 6, `upload-pages-artifact` 3 to 5, `deploy-pages`
  4 to 5).
- GitHub Pages site deployed from `develop` under `/next`; the three language
  books serve 200 and the root page's fallback links now carry the base prefix.
- Labels created: `crate:*` for all 12 crates and both apps, `parity`, `docs`,
  `i18n` and `platform:windows|linux|macos`; milestones M1 to M7.
- Phase 0.3 inventory: twelve reports in `docs/inventory/`, 4,033 lines, with
  304 code citations all verified to resolve to an existing file and an
  in-range line in the read-only Insomnia tree.
- `docs/inventory/FEATURES.md`, the parity matrix: 422 rows across 11 areas,
  P0 183, P1 162, P2 77, each with owning crate, milestone, platform scope,
  status and issue number. Twelve priority conflicts between reports are
  recorded under Cross-cutting risks with the resolution taken.
- All 345 P0/P1 rows have a GitHub issue, numbered `#13` to `#359`, labelled
  `parity` plus `crate:<name>` and assigned to the milestone of the row.

## In progress

- Nothing. Phase 0 is complete apart from the two approval-gated items below.

## Blocked

- **Branch protection** for `main` and `develop` needs approval before the
  rules are written.
- **Release rehearsal**: `release/0.0.1` → PR `#11` into `main` is open, green
  on the eight checks and mergeable. Merging it and pushing the tag needs
  approval.
- **Deleting the rehearsal release and tag** needs approval after the run.

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
- Dependabot branches are valid under the Gitflow gate as long as they target
  `develop`.
- The parity matrix carries `Platforms`, `Status` and `Issue` columns so it is
  the delivery checklist, not only an inventory; P2 rows stay issue-free until
  a milestone schedules them.

## Next

1. Apply branch protection to `main` and `develop` with the eight published
   check names, block force-push and deletion, require pull requests (after
   approval).
2. Merge PR `#11`, tag the rehearsal release, check the artifacts, then remove
   the test release and tag after confirmation.
3. Report Phase 0 results in Portuguese and wait for approval before Phase 1.
