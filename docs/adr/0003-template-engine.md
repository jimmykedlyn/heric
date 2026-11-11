# 0003 — Template engine

- Status: accepted
- Date: 2026-11-10

## Context

Insomnia renders request fields with **LiquidJS 10.27**, configured to be
compatible with the Nunjucks syntax its older templates used, and extended
with custom tags (`response`, `prompt`, `uuid`, `timestamp`, `base64`,
`hash`, `faker`, `file`, `jsonpath`, `cookie`, `os`, ...). An earlier version
of this ADR said "Nunjucks" throughout, following the naming used in the
original codebase. That naming is misleading: the semantics that matter are
Liquid's.

The differences are not cosmetic:

- Filters are Liquid's (`| default:`, `| json`, `| size`, ...), not Jinja's.
- `strictVariables` is on by default and raises on undefined variables.
- Truthiness is JavaScript's, not Ruby's or Python's.
- The render context has a dual shape: variables live both at the root and
  under a `_` object, and tags read one or the other.
- Tag arguments are tokenised Liquid-style, with quoted strings and keyword
  arguments (`{% response 'body', 'req_a', 'b64::JC5pZA==::46b', 'never', 60 %}`).

`docs/inventory/templating.md` records the engine configuration in full, and
`docs/inventory/FEATURES.md` rows `TPL-001` and `FIX-010` make the 99-case
conformance suite a P0/M1 deliverable.

No Rust crate implements this dialect. `minijinja` and `upon` implement Jinja2,
which is a different language with different filters, different escaping rules
and different block syntax; translating templates at import time would mean
rewriting user templates in a format whose semantics differ in the cases that
matter.

## Options

1. **Rewrite the engine in `heric-template`**, with Liquid semantics, covering
   exactly the syntax, filters and tags the original supports.
2. **Embed a JS engine and run the original LiquidJS bundle.** Guarantees
   identical semantics, but the bundle is a large Imba/TypeScript artefact
   whose plugin surface is not designed for embedding, and every render would
   cross a JS boundary.
3. **Translate templates at import time to a Jinja-compatible dialect.** Every
   template becomes a lossy transformation at the moment the user's data is
   migrated, which conflicts with the project's anti-goal about silent data
   loss.

## Decision

Option 1: a Rust reimplementation with **Liquid semantics**.

The scope is not guessed. It is bounded by the conformance suite the original
already maintains (`templating/__tests__/liquid-compat.test.ts`, 99 cases) plus
the render test tables in `common/__tests__/` and
`common/templating/__tests__/`. Those become the golden fixtures in
`crates/heric-template/tests/`, and a case that fails is a bug in our engine,
not a reinterpretation. The parser design is left to the builder (recursive
descent or a parser-combinator crate); only the public API is fixed.

The feature name "Nunjucks" stays in the inventory so the two remain greppable
together, and this ADR is the place that records what it actually means.

## Consequences

- We own a parser and an evaluator, and we must keep them aligned with the
  conformance table. That is the largest single crate in M1.
- Errors are richer than the original's in one direction only: undefined
  variable versus evaluation error stays a distinction the UI branches on, and
  the message carries template path, line and column.
- IO-bearing tags (`file`, `prompt`, `response`, `cookie`, `os`) do not
  reach the store or the network directly. They go through `TagHost`, which
  `heric-core` implements, so the renderer is testable without a database and
  the CLI can render with a different host.
- `{% prompt %}` cannot block outside the send purpose (`TPL-017`); the host
  returns `NotAvailableInThisPurpose` instead of waiting.
