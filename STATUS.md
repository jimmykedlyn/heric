# Heric — Status

Updated at the end of every work session and committed to `develop`.

Last update: 2026-11-11.

## Done

- Phase 1, merged in PR #364 (`08c8932`, squash):
  - Brainstorming with the owner closed four decisions: secrets copied as
    ciphertext with pending re-entry, dedicated Intel and Silicon macOS
    runners, `curl` as the engine, and a Heric-owned data directory that
    never writes back into the Insomnia tree.
  - Architecture design approved (1,008 lines, in
    `docs/superpowers/specs/`), with ADRs 0001–0013. ADR 0010 (libcurl build
    and TLS) stays *proposed* until its CI spike runs.
  - Twelve library crates expose their public contracts with `todo!()` bodies
    naming the milestone. `heric-core` wires them and owns the concurrency
    model from ADR 0009.
  - `heric-core` re-exports every type its service signatures name, so a front
    end depends on one crate rather than twelve.
  - `apps/gui` owns the event bridge, drains it once per frame and renders a
    console; `apps/cli` exposes `version`, `migrate`, `import`, `export`,
    `run`, `test`, `lint` and `snippet`, each reporting its milestone.
  - ADR 0013 replaced the planned `egui_dock`: it pins `egui` 0.36 against a
    0.37 workspace and docking has no parity row. The GUI uses `egui`'s own
    panels; `egui_tiles` 0.18 is pinned as the approved dock for later.
  - Workspace dependencies reconciled with the ADRs: `curl` added,
    `reqwest`, `rustls-platform-verifier` and `egui_dock` removed.
  - All five gates pass locally and all eight CI checks pass on the four
    targets: 245 tests, clippy with warnings denied, `cargo deny`, rustdoc
    and the three-language docs build.

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
- Branch protection on `main` and `develop`: pull request required, the eight
  CI checks required with the head branch up to date, force-push and deletion
  blocked, rules enforced for administrators.
- Release cycle exercised end to end and the rehearsal cleaned up:
  - PR `#11` (`release/0.0.1`) merged into `main` as a merge commit `3806865`,
    carrying ten commits and the 0.0.1 version bump.
  - Tag `v0.0.1` pushed on that commit; the `Release` workflow passed
    `tag is on main`, built the four targets and published a GitHub Release
    with changelog notes.
  - The rehearsal GitHub Release and the `v0.0.1` tag were deleted afterwards,
    so the repository carries no release and no tag until the first real one.
  - Back-merge `main` into `develop` landed as PR `#361`.
  - Defects found by running it: the `github-pages` environment only allowed
    `develop` to deploy, so the `main` deploy failed and `main` was allowed as
    a deployment branch; and the build jobs published `dist/**` alongside the
    archives, so each target left four loose files in the release. Both fixed,
    the second by PR `#362`.

## In progress

- Nothing. Phase 1 is complete and merged into `develop` as PR #364
  (`08c8932`); M1 starts next.

## Blocked

- Nothing.

## Decisions taken

- Package and binary naming: crate `heric` (GUI binary `heric`), crate
  `heric-cli` (binary `heric-cli`).
- MSRV 1.95, the floor imposed by `eframe`/`egui` 0.37.
- The GUI uses `egui`'s own panels, not a docking crate (ADR 0013):
  `egui_dock` 0.21.1 pins `egui` 0.36 and would link a second copy of the
  toolkit. `egui_tiles` 0.18 is pinned as the approved dock if draggable splits
  ever become a requirement.
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

1. M1: SQLite store and blob store (`heric-storage`), the `curl` engine
   (`heric-http`), the M1 template tags and the first end-to-end GET/POST in
   the window. `heric-core` already wires all of it; the `todo!()` bodies are
   the work.
2. ADR 0010's spike: build and test the workspace with the selected libcurl
   feature set on all four targets, then move it from *proposed* to *accepted*
   or fall back as it describes.
3. The first real release is `v0.1.0` at the end of M1; until then the
   repository stays untagged.
