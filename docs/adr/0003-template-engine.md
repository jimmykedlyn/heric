# 0003 — Template engine

- Status: proposed
- Date: 2026-11-10

## Context

Insomnia renders request fields with Nunjucks, extended by custom template
tags (`response`, `prompt`, `uuid`, `timestamp`, `base64`, `hash`, `faker`,
`file`, ...). No Rust crate implements Nunjucks or its tag syntax.

## Options

1. Rewrite a Nunjucks-compatible subset in `heric-template` with a parser and
   evaluator, covering the syntax Insomnia actually uses.
2. Embed a JS engine and run the original Nunjucks bundle.
3. Use a different Rust template crate (minijinja, upon) and translate
   templates at import time.

## Decision

Proposed: option 1, scoped by the inventory in
`docs/inventory/templating.md`. The deciding evidence is the golden test table
extracted from the original TypeScript tests; if the subset turns out to be
much larger than expected, this ADR is revisited before M1 closes.

## Consequences

- Full control over errors, async template tags and environment chaining.
- We own the parser and must keep it aligned with the original test table.
