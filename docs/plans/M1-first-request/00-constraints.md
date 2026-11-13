# M1 global constraints and review focus

Extracted from `docs/plans/M1-first-request.md`, lines above Task 1.
Everything here applies to every task and is not repeated in the task
files.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A user launches `heric`, creates a workspace with one request, presses Send, and sees the real response and libcurl timeline in the window, with every document persisted in Heric's own SQLite store.

**Architecture:** Phase 1 landed the contracts: twelve library crates expose their public types and every unimplemented body is a `todo!()` naming its milestone. M1 fills the M1 bodies only — the SQLite `Store`, the `BlobStore`, the Liquid renderer, the `curl`-backed `HttpEngine`, three auth providers, the send pipeline's non-script stages, and a GUI that edits one request and displays one response. `heric-core` keeps its Phase 1 shape; this milestone supplies the adapters it already expects.

**Tech Stack:** Rust 2024, `rusqlite` 0.41.1 (`bundled` SQLite), `curl` 0.4.51 (`curl-sys` 0.4.91, libcurl 8.22.0), `flate2` 1.1.11, `tempfile` 3.27.0 (tests), `serde_json`, `thiserror`, `tokio` 1.54, `eframe`/`egui` 0.37 with `egui_extras::TableBuilder`, `wiremock` 0.6.5.

**Spec:** `docs/superpowers/specs/2026-11-10-heric-architecture-design.md` (approved), read together with ADRs `0003-template-engine`, `0004-http-client`, `0006-storage-format`, `0009-gui-concurrency`, `0010-libcurl-build-and-tls`, `0011-secrets`, `0013-gui-layout`. The plan argues from those; executors read both.

## Global Constraints

- MSRV **1.95**, edition **2024** from `[workspace.package]`. An ADR records any new floor and the `msrv` CI job enforces it.
- `unsafe_code = "forbid"`. `clippy::unwrap_used` and `clippy::expect_used` are **denied workspace-wide, including `#[cfg(test)]`**. Test functions return `Result<(), Box<dyn std::error::Error>>` and use `?`, or `.ok_or("…")?` for an `Option`. Never add `#[allow(...)]`.
- Libraries use `thiserror` and return their crate's error type. `anyhow` only in `apps/gui` and `apps/cli`.
- No test touches the network. Every HTTP test is against `wiremock` or a loopback listener.
- `D:\Projects\Heric\Insomnia` is **read-only**. Fixtures are transcribed from it, never written into it, and Heric never writes into an Insomnia data directory.
- Licences: everything on `deny.toml`'s allow-list. New crates in this plan: `rusqlite` 0.41.1 (MIT), `flate2` 1.1.11 (MIT OR Apache-2.0), `tempfile` 3.27.0 (MIT OR Apache-2.0). `wiremock` 0.6.5 (MIT) is already in `[workspace.dependencies]` but unused, so it enters `Cargo.lock` for the first time in Task 15.
- A dependency reaches `[workspace.dependencies]` only when a task here names it, its version, and the ADR covering it.
- Conventional Commits in English with `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. Nothing is committed to `main` or `develop`; every task lands on `feature/<crate>-<task>` and merges by PR into `develop`.
- Code, comments, ADRs and plan documents in English. User docs ship `en`, `pt-BR` and `es` with identical heading shape, or `scripts/check-docs-structure.sh` fails.
- Gates before every task's final commit: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.

## Review Focus

The spec says what Heric must do; it does not enumerate every input that will reach it, and its silence is not permission to break. These five are the inputs most likely to hurt a user, most likely first. Each gets a test in the task named beside it.

1. **A document carrying a field Heric has never heard of.** The original's 44 models drift between releases, and a user's workspace holds keys no table models. Expected: read, store and write back unchanged, and never fail a load over an unknown key. → Task 2.
2. **A request whose header references an undefined variable.** Expected: the send refuses and names the variable and its field path; it never sends a header with an empty value in its place. → Task 13 renders it and leaves the document untouched, and Task 17 proves the engine was never called.
3. **A response body containing `{{` or `{%`.** The second render pass is gated on the *original input* having no tags, so an echoed body must never be re-interpreted. Expected: a literal round trip. → Task 12.
4. **A secret awaiting re-entry after migration.** Expected: the send stops with `StopReason::PendingCredential` and the UI offers the environment editor; it never emits a blank credential. → Task 16 returns the error instead of an empty value, and Task 17 turns it into the stop reason.
5. **A body-less response (204, 304, HEAD) and a body over 1 MB.** Expected: status and headers render with an empty body and no panic, and the large body streams from the blob store instead of being held twice. → Task 4 streams, Task 15 pins the empty 204 at the engine, and Task 19 renders both cases.

---

## Test support the tasks add

These helpers do not exist yet and each task that uses one creates it inside its own `#[cfg(test)]` module. They are listed once here so two tasks do not invent two spellings of the same thing.

- `ephemeral() -> Result<StoreHandle, StorageError>` — Task 2, a store over a `tempfile::TempDir` database. `tempfile = "3.27.0"` (MIT OR Apache-2.0) joins `[workspace.dependencies]` in Task 2, and each crate that needs it takes it under `[dev-dependencies]`.
- `request_doc() -> Doc` — Task 2, one `Request` with a parent, a `url`, a `method` and one `headers` row. Task 3 adds `workspace_doc(id)`, `folder_doc(id, parent)`, `request_doc_at(id, parent)`, `response_doc(id, parent)`. Task 7 reuses `request_doc()?` from its own module, because each crate's tests are separate.
- `ephemeral_blobs() -> Result<BlobStoreHandle, StorageError>` — Task 4, blobs over a `TempDir`.
- `NoHost` — Task 10, a `TagHost` whose `read_file`, `prompt` and `send` all return `TagError::PathNotAllowed` or `TagError::NotAvailableInThisPurpose`. It lives in a `pub(crate) mod testing` inside `heric-template` so every module's tests share it rather than five modules each define one.
- `context(&Scope) -> RenderContext<'_>` and `context_with(&[(&str, Value)]) -> RenderContext<'_>` — Task 10, built from the `Scope`/`Scope` shape the Phase 1 `RenderContext` already declares.

---

