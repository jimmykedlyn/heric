<!-- Extracted from the tail of docs/plans/M1-first-request.md; this is the
     parity-row map for the whole milestone, not part of Task 22. -->

## Row traceability

All 57 M1 rows in `docs/inventory/FEATURES.md`, each mapped to exactly one task.

| Rows | Task | What the task must make true |
|---|---|---|
| PLT-001, PLT-002, PLT-007 | 1 | Platform data paths, `INSOMNIA_DATA_PATH`, platform detection |
| STO-001, STO-002, STO-003 | 2 | The store over all 44 kinds, injected per binary, full CRUD with upsert |
| STO-004, STO-005 | 3 | Descendant traversal and the change broadcast |
| STO-015, STO-020 | 4 | Body and timeline blobs; datafiles refused to templates and plugins |
| MOD-001, MOD-002, MOD-003, MOD-006, MOD-017, MOD-030, MOD-035, FIX-006 | 5 | Base fields, `initModel`, registry, scratchpad sentinels, request defaults, groups, environments |
| MOD-005, MOD-007, MOD-009, MOD-010, MOD-012, MOD-032, MOD-037 | 6 | Workspace scopes, parent rule, meta lazily, project variants and order, base environment |
| MOD-015, MOD-020, MOD-021, MOD-022, MOD-023, MOD-024, MOD-025, MOD-026, MOD-027, MOD-031 | 7 | Request and response models, creation, pruning, latest lookup, gunzip, history delete, snapshots, path parameters, sort keys |
| MOD-054 | 8 | Settings defaults and repair |
| TPL-002, TPL-003 | 9 | Comment stripping and the no-delimiter short-circuit |
| TPL-001, TPL-005 | 10 | Strict variables and filters, limits, structured errors with line and column |
| TPL-006, TPL-014 | 11 | Argument tokenizer; the tag contract and its thirteen tags |
| TPL-004, TPL-008, TPL-009, TPL-010, TPL-011, TPL-015 | 12 | Dual context, layering, three passes, traversal, injection guard, masking |
| TPL-012, TPL-030, TPL-040, FIX-011 | 13 | Request-level render, error paths, breadcrumb, golden expectations |
| FIX-010 | 14 | The 99-case Liquid conformance suite |
| FIX-012 | 15 | The engine and the pipeline table test |
| STR-001 | 16, 17 | The contracts realised end to end: one wiring per binary, the send path |
| PLT-015 | 20 | macOS window lifecycle |
| STR-002 | 21 | The smoke suite and its harness |

Rows with no dedicated task are deliberate: `MOD-024`'s file-ownership check is the blob store's `remove` in Task 4, and `MOD-025` is Task 3's cascade plus that removal, both proved in Task 17. Every row above ends the milestone in `Done` or, if deferred, `In progress` with the reason recorded in Task 22.
