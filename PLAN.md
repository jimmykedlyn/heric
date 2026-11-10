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
| 0.1 GitHub repository and settings | in progress | Remote public at `jimmykedlyn/Heric`; labels, templates and Dependabot configured. Branch protection still needs approval. |
| 0.2 CI workflows | done | Eight jobs green on Windows, Linux, both macOS targets, MSRV, lint, docs and gitflow, on `develop` and on every PR since #6. |
| 0.2 Release cycle rehearsal (`release/0.0.1`) | blocked | Branch and PR ready; merging into `main` and tagging need approval. |
| 0.3 Inventory (12 subagents A-L) | done | Twelve reports in `docs/inventory/`, 4,033 lines, 304 citations verified. Merged in PR #8. |
| 0.3 Parity matrix and per-item issues | done | `docs/inventory/FEATURES.md`: 422 rows (P0 183, P1 162, P2 77) across 11 areas, with platforms, status and issue number per row. Matrix merged in PR #12; all 345 P0/P1 rows have an issue (#13 to #359). |

## Phase 1 — architecture and contracts

Not started. Begins with brainstorming in Portuguese, then ADRs and public
traits/type signatures with `todo!()` stubs. Ends with `feature/architecture`
merged into `develop` and a green CI on the three platforms.

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
