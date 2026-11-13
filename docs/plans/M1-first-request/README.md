# M1 — First Request: task index and execution order

This directory holds `docs/plans/M1-first-request.md` split into one file per
task, so an executor reads only the task it is working on instead of the whole
1090-line plan. The plan is still the single source of truth: these files are
generated from it and carry its text verbatim, minus the separators.

- `00-constraints.md` — the plan's global constraints, review focus and shared
  test-support helpers. Applies to **every** task; read it before the first one
  and again before any commit.
- `task-NN-<slug>.md` — one task, unchanged.
- `23-row-traceability.md` — the parity-row map for the whole milestone (which
  `FEATURES.md` row each task closes). Not a task; Task 22 uses it.

## How to run a task

Each task has five steps in a fixed shape:

1. **Write the failing tests** — the code blocks in the task are the tests,
   transcribed from the spec. Run them and watch them fail for the right reason
   (a `todo!()`, not a typo).
2. **Run to verify failure.**
3. **Implement** to the interfaces listed in the task's `Produces:` line.
4. **Run to verify passing.**
5. **Commit** with the conventional message the task names.

The verification command per task is in the table below. Tasks that do not name
one are verified with their crate's suite. Before any commit, all five gates in
`00-constraints.md` must pass:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
scripts/check-docs-structure.sh   # only when docs changed
```

Two constraints bite in almost every task: `clippy::unwrap_used` and
`clippy::expect_used` are denied including tests, so test functions return
`Result<(), Box<dyn std::error::Error>>` (or the crate's error) and use `?`; and
`unsafe_code` is forbidden, so no escape hatch exists.

## Order and dependencies

The tasks are numbered in dependency order, but three of them can run in
parallel with the template track.

```text
storage track   1 → 2 → 3
                2 → 4

model track     2 → 5 → 6
                    5 → 7
                    5 → 8            (8 has no consumer in M1)

template track  9 → 10 → 11
                 10 → 12 → 13 → 14

http track      15                 (independent; its Step 1 is a CI spike)
auth track      16                 (independent)

integration     17 ← 1-8, 12, 13, 15, 16
                17 → 18
                17 → 19 → 20
                17, 19 → 21

close-out       22 ← everything
```

Why those edges, in one line each:

| Edge | Reason |
|---|---|
| 1 → 2 | Task 2's store takes `DataPaths` and `StorageError` from Task 1. |
| 2 → 3 | Task 3 implements `remove`'s descendant walk and `changes` on Task 2's store. |
| 2 → 4 | Both touch `heric-storage/Cargo.toml` and `lib.rs`; 4 adds the `insomnia.*.db` guard that 17 consumes. |
| 2 → 5 | The kind registry and `initModel` operate on the `Doc` Task 2 persists. |
| 5 → 6, 7, 8 | All three build on `kind`, `init` and `Doc`. |
| 9 → 10 | Evaluator consumes the AST Task 9 produces. |
| 10 → 11, 12 | Filters/tags and the context merge both run inside the evaluator. |
| 12 → 13 | Request-level render is the layered context applied per field. |
| 13 → 14 | The conformance suite exercises the whole engine. |
| 12, 13, 15, 16 → 17 | The send pipeline is render + engine + auth + store, in that order. |
| 17 → 18, 19 | CLI and GUI both drive the pipeline. |
| 19 → 20, 21 | Lifecycle and the e2e harness both need the GUI event loop. |

**Task 15 has a gate before its code.** Step 1 is a dependency spike: add the
manifest lines with a body that only constructs `curl::easy::Easy::new()`, push
the branch, and confirm all four CI `test` jobs build. That push is a side
effect outside this worktree — ask before doing it.

## The tasks

`Done` means the commit named in the ledger exists and the crate's suite is
green. `Next` is the task the numbering expects next.

| # | Task | Crate | Verification | Status |
|---|---|---|---|---|
| 1 | Platform paths and data-directory discovery | `heric-storage`, `heric-core` | `cargo test -p heric-storage -p heric-core paths platform` | Done — `6ef0234`, fix `40fb023` |
| 2 | SQLite document store | `heric-storage` | `cargo test -p heric-storage` | Done — `fd1ff2b`, fixes `eda3040` |
| 3 | Descendant traversal and the change broadcast | `heric-storage` | `cargo test -p heric-storage` | Done — `b5ddb15`, fix `69598fd` |
| 4 | Blob store and datafile protection | `heric-storage` | `cargo test -p heric-storage` | Done — `acef2ab`, fix `006a003` |
| 5 | Model defaults, `initModel` and the kind registry | `heric-model` | `cargo test -p heric-model` | Done — `d572cb8`, `82a981c`, `c5d318e`, fix `0756fce` |
| 6 | Workspace, project and environment views | `heric-model` | `cargo test -p heric-model` | Done — `967df80` |
| 7 | Responses, snapshots and delete history | `heric-model` | `cargo test -p heric-model` | Done — `aedc9ac` |
| 8 | The settings singleton | `heric-model` | `cargo test -p heric-model settings` | Done — `d191cde` |
| 9 | Liquid lexer, parser and comment stripping | `heric-template` | `cargo test -p heric-template` | Done — `30e909d` |
| 10 | Evaluation, truthiness, limits and structured errors | `heric-template` | `cargo test -p heric-template` | Done — `d9a60c0`, fix `8c8cc4d` |
| 11 | Filters, the tag argument tokenizer and the built-in tags | `heric-template` | `cargo test -p heric-template` | **Done, uncommitted** |
| 12 | Layered context, three-pass render and confidentiality | `heric-template` | `cargo test -p heric-template` | Next |
| 13 | Request-level render and error paths | `heric-template` | `cargo test -p heric-template` | — |
| 14 | The Liquid conformance suite | `heric-template` | `cargo test -p heric-template --test liquid_compat` | — |
| 15 | ADR 0010 spike and the libcurl engine | `heric-http` | `cargo test -p heric-http` | — |
| 16 | The auth providers M1 needs | `heric-auth` | `cargo test -p heric-auth` | — |
| 17 | The send pipeline | `heric-core` | `cargo test -p heric-core` | — |
| 18 | A working `heric-cli run` | `apps/cli` | `cargo test -p heric-cli` | — |
| 19 | The request editor and response pane | `apps/gui` | `cargo test -p heric` | — |
| 20 | macOS window lifecycle and the Intel runner | `apps/gui` | `cargo test -p heric lifecycle` | — |
| 21 | The end-to-end smoke harness | `apps/gui` | `cargo test -p heric --test e2e` | — |
| 22 | Documentation, status and the parity matrix | docs | `scripts/check-docs-structure.sh` | — |

## The ledger

Decisions, rulings and per-task records live in
`.superpowers/sdd/M1-first-request/progress.md`. That path is git-ignored, so it
is per-worktree memory rather than history. Record a `Ruling: <what> — <why> —
<cost if wrong>` line whenever you deviate from the plan; an unrecorded
deviation is a decision made in secret.

The plan's global constraint is that every task lands on
`feature/<crate>-<task>` and merges by PR into `develop`, never straight onto
`main` or `develop`.
