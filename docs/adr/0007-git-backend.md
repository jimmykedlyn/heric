# 0007 — Git backend

- Status: accepted
- Date: 2026-11-10

## Context

Git sync needs clone, pull, push, branch and diff operations on collections.
The original uses `isomorphic-git` in the renderer.

## Options

1. `gix` — pure Rust, MIT/Apache-2.0, actively developed.
2. `git2` — libgit2 bindings; libgit2 is GPL-2.0 with a linking exception,
   which needs an explicit legal review under this project's license policy.

## Decision

Proposed: `gix`. It avoids the GPL question entirely and matches the
`cargo-deny` allow-list.

## Consequences

- Some porcelain operations are less complete than libgit2; anything missing
  is implemented on top of the lower-level API.
- Revisit only with an ADR and legal review if a required operation is
  impractical.
