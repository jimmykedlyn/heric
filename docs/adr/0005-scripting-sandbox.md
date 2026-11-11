# 0005 — Scripting sandbox

- Status: accepted
- Date: 2026-11-10

## Context

Pre-request and after-response scripts run user JavaScript against the
`insomnia.*` and `pm.*` API, with a time budget and no filesystem or network
access.

## Options

1. `rquickjs` — QuickJS bindings, small, fast to embed, clear sandbox story.
2. `boa_engine` — pure Rust, no C dependency, slower.
3. `deno_core` — V8, full modern JS, large binary and longer builds.

## Decision

Proposed: `rquickjs`. It has the best build time to compatibility ratio and
vendors QuickJS, so the three platforms build without a system JS engine.
Revisit if script compatibility tests expose missing language features.

## Consequences

- A C compiler is required at build time (already true for `protoc`-driven
  crates on the build matrix).
- The `insomnia.*` surface is implemented in Rust as host functions.
