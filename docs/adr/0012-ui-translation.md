# 0012 — UI translation from M1

- Status: accepted
- Date: 2026-11-10

## Context

The user documentation ships in English, Portuguese and Spanish from the first
release (ADR 0002). The application's own interface was going to stay English
until later. That split produces a documented product the UI does not match,
and retrofitting translation into an egui application after the fact means
touching every widget, because an immediate-mode UI builds its strings on
every frame.

## Options

1. **`fluent`** (`fluent` + `fluent-bundle` + `unic-langid`) — Fluent syntax
   with plural and gender selectors, catalogs compiled into the binary.
2. **`rust-i18n`** — YAML catalogs, `t!` macro, flat messages, actively
   released.
3. **English only until M6**, then a migration.

## Decision

Translate from M1, with three languages: English (source), Portuguese
(pt-BR) and Spanish.

Library: **`fluent`**, with `rust-i18n` as the fallback if its maintenance
lags. The deciding constraint is not syntax but message quality: plural and
gender selectors matter in pt-BR and Spanish, and flat key-value messages force
sentence fragments into the UI. Both compile catalogs into the binary and
neither generates translation code at runtime, which is the other constraint.

Ownership: **strings live in the crate that owns them**, as resource files
with English as the source of truth. `heric-template` owns its error messages,
`heric-storage` owns its own. The GUI's chrome is a catalog in `apps/gui`.

Errors need special handling because a `thiserror` enum's `Display` is fixed
English and cannot be translated at display time. Every error enum therefore
also exposes a stable message key and its arguments, through one trait in
`heric-model`:

```rust
pub trait Diagnostic {
    fn message_key(&self) -> MessageKey;
    fn message_args(&self) -> Vec<(String, MessageValue)>;
    fn hint_key(&self) -> Option<MessageKey> { None }
}
```

`Display` stays English for logs and for the CLI. A key missing from a catalog
fails a test in debug builds and falls back to English in release.

## Consequences

- Three languages are part of the M1 definition of done, not a polish item. A
  feature that adds a user-visible string adds it in three catalogs.
- The GUI never formats an error itself; it asks the error for a key and
  arguments. That keeps the translation layer out of the widget code.
- Plural handling works for pt-BR and Spanish plurals from the start, which
  `rust-i18n`'s flat messages would have made a later refactor.
- These resources are separate from the documentation books in `docs/book/`.
  The docs check (`scripts/check-docs-structure.sh`) governs the books; the
  UI catalogs are checked by a test that compares key sets across languages,
  which is stricter than comparing file names.
