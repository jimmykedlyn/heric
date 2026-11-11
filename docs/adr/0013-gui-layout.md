# 0013 — GUI layout without a docking crate

- Status: accepted
- Date: 2026-11-10

## Context

The architecture design (§16.2) named `egui_dock` to host the panels. Two
facts discovered while writing the contracts make that impossible as written.

**The whole stack is on egui 0.37.** The MSRV of 1.95 is the floor imposed by
`eframe`, `egui` and `egui_extras` 0.37, which are the versions `cargo` resolves
for this workspace.

**`egui_dock` is on egui 0.36.** Its newest release, 0.21.1, declares
`egui = "0.36.2"`. Cargo therefore links two copies of `egui`, and a
`DockState` built against 0.36 cannot be rendered from a 0.37 `Ui`: the
`egui::Ui`, `Context` and `Id` types are distinct types across those versions.
The dependency pins the *whole* application UI, not just the dock.

**Docking is not a parity requirement.** `docs/inventory/FEATURES.md` has no
row whose acceptance needs draggable, dockable tabs. The layout in §16.2 is a
fixed three-region frame (sidebar, request, response) with tabs inside each
region, which is what the original renders in practice.

## Options

1. **Pin `egui` 0.36 for the whole workspace** and keep `egui_dock` 0.21.1.
   Costs the current release of every egui crate, including
   `egui_extras`' tables and `egui_kittest`'s harness, for a feature with no
   parity row. Also splits the MSRV story: 0.36 is older, so it is buildable,
   but it freezes the stack at the version the dock happened to target.
2. **`egui_stack` 0.25.6** — a continuation of `egui_dock`, MIT, egui ^0.37,
   MSRV 1.95. Fits technically, but it is a young fork (about 1.8k downloads)
   and adopting it betrays the project's preference for a mainline crate.
3. **`egui_tiles` 0.18.0** — rerun's tile-tree widget, MIT OR Apache-2.0,
   egui ^0.37, MSRV 1.95, about 2.6M downloads. Mainline, but it is a tile
   tree (splits with arbitrary depth and orientation), which is more layout
   model than the fixed regions need today.
4. **`egui`'s own panel API** (`Panel::top`, `Panel::left`, `Panel::right`,
   `CentralPanel`), with per-region tab strips drawn by Heric, and
   `egui_extras::TableBuilder` for the key-value grids.

## Decision

Option **4** now, with **`egui_tiles` 0.18.0 pinned in
`[workspace.dependencies]` as the approved dock** for a later milestone, behind
this ADR rather than as an unrecorded addition.

Reasons: it keeps one `egui` version in the tree, it adds no dependency to the
GUI for a capability no parity row asks for, and the §16.2 regions are fixed by
design. `TableBuilder` still renders the grids and cookie tables. The docking
crate is pinned now so that the first milestone that *does* need draggable
splits is not blocked on an approval cycle; this ADR is the approval.

`egui_stack` is rejected: a fork is a maintenance commitment, and option 4
needs no dock at all. Pinning `egui` to 0.36 is rejected because it downgrades
every other egui crate for a single widget.

## Consequences

- `egui_dock` is removed from the workspace and from `apps/gui`. One `egui`
  version is linked.
- The layout is `Panel::top` for the toolbar, `Panel::left` for the sidebar,
  `Panel::right` or a second `Panel::left` for the response pane, and
  `CentralPanel` for the request editor. Each region owns its own tab strip.
- Layout state (panel widths, open tabs, split visibility) is persisted per
  workspace in the UI-state file described by ADR 0009, not in the document
  store.
- If a milestone later needs draggable splits, it adopts `egui_tiles` behind a
  single `DockHost` seam in `apps/gui`; no service or port in `heric-core`
  changes.
- §16.2 of the design document is amended to match this ADR.
