# Release process

## Cutting a release

1. Branch `release/X.Y.0` from `develop`.
2. Bump `version` in `[workspace.package]` of the root `Cargo.toml`.
3. Regenerate `CHANGELOG.md` with `git-cliff --tag vX.Y.0 --output CHANGELOG.md`.
4. Confirm the documentation in the three languages describes every new
   user-facing feature.
5. Open a PR from `release/X.Y.0` to `main`. Merge with a merge commit.
6. Tag on `main`: `git tag -a vX.Y.0 -m "vX.Y.0" && git push origin vX.Y.0`.
7. The release workflow builds and publishes the installers, checksums and
   GitHub Release.
8. Back-merge: PR from `main` to `develop`.

## Artifacts

| Platform | Artifacts |
|---|---|
| Windows | portable `.zip`, `.msi` |
| Linux | `.tar.gz`, `.deb`, `.AppImage` |
| macOS | `.dmg` with a universal binary (arm64 + x86_64) and `.app` bundle |
| All | `heric-cli` archive per platform, `SHA256SUMS` |

## Code signing secrets

Signing is optional. When the secrets are absent the workflow publishes
unsigned artifacts and the release notes say so.

| Secret | Purpose |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | password for that `.p12` |
| `APPLE_ID` | Apple ID for notarization |
| `APPLE_TEAM_ID` | team identifier |
| `APPLE_APP_PASSWORD` | app-specific password for notarization |
| `WINDOWS_CERTIFICATE` | base64 of the code signing `.pfx` |
| `WINDOWS_CERTIFICATE_PASSWORD` | password for that `.pfx` |
