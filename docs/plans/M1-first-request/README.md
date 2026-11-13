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

## How to run a task — copy and paste

Every task follows the same five steps. This is the whole loop, for whatever
task you are on; only the file name, the crate and the commit message change.

```bash
# Always start here. Every command runs from this worktree.
cd "D:/Projects/Heric/Heric/.claude/worktrees/storage-paths"

# 1. Read the task and the constraints that apply to all of them.
cat docs/plans/M1-first-request/00-constraints.md
cat docs/plans/M1-first-request/task-12-*.md

# 2. Write the tests from the task, then watch them fail.
cargo test -p heric-template

# 3. Implement to the task's `Produces:` line.

# 4. Watch them pass.
cargo test -p heric-template

# 5. The five gates, then the commit.
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git add -A && git commit -m "feat(template): layer environments and render in three passes"
```

Steps 2 and 4 use the same command on purpose: the first run must fail for the
right reason (a `todo!()`, not a typo), and the second must pass.

Two constraints bite in almost every task: `clippy::unwrap_used` and
`clippy::expect_used` are denied including tests, so test functions return
`Result<(), Box<dyn std::error::Error>>` (or the crate's error) and use `?`; and
`unsafe_code` is forbidden, so no escape hatch exists.

## Order

**Do them in numeric order: 1, 2, 3, … 22.** The numbering is the dependency
order, so following it never leaves a task waiting on something unbuilt.

Two tasks are exceptions in the sense that they depend on nothing and nothing
waits on them until Task 17: **15** (libcurl engine) and **16** (auth
providers). They can be done at any point before 17, or simply in their turn.

The only hard rule the numbering does not make obvious: **17 needs 1–8, 12, 13,
15 and 16 done**, and **22 comes last**. Everything else is a straight line.

**Task 15 has a gate before its code.** Step 1 is a dependency spike: add the
manifest lines with a body that only constructs `curl::easy::Easy::new()`, push
the branch, and confirm all four CI `test` jobs build. That push is a side
effect outside this worktree — ask before doing it.

## The tasks

`Done` means the commit exists and the crate's suite is green. The test command
is the one to run for steps 2 and 4; the commit message is the one step 5 wants.

| # | Task file to read | Test command | Commit message | Status |
|---|---|---|---|---|
| 1 | `task-01-platform-paths-and-data-directory-discovery.md` | `cargo test -p heric-storage -p heric-core paths platform` | `feat(storage): resolve Heric and Insomnia data directories` | Done — `6ef0234`, fix `40fb023` |
| 2 | `task-02-sqlite-document-store.md` | `cargo test -p heric-storage` | `feat(storage): add the SQLite document store` | Done — `fd1ff2b`, fixes `eda3040` |
| 3 | `task-03-descendant-traversal-and-the-change-broadcast.md` | `cargo test -p heric-storage` | `feat(storage): cascade deletes and broadcast document changes` | Done — `b5ddb15`, fix `69598fd` |
| 4 | `task-04-blob-store-and-datafile-protection.md` | `cargo test -p heric-storage` | `feat(storage): add the blob store and datafile guards` | Done — `acef2ab`, fix `006a003` |
| 5 | `task-05-model-defaults-initmodel-and-the-kind-registry.md` | `cargo test -p heric-model` | `feat(model): add initModel defaults, pruning and the kind registry` | Done — `d572cb8`, `82a981c`, `c5d318e`, fix `0756fce` |
| 6 | `task-06-workspace-project-and-environment-views.md` | `cargo test -p heric-model` | `feat(model): add workspace, project and environment views` | Done — `967df80` |
| 7 | `task-07-responses-snapshots-and-delete-history.md` | `cargo test -p heric-model` | `feat(model): add responses, snapshots and delete history helpers` | Done — `aedc9ac` |
| 8 | `task-08-the-settings-singleton.md` | `cargo test -p heric-model settings` | `feat(model): add the settings singleton and its defaults` | Done — `d191cde` |
| 9 | `task-09-liquid-lexer-parser-and-comment-stripping.md` | `cargo test -p heric-template` | `feat(template): add the Liquid lexer, parser and comment stripping` | Done — `30e909d` |
| 10 | `task-10-evaluation-truthiness-limits-and-structured-errors.md` | `cargo test -p heric-template` | `feat(template): evaluate Liquid with strict variables and structured errors` | Done — `d9a60c0`, fix `8c8cc4d` |
| 11 | `task-11-filters-the-tag-argument-tokenizer-and-the-built-in-tags.md` | `cargo test -p heric-template` | `feat(template): add filters, tag arguments and the built-in tags` | Done — `7a47d9e` |
| **12** | `task-12-layered-context-three-pass-render-and-confidentiality.md` | `cargo test -p heric-template` | `feat(template): layer environments and render in three passes` | **NEXT** |
| 13 | `task-13-request-level-render-and-error-paths.md` | `cargo test -p heric-template` | `feat(template): render a request's fields atomically` | — |
| 14 | `task-14-the-liquid-conformance-suite.md` | `cargo test -p heric-template --test liquid_compat` | `test(template): port the 99-case Liquid conformance suite` | — |
| 15 | `task-15-adr-0010-spike-and-the-libcurl-engine.md` | `cargo test -p heric-http` | `feat(http): execute requests through libcurl` | — |
| 16 | `task-16-the-auth-providers-m1-needs.md` | `cargo test -p heric-auth` | `feat(auth): add the basic, bearer and API-key providers` | — |
| 17 | `task-17-the-send-pipeline.md` | `cargo test -p heric-core` | `feat(core): run the M1 send pipeline end to end` | — |
| 18 | `task-18-a-working-heric-cli-run.md` | `cargo test -p heric-cli` | `feat(cli): send a request and print the response` | — |
| 19 | `task-19-the-request-editor-and-response-pane.md` | `cargo test -p heric` | `feat(gui): edit a request and show the response` | — |
| 20 | `task-20-macos-window-lifecycle-and-the-intel-runner.md` | `cargo test -p heric lifecycle` | `ci: run the full suite on an Intel macOS runner` | — |
| 21 | `task-21-the-end-to-end-smoke-harness.md` | `cargo test -p heric --test e2e` | `test(gui): add the M1 end-to-end smoke harness` | — |
| 22 | `task-22-documentation-status-and-the-parity-matrix.md` | `scripts/check-docs-structure.sh` | `docs: describe M1 in three languages and close its parity rows` | — |

To start Task 12 right now, paste this:

```bash
cd "D:/Projects/Heric/Heric/.claude/worktrees/storage-paths"
cat docs/plans/M1-first-request/00-constraints.md
cat docs/plans/M1-first-request/task-12-layered-context-three-pass-render-and-confidentiality.md
```

...write the tests, run `cargo test -p heric-template` until it fails for the
right reason, implement, then:

```bash
cd "D:/Projects/Heric/Heric/.claude/worktrees/storage-paths"
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
git add -A
git commit -m "feat(template): layer environments and render in three passes"
```

For any other task, change the two file names in the first block and the test
and commit command here; nothing else changes.

## The ledger

Decisions, rulings and per-task records live in
`.superpowers/sdd/M1-first-request/progress.md`. That path is git-ignored, so it
is per-worktree memory rather than history. Record a `Ruling: <what> — <why> —
<cost if wrong>` line whenever you deviate from the plan; an unrecorded
deviation is a decision made in secret.

The plan's global constraint is that every task lands on
`feature/<crate>-<task>` and merges by PR into `develop`, never straight onto
`main` or `develop`.
