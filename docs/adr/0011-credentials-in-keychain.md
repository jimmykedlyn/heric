# 0011 — Credentials in the OS keychain

- Status: accepted
- Date: 2026-11-10

## Context

Heric stores secrets of four kinds: environment values the user marked
confidential, OAuth 2.0 access and refresh tokens, Git credentials, and the
vault key that protects imported Insomnia secrets. The original keeps them in
Electron's `safeStorage` — DPAPI on Windows, a Keychain item on macOS, and a
plaintext fallback on Linux — with the ciphertext kept in
`<dataPath>/localStorage/`.

Writing secrets into SQLite, even encrypted with a key we ship, would mean
shipping that key, which is not encryption. Writing them in plaintext
contradicts the project's anti-goal about silent exposure.

## Options

1. **The `keyring` crate** — one API over Keychain, Credential Manager and
   Secret Service.
2. **A local encrypted file** with a key derived from the OS credential via
   our own FFI.
3. **No persistent secrets**: prompt every time.

## Decision

Option 1, the `keyring` crate, keyed by collection and entity id.

- Secrets never enter SQLite and never enter the timeline in clear. The
  documents keep a reference, not a value.
- When the keychain is unavailable (a headless Linux host with no Secret
  Service), the core enters a **degraded mode** at startup and the CLI refuses
  the operations that need a secret. It does not fall back to a plaintext
  file: a silent downgrade is worse than a refusal, and the original's
  plaintext fallback is a defect worth not reproducing.
- Imported secrets that arrived as Electron ciphertext are a separate state,
  **pending re-entry** (design §9, §17). They are stored as ciphertext,
  flagged in the migration report, and clear when the user re-enters the
  value. Decryption needs platform FFI and, on macOS, the item is bound to the
  Electron application, so it is not reachable from a different binary.
- `PLT-005`: opt-in persistence of the vault key as `vault_<accountId>`,
  bypassed by `INSOMNIA_VAULT_KEY` in tests.
- `PLT-006`: Git credentials delegate to `git credential fill` and are never
  stored by us.

## Consequences

- A dependency on a crate that itself wraps three platform APIs. Its licence
  clears the allow-list.
- Headless CI and the CLI need a defined story: the test harness uses the
  `INSOMNIA_VAULT_KEY` bypass, and the CLI reports degraded mode in its
  `--json` output instead of failing obscurely.
- A secret's lifetime is the OS's, not ours: locking the session may drop the
  key, and the app must handle a read that succeeds at startup and fails
  later.
- Backups never contain secrets, so restoring a backup on another machine
  leaves secrets pending, which the UI reports rather than hiding.
