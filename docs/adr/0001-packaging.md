# 0001 — Packaging and installers

- Status: accepted
- Date: 2026-11-10

## Context

The release workflow must produce installers for Windows (`.zip`, `.msi`),
Linux (`.tar.gz`, `.deb`, `.AppImage`) and macOS (universal `.dmg` plus `.app`)
from GitHub Actions, without a large amount of bespoke shell code that we
would have to maintain on three platforms.

Candidates:

- **`cargo-dist`** — opinionated, generates installer workflows from
  `dist-workspace.toml`, handles the matrix, checksums, and GitHub Release.
  Installer coverage for `.msi`, `.deb`, `.AppImage` and `.dmg` is good but
  the universal macOS binary and notarization still need custom steps.
- **`cargo-packager`** — library and CLI, generates `.msi`/`.dmg`/`.deb`/
  `.AppImage` from a `Packager.toml`. Straightforward to call from a matrix
  job and less workflow generation to fight.
- **Manual workflow** — maximum control, highest maintenance.

## Decision

Use **`cargo-packager`** in a matrix job of a hand-written `release.yml`.

Reasoning: the artifact set in the prompt matches what `cargo-packager`
produces natively, the configuration stays in the repository as one file, and
the workflow keeps explicit control over the macOS `lipo` universal binary
step and notarization, which no generator handles exactly the way we want.
`cargo-dist` would still need custom steps for those two, so its main
advantage disappears.

The CLI (`heric-cli`) is packaged as a plain archive per target instead of an
installer.

## Consequences

- The workflow has about 150 lines of YAML we own and must test on each
  release rehearsal.
- `cargo-packager` must build on each runner natively; the macOS universal
  binary is produced by building both targets and running `lipo`.
- Revisit if `cargo-dist` starts handling notarization and universal binaries
  natively.
