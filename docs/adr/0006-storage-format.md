# 0006 — Storage format

- Status: proposed
- Date: 2026-11-10

## Context

The original stores data in NeDB files per model type under a
platform-specific directory. A new format is needed for Heric, while the
reader must still migrate existing Insomnia installations.

## Options

1. One JSON document per project with the full tree.
2. SQLite with a schema per entity type.
3. NeDB-compatible files, keeping the original layout.

## Decision

Proposed: option 2, SQLite, with a migration layer that imports NeDB
directories. Rationale: transactional writes, no full-file rewrites on every
edit, and a stable file format across versions. The inventory in
`docs/inventory/storage.md` supplies the exact paths and migration steps.

## Consequences

- A migration tool must round-trip every entity type; golden fixtures from a
  real installation cover it in M2.
- The format is decided before M2 code starts, per the project rules.
