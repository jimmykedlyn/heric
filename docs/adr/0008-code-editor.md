# 0008 — Code editor widget

- Status: proposed
- Date: 2026-11-10

## Context

Request bodies, headers and response bodies need syntax highlighting, line
numbers, search and virtualization for multi-megabyte documents inside egui.

## Options

1. `egui_code_editor` — ready-made widget with several languages.
2. A custom widget on top of `egui_extras` with a highlighting crate
   (`syntect` or `tree-sitter`) and a virtualized layout.
3. `egui_text_edit`-style text area with a separate read-only highlighted view
   for responses.

## Decision

Proposed: option 2 for responses (read-only, virtualized, syntax highlighted
with `syntect`) and option 1 for small editable fields, pending a spike in M1.

## Consequences

- Response rendering stays cheap for large bodies; the editable path is
  simple and replaced later if the widget proves weak.
