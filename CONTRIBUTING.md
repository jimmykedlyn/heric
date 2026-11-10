English | [Português](CONTRIBUTING.pt-BR.md) | [Español](CONTRIBUTING.es.md)

# Contributing to Heric

Thanks for helping. This document explains the branch flow, the quality gates
and the review process. It applies to every change, including documentation.

## Code of conduct

Be respectful. Discuss the code, not the person. Disagreements about design
are settled with evidence from the original client or from a test, not by
seniority.

## Branch model (Gitflow)

| Branch | From | Into | Purpose |
|---|---|---|---|
| `main` | — | — | Released code only. Every commit on `main` has a `vX.Y.Z` tag. |
| `develop` | `main` | — | Integration. Default branch of the repository. |
| `feature/<crate>-<task>` | `develop` | `develop` | Every feature, fix, docs or CI change. |
| `release/X.Y.0` | `develop` | `main` + `develop` | Release preparation: version bump, changelog, fixes only. |
| `hotfix/X.Y.Z` | `main` | `main` + `develop` | Urgent fix on a released version. |

Rules:

- Never commit directly to `main` or `develop`. Everything goes through a
  pull request.
- `feature/*` targets `develop` and is squash merged. Delete the branch after
  the merge.
- `release/*` and `hotfix/*` target `main` with a merge commit, no squash.
  After the merge, tag `vX.Y.Z` on `main` and open a back-merge PR from `main`
  to `develop`.
- Only version bumps, `CHANGELOG.md`, documentation adjustments and bug fixes
  go into a `release/*` branch. No new features.
- Rebase your feature branch on `develop` before merging if `develop` moved.

The CI `gitflow` job validates the source and target branch pair on every PR
and fails on any other combination.

## Development setup

1. Install the Rust toolchain. `rust-toolchain.toml` pins the channel and the
   components, so `rustup` picks it up automatically.
2. On Linux, install the desktop dependencies: `libxkbcommon-dev`,
   `libgtk-3-dev`, `libxcb-render0-dev`, `libxcb-shape0-dev`,
   `libxcb-xfixes0-dev`, `libssl-dev` and `protobuf-compiler`.
3. Build and test:

```bash
cargo build --workspace --all-targets
cargo test --workspace
```

## Quality gates

A change is mergeable only when all of these pass locally and in CI on the
three platforms:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

## Coding rules

- No `unwrap()` or `expect()` in library code. Libraries use `thiserror`;
  only binaries use `anyhow`.
- Keep functions small, documented in rustdoc, and free of platform-specific
  paths. Use `directories` for configuration and data locations.
- New behavior needs a failing test first (red, green, refactor).
- Behavior must match the original client. Cite the source file and line in
  the code comment or the PR body when implementing parity.

## Commit messages

Conventional Commits, in English:

```
feat: add response timeline panel
fix(http): keep trailers on chunked responses
docs: explain template tag chaining
```

Use a scope for the affected crate when it helps.

## Pull requests

- Target `develop` (or `main` only from `release/*` and `hotfix/*`).
- Reference the issue with `Closes #123`.
- Fill the checklist in the PR template.
- If the change affects user-facing behavior, update the English
  documentation in the same PR and include the `pt-BR` and `es` translations.
- Wait for CI on all three platforms. A red platform blocks the merge.

## Documentation

English is the source of truth. Every change to it ships with `pt-BR` and
`es` updates in the same PR. Code blocks, commands and flag names are never
translated. Follow the terminology in `docs/book/GLOSSARY.md`.

## Reporting bugs and requesting features

Use the issue templates. For bugs, include the operating system, the version
(visible in the About dialog) and the smallest reproduction you can.

## License

By contributing you agree that your contribution is licensed under the MIT
License, and that portions derived from Insomnia remain under the Apache
License 2.0 as described in `NOTICE`.
