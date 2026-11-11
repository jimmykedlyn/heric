# Heric — Project Plan

This file is the durable plan for the project and is kept up to date in
`develop`. It survives session interruptions; read it together with
`STATUS.md` at the start of every session.

## Mission

Rewrite the Insomnia API client in Rust with an egui/eframe UI, reaching
feature parity and keeping compatibility with users' existing data.

- Source of truth for behavior: the Insomnia repository (read only).
- Target platforms: Windows x86_64, Linux x86_64, macOS Apple Silicon and
  Intel.
- License: MIT, with Apache-2.0 portions derived from Insomnia.
- Branch model: Gitflow. `develop` is the default branch; everything goes
  through pull requests.

## Phase 0 — repository, CI/CD and inventory

| Step | Status | Notes |
|---|---|---|
| 0.1 Workspace skeleton and root files | done | Local scaffold, validated by the quality gates. Merged in PR #6. |
| 0.1 GitHub repository and settings | done | Remote public at `jimmykedlyn/Heric`; labels, templates and Dependabot configured. Branch protection on `main` and `develop`: pull request required, the eight CI checks required and branches must be up to date, force-push and deletion blocked, rules enforced for admins. |
| 0.2 CI workflows | done | Eight jobs green on Windows, Linux, both macOS targets, MSRV, lint, docs and gitflow, on `develop` and on every PR since #6. |
| 0.2 Release cycle rehearsal (`release/0.0.1`) | done | PR #11 merged into `main` as a merge commit, tag `v0.0.1` pushed, `Release` workflow green on all four targets, GitHub Release published. The rehearsal tag and release were deleted afterwards, as the plan requires. Defect found and fixed by PR #362: the build jobs uploaded `dist/**` as well as the archives, so the release carried four loose files per target. |
| 0.3 Inventory (12 subagents A-L) | done | Twelve reports in `docs/inventory/`, 4,033 lines, 304 citations verified. Merged in PR #8. |
| 0.3 Parity matrix and per-item issues | done | `docs/inventory/FEATURES.md`: 422 rows (P0 183, P1 162, P2 77) across 11 areas, with platforms, status and issue number per row. Matrix merged in PR #12; all 345 P0/P1 rows have an issue (#13 to #359). |

## Phase 1 — architecture and contracts

Complete. Merged into `develop` by PR #364 (`08c8932`).

| Step | Status | Notes |
|---|---|---|
| Brainstorming with the owner, in Portuguese | done | Four decisions locked: secrets copied as ciphertext with pending re-entry, dedicated Intel and Silicon macOS runners, `curl` as the HTTP engine, and a Heric-owned data directory that never writes back into the Insomnia tree. |
| Design document | done | `docs/superpowers/specs/2026-11-10-heric-architecture-design.md`, 999 lines, approved by the owner. |
| ADRs | done | 0001–0013 in `docs/adr/`. 0010 (libcurl build and TLS) stays *proposed* until its CI spike runs. |
| Public contracts per crate | done | Twelve library crates expose traits, types and signatures with `todo!()` bodies where the implementation belongs to a later milestone. `heric-core` wires them and owns the GUI concurrency model. |
| GUI concurrency model | done | ADR 0009: one UI thread, one tokio runtime, one unbounded event channel, repaint requested after the event is enqueued. |
| Binaries wired to the core | done | The GUI owns the event bridge and renders the console; the CLI exposes `version`, `migrate`, `import`, `export`, `run`, `test`, `lint` and `snippet`, each reporting its milestone until the port exists. |
| Quality gates | done | All five pass locally and all eight CI checks pass on the four targets: 245 tests, `clippy -D warnings`, `cargo deny`, rustdoc and the three-language docs build. |
| Merge | done | PR #364, squash-merged into `develop` as `08c8932`; the branch was deleted. |

Phase 1 is complete. M1 begins on `develop`.

Initial workspace proposal (adjust after the inventory):

```
crates/
  heric-model/       domain types, IDs, validation (no IO)
  heric-storage/     new persistence + Insomnia NeDB reader/migrator
  heric-template/    Nunjucks-compatible engine and template tags (rewritten)
  heric-http/        execution, timeline, cookies, TLS, proxy
  heric-auth/        every authentication scheme
  heric-protocols/   gRPC (tonic + prost-reflect), WebSocket, SSE, GraphQL
  heric-script/      JS sandbox + insomnia.* API
  heric-import/      importers (Insomnia v4/v5, Postman, HAR, cURL, OpenAPI, WSDL)
  heric-codegen/     snippet generation (httpsnippet replacement)
  heric-git/         Git sync
  heric-openapi/     spec parsing and lint
  heric-core/        application services and events
apps/
  gui/               eframe/egui, binary `heric`
  cli/               clap, binary `heric-cli`
```

## Milestones

| Milestone | Scope | Release |
|---|---|---|
| M1 | model, storage, basic template, HTTP, minimal GUI; real GET/POST end to end | v0.1.0 |
| M2 | read a real Insomnia installation, import/export with tested round trip | v0.2.0 |
| M3 | all auth schemes, cookies, certificates, proxy, environments, full template tags | v0.3.0 |
| M4 | GraphQL, gRPC, WebSocket, SSE | v0.4.0 |
| M5 | scripting and importers (Postman, HAR, cURL, OpenAPI) | v0.5.0 |
| M6 | codegen, Git sync, Design mode, full CLI | v0.6.0 |
| M7 | UI polish, performance, icons, installers, complete docs in three languages | v1.0.0 |

## Process per milestone

1. `superpowers:writing-plans` produces `docs/plans/MN-<name>.md` and it is
   merged into `develop` by PR.
2. `superpowers:subagent-driven-development`: one fresh subagent per task,
   independent tasks in parallel, each in its own worktree and
   `feature/<crate>-<task>` branch.
3. An independent reviewer subagent checks each task against the original
   behavior and the acceptance criteria.
4. Documentation changes ship with `pt-BR` and `es` translations in the same
   branch.
5. `superpowers:finishing-a-development-branch`: PR to `develop`, wait for
   CI, squash merge, clean the worktree, update `STATUS.md` and the parity
   matrix.
6. Milestone release: `release/X.Y.0`, version bump, `git-cliff` changelog,
   PR to `main`, merge commit, tag, release workflow, back-merge to `develop`.

## Out of scope

Cloud sync and Kong account login. Interfaces stay as stubs; see
`docs/out-of-scope.md`.
