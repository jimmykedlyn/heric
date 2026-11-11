# Heric architecture — design

- Date: 2026-11-10
- Status: approved by the owner on 2026-11-10
- Scope: Phase 1 (architecture and contracts). Implementation starts in M1.

## 1. What this document is

A description of the system Heric will be, written so that a competent Rust
engineer can plan and build it without coming back to the author. It records
the decisions the owner made, the decisions this document makes on the
owner's behalf, and the constraints that are not negotiable.

It is not a plan. The per-milestone plans in `docs/plans/` come next (the
directory is created by the first milestone plan), and the ADRs in `docs/adr/`
record each architectural choice with its alternatives.

## 2. Intent

Reimplement the Insomnia API client as a native desktop application in Rust
with an egui front end, so that a user can point Heric at an existing
Insomnia installation, keep working with the same collections, requests,
environments, cookies and certificates, and send the same requests through an
engine that behaves the way libcurl behaves — because that is the engine the
original uses.

The reason this exists: Insomnia is an Electron application. A native build
removes the browser runtime from the request path, cuts the memory and
startup cost, and lets the same code run headless as a CLI for CI use.

## 3. Goals

- **Data compatibility.** Read a real Insomnia installation (NeDB directories
  at the platform paths) and every export format the original produces,
  without loss. Write the v5 YAML export byte-identically after normalisation.
- **Behavioural parity.** For a given request, the bytes on the wire, the
  headers, the cookies set and the response the user sees match the original,
  including its quirks.
- **One codebase, three front ends.** The GUI (`heric`), the CLI
  (`heric-cli`) and the test harness all drive the same `heric-core`
  services. No behaviour lives only in the GUI.
- **Never block the UI.** No request, disk read or script keeps a frame from
  being drawn.
- **Three platforms from day one.** Windows x86_64, Linux x86_64, macOS Apple
  Silicon and macOS Intel all run the full test suite on every pull request,
  each on a runner of its own architecture. The current CI builds Intel from
  the Apple Silicon runner and marks it `build_only`; that stops being true in
  M1, and the `test` job gains an Intel image.

## 4. Non-goals

- Cloud sync, Kong account login, Konnect organisation features and
  telemetry. Interfaces exist as stubs, nothing is implemented; see
  `docs/out-of-scope.md`.
- Plugin execution. The original runs user JavaScript plugins with a
  capability sandbox. Heric does not host a JS plugin runtime in v1. Template
  tags and scripts are supported; third-party plugin bundles are not.
- Rewriting Insomnia's on-disk data. Heric reads it and migrates a copy into
  its own store. It never writes into `~/.config/Insomnia` or
  `%APPDATA%\Insomnia`.
- Mobile, browser and web builds. Desktop only.

## 5. Anti-goals

Things that would mean the project failed even if it shipped:

- **A silent data loss.** Migrating a user's collection and dropping a field,
  a sub-environment or a cookie jar is a worse outcome than not migrating.
  Every migration path is reversible by keeping the source directory intact,
  and every entity type is round-tripped by a golden test before M2 closes.
- **A frozen UI.** Any `block_on`, `join`, or synchronous file read on the
  egui thread is a defect, not a shortcut.
- **A second dialect of the template language.** If a user's request renders
  differently than it does in Insomnia, the tool is not a replacement.
- **A blob that cannot be reviewed.** One crate that does everything, or
  shared mutable state reached through globals, would make the parallel
  development in Phase 2 impossible.

## 6. Constraints

Non-negotiable, from `PROMPT-HERIC.md`:

- **License.** MIT for the work, `NOTICE` attributing the Apache-2.0 Insomnia
  derivation, no Insomnia or Kong branding. `cargo-deny` allow-list as
  configured; GPL, AGPL and LGPL are blocked.
- **No `unwrap()` or `expect()` in library code.** `thiserror` in libraries,
  `anyhow` only in binaries. Enforced by workspace lints.
- **`unsafe_code = "forbid"`.** If a dependency needs `unsafe`, that is the
  dependency's business, not ours.
- **MSRV 1.95**, Rust 2024 edition. This floor comes from eframe/egui 0.37.
- **Gitflow.** Nothing is committed directly to `main` or `develop`.
  Phase 1 lands as `feature/architecture` through a pull request.
- **Quality gates.** `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, `cargo deny check`, and CI green on the four
  release targets.
- **The original source is read-only.** `D:\Projects\Heric\Insomnia` is
  consulted, never modified.

## 7. The crate graph

```text
apps/gui   apps/cli
      \       /
     heric-core
   /   |     |     |     |      |        |         |        |
import script prot auth http git openapi codegen template protocols
   \    |     |    |     |      |        |         |        |    /
     +--+-----+----+-----+------+--------+---------+--------+---+
                              |
                    heric-model   heric-storage
```

`heric-model` has no Heric dependencies. `heric-storage` depends on
`heric-model` and on nothing else inside the workspace. Every other library
crate depends on `heric-model`, and on `heric-storage` only when it genuinely
persists something (`heric-git`, `heric-import`). `heric-core` depends on all
of them. The two binaries depend only on `heric-core` and on presentation
crates (`eframe`, `egui`, `clap`).

No library crate may depend on `apps/*`, on `eframe`, `egui` or `clap`. The
`clippy` and `cargo-deny` gates do not catch a layering violation; the review
does, and the rule is written down here so it is reviewable.

### 7.1 Ports and adapters

Two crates need capabilities they must not own:

- `heric-template` executes `{% response %}` and `{% file %}`, which need
  network and filesystem access.
- `heric-script` exposes `insomnia.sendRequest`, which needs the HTTP engine.

Both would create a cycle if they depended on `heric-http`. Instead each
defines a *port*: a trait describing the capability it needs, in its own
crate. `heric-core` implements those traits and injects the implementations.
This is why `heric-core` exists as a crate rather than as a module of the GUI.

The same pattern applies to storage: `heric-import` writes through an
`ImportSink` port and `heric-git` through a `CollectionFs` port, so neither
crate knows about SQLite.

## 8. The data model

### 8.1 Documents, not a 44-variant enum

The original has 44 entity types sharing a base (`_id`, `type`, `parentId`,
`modified`, `created`, `isPrivate`, `name`) and differing in everything else.
A Rust enum with 44 variants would be type-safe and would make the migration
reader brittle: any field the inventory missed would be dropped silently,
which is exactly the anti-goal in section 5.

The chosen shape is a document with typed views:

```rust
pub struct Doc {
    pub meta: DocMeta,           // _id, kind, parent, modified, created, private, name
    pub body: serde_json::Value, // the remaining fields, verbatim
}

pub enum DocKind { Request, RequestGroup, Workspace, /* ... 44 total */ }

pub struct Request<'a> { /* typed view borrowed from a Doc */ }
impl<'a> TryFrom<&'a Doc> for Request<'a> { /* ... */ }
```

`Doc` round-trips unknown keys. Typed views give ergonomics where the code
needs them. `DocKind` gives exhaustive matching where a new type must not be
forgotten. The migration layer reproduces `initModel` exactly, including the
pruning of keys outside the model's defaults and `optional_keys` — pruning
belongs to the *migration*, not to `Doc`, so imported data matches Insomnia's
in-memory view while Heric's own store keeps what it was given.

### 8.2 Identity

`Id` stays a transparent newtype over `String`, because ids are opaque
strings with a type prefix (`req_`, `wrk_`, `env_`) and half the original
code compares them as strings. `heric-model` provides `Id::generate(kind)`
producing `${prefix}_${uuid_v4_without_dashes}`, matching
`common-src/misc.ts`.

### 8.3 The 44 types

`DocKind` is generated from the list in `docs/inventory/models.md` and keeps
the original type names verbatim (`SocketIORequestMeta`, `RunnerTestResult`)
because they appear in the files Heric reads and writes. The Rust enum uses
`#[serde(rename = "...")]` so the wire form is the original string.

### 8.4 Mutations

Every mutation is expressed as a `Patch` — a map of field names to JSON
values, with a sentinel for "remove this field" — and never as a whole
document replacement. Two reasons: the original's `docUpdate` merges patches
and re-stamps `modified`, and concurrent edits from the autosave buffer and
from a script must not clobber each other.

## 9. Storage and migration

**Decision (owner):** read the Insomnia NeDB directories and migrate into a
Heric-owned data directory. Never write back.

**Decision (ADR 0006):** the new store is SQLite via `rusqlite`, with a `meta`
table carrying a schema version. Whether each entity type gets its own table
or shares one table with a `kind` discriminator is left to the builder
(section 22); `Store` is the only access path either way.

Layout:

```text
<platform data dir>/Heric/
  heric.db              SQLite database
  backups/<timestamp>/  copy of the source NeDB directory taken before import
  logs/
```

Platform data directories come from the `directories` crate: `%APPDATA%\Heric`
on Windows, `~/.local/share/Heric` (with `~/.config/Heric` for config) on
Linux, `~/Library/Application Support/Heric` on macOS. The Insomnia source
directories are detected, not assumed: `%APPDATA%\Insomnia`,
`~/.config/Insomnia`, `~/Library/Application Support/Insomnia`, plus a
`INSOMNIA_DATA_PATH` override when the user points at one.

The storage crate exposes one trait and hides SQLite behind it:

```rust
pub trait Store: Send + Sync {
    fn get(&self, id: &Id) -> Result<Option<Doc>, StorageError>;
    fn find(&self, query: &Query) -> Result<Vec<Doc>, StorageError>;
    fn insert(&self, doc: &Doc) -> Result<Doc, StorageError>;
    fn update(&self, id: &Id, patch: &Patch) -> Result<Doc, StorageError>;
    fn remove(&self, id: &Id) -> Result<Vec<Id>, StorageError>; // returns descendants removed
    fn changes(&self) -> tokio::sync::broadcast::Receiver<Change>;
}
```

`Change` is the `db.changes` broadcast the GUI tab bar depends on: deleting a
request closes its tab. Descendant deletion walks the parent map, mirroring
`getAllDescendantMap`.

Response bodies and timelines are not documents. The original writes them as
`responses/<id>.response` and `responses/<id>.timeline` side files and the
`Response` document only carries the path (`STO-015`). They migrate, and the
port that reads them afterwards keeps the medium behind it, so nothing in
Phase 1 ties stored bytes to a file on disk:

```rust
pub trait BlobStore: Send + Sync {
    fn put(&self, id: &BlobId, bytes: &[u8]) -> Result<(), StorageError>;
    fn get(&self, id: &BlobId) -> Result<Option<Vec<u8>>, StorageError>;
    fn stream(&self, id: &BlobId) -> Result<Box<dyn Read + Send>, StorageError>;
    fn remove(&self, id: &BlobId) -> Result<(), StorageError>;
}
```

`stream` exists because a multi-megabyte body must not be materialised twice on
its way to the response viewer (section 16.3).

**Write buffering.** The original buffers changes and flushes on a timer: a
1000 ms window by default, 300 ms for the Git filesystem bridge, and 100 ms
per key for the Electron key-value store. The GUI needs the same thing for one
reason: typing in a request body must not cause a SQLite write per keystroke.
The core keeps a pending patch set and flushes on a 1000 ms debounce, on blur
and on send. This is UI-visible behaviour (the original's autosave cadence)
and belongs in M1. The exact default is a settings value, so the number is a
starting point rather than a constant.

**Migration runs once, verified.** Before the first import Heric copies what it
will read to `backups/<timestamp>/`: the `insomnia.*.db` files and the other
small state files. It does not copy `responses/`, `version-control/` or the
original's own `backups/` directory. Those are read in place and never written,
so they need no copy, and copying them would turn a five-second migration into
a multi-gigabyte one on a heavy installation. The import is a single SQLite
transaction; a failure rolls back and the user keeps an untouched Insomnia
installation. `heric-cli import` performs the real migration and `--dry-run`
prints the per-type counts without writing, because migration cannot be a
GUI-only path (section 3).

**Secrets the migration cannot read.** Not everything secret is in NeDB. The
original stores environment secret values as AES-GCM ciphertext inside the
`Environment` document, and the key that decrypts them lives in
`<dataPath>/localStorage/<key>`, itself encrypted by Electron `safeStorage`
(DPAPI on Windows, a Keychain item on macOS, and a plaintext fallback on Linux)
— `docs/inventory/storage.md`, `docs/inventory/structure.md`. The vault key may
also derive from a Kong account session, which is out of scope (`§4`).

**Decision (owner):** the migration copies that ciphertext verbatim and does
not attempt to decrypt it. Each affected secret is marked *pending* in the
migration report, with the environment and key name, and the GUI opens the
environment editor with those rows highlighted so the user re-enters each value
once. The reasoning: decrypting `safeStorage` needs platform FFI, and on macOS
the item is bound to the Electron application, so a correct implementation is
not reachable from a different binary; on Linux the fallback is often plaintext
anyway, which Heric's own rule forbids storing. Copying the ciphertext is
lossless and reversible, and the parts the vault key was never saved for would
have been unreadable regardless.

Consequences: an imported secret is never silently empty — a pending secret
renders as a masked placeholder plus a warning, and sending a request that
references one fails with `SecretPendingReentry` rather than sending a blank
header. The M2 golden test asserts that a fixture with a private environment
imports with its ciphertext byte-identical and its keys marked pending.

**NeDB reading.** NeDB files are newline-delimited JSON with a few metadata
lines (`$$indexCreated`, `$$deleted`). The reader applies appends and deletes
in file order, ignores the index records, and hands raw documents to the
migration layer. It never assumes one file per type: it globs `insomnia.*.db`
and derives the type from the document's `type` field.

## 10. The HTTP engine

**Decision (owner, on the recommendation below):** the `curl` crate
(libcurl), not `reqwest`.

The reason is fidelity. The original's timeline, its multipart building, its
redirect handling, its cookie jar semantics and its NTLM/Digest support all
come from libcurl. Using the same engine removes an entire class of "works in
Insomnia, not in Heric" bugs, and the Console timeline (`HTTP-052`, P0) is
literally libcurl's debug callback rendered on screen.

The cost, stated plainly:

- libcurl is built from source through `curl-sys`, which compiles the C
  sources directly with the `cc` crate. It does not invoke cmake, make,
  autotools or vcpkg, so the build needs a C toolchain and nothing else. The
  Windows path rewrites `config-win32.h` for MSVC and needs no nasm; nasm
  appears only if the `rustls` or `static-ssl` feature drags in `aws-lc-sys`
  or `openssl-src`, which is one more reason to avoid those features (see
  below). A local Windows checkout still needs the MSVC build tools to compile
  any C at all, which is the same prerequisite the `egui` stack already has.
- The TLS backend is a compile-time feature choice, not a runtime one, and the
  default (`ssl`) is the platform backend: Schannel on Windows, OpenSSL
  elsewhere. This is where the design departs from the prompt's instruction:
  the prompt asked for `rustls` plus `rustls-platform-verifier`, and libcurl
  can only reach rustls through `rustls-ffi`, which libcurl itself documents
  as experimental and which is broken in exactly the combination this project
  would use. Checked against the libcurl 8.22.0 sources and the `curl-sys`
  build script on 2026-11-10:

  | Combination | Trust store | Cost |
  |---|---|---|
  | `ssl` (Schannel on Windows, OpenSSL on Linux) plus `apple-sectrust` on macOS | native on all three | OpenSSL on Linux means a system `libssl` or a vendored build; `apple-sectrust` evaluates the macOS Keychain policy |
  | `rustls` feature | `SslOpt::native_ca` routes to `rustls-platform-verifier` on Windows and macOS, but on Linux it means the Mozilla bundle, not the system store | pulls `rustls-ffi` and `aws-lc-sys`, needs nasm on Windows, and `curl-sys` issue #687 makes rustls plus `static-curl` fail every HTTPS request until `native_ca` is set on each handle |

  Two facts decide this in favour of the first row. Secure Transport was
  removed from curl in 8.15.0, so the README's "Secure Transport on macOS"
  claim is stale and the macOS answer is OpenSSL plus `apple-sectrust`, or
  rustls; and libcurl's rustls backend has been marked EXPERIMENTAL since it
  landed in 7.76.0. The requirement does not move: system trust roots on all
  three platforms, reproducible builds, no hand-maintained CA bundle. ADR 0010
  records the decision after the build spike, and if no combination satisfies
  the requirement, that is a reason to revisit the engine before M1 closes.
- NTLM is at risk, and it is P0 (`HTTP-037`). `curl-sys` 0.4.91 does not
  define `CURL_ENABLE_NTLM`, which libcurl has required since 8.20, and it does
  not define `HAVE_DES_ECB_ENCRYPT`, so the `ntlm` feature compiles and
  produces no NTLM symbols on Unix (`curl-sys` issue #665, open). On Windows
  `config-win32.h` defines `USE_WIN32_CRYPTO` and NTLM probably still works,
  but that is unverified. The plan therefore treats NTLM as deliverable on
  Windows and as depending on an upstream fix or a `curl-sys` patch elsewhere,
  and the milestone plan carries that risk explicitly instead of discovering it
  at M3. HSTS is a smaller gap: `curl-sys` exports no `CURLOPT_HSTS`, so Heric
  keeps its own HSTS handling or does without.
- The binary grows by a few megabytes. The crates as published on
  2026-11-10: `curl` 0.4.51 and `curl-sys` 0.4.91, which vendors libcurl
  8.22.0. Both are MIT and clear the allow-list at the crate level. The
  vendored tree is another matter: libcurl itself ships under the `curl`
  SPDX licence (MIT/X with a name-use clause), plus `ISC` and
  `BSD-4-Clause-UC` for two source files. None of those is copyleft and none
  requires source disclosure, but a static binary carries an attribution
  obligation, so the release archives ship a `THIRD-PARTY-NOTICES` file. The
  `deny` job cannot enforce this: it sees the crate metadata, which says MIT,
  and never reads the vendored C tree. The notice file is what actually
  discharges the obligation, which makes generating and shipping it a release
  step rather than a lint.
- The crate is `unsafe` internally. That is acceptable: it is a `-sys`
  binding, the one category of dependency that cannot be written in safe
  Rust, and our own crates keep `unsafe_code = "forbid"`. The same goes for
  the precision limit below: `_TIME_T` counters are not bound, and reaching
  them would mean raw FFI from our own code, which the workspace forbids.


The engine sits behind a trait so a different backend can be added without
touching the pipeline, and so tests can substitute a fake:

```rust
pub trait HttpEngine: Send + Sync {
    fn execute(
        &self,
        request: PreparedRequest,
        options: ExecuteOptions,
    ) -> Result<Execution, HttpError>;
}
```

`PreparedRequest` is fully rendered — the engine never sees a template.
`Execution` carries the status, headers, body bytes, the timeline and the
per-phase timings.

**Timing precision.** The `curl` crate exposes exactly seven phase getters —
`namelookup_time`, `connect_time`, `appconnect_time`, `pretransfer_time`,
`starttransfer_time`, `redirect_time` and `total_time` — each returning a
`Duration` converted from libcurl's `f64` seconds. The microsecond `_TIME_T`
variants are not bound by `curl-sys`, so a waterfall that matches the
original's *phases* will not match its *numbers* to the last digit. The
waterfall is a picture, not a measurement artefact; the wire tests compare
phase ordering and the presence of each phase, never exact durations.


**Timeline.** Two distinct things share the word, and the design keeps them
distinct:

- `TimelineEntry` — the console log (`HeaderOut`, `HeaderIn`, `DataOut`,
  `Text`), persisted as NDJSON next to the response and redacted before it is
  written. This is `HTTP-052`, P0.
- `TimingStep` — the live "Rendering request... Sending request..." indicator
  in the response header. In-memory only, broadcast as events.

Redaction is a property of the *sink*, not of the emitter: the engine pushes
everything, and the sink that writes to disk drops or masks values registered
in the sensitive-value registry. That way the on-disk timeline is safe and the
in-memory one is complete for the UI.

**Streaming.** SSE and chunked responses use a second entry point returning a
handle:

```rust
pub trait StreamingEngine {
    fn open(
        &self,
        request: PreparedRequest,
        options: ExecuteOptions,
    ) -> Result<StreamHandle, HttpError>;
}

pub struct StreamHandle { /* send, close, and a Receiver<StreamEvent> */ }
```

**Cancellation.** Every execution carries a `RequestId` and a cancellation
token. `Command::Cancel { id }` sets it; the engine's progress callback
returns abort, which is how libcurl expects to be stopped. The outcome is a
distinct `Aborted` result, not an error, because the UI shows it as a
cancelled send rather than a failure.

**Cookies.** A per-workspace jar persisted in the store, serialised to
Netscape format for libcurl, with origin tagging and rejection reporting
(`HTTP-020`, `HTTP-021`). Cookies are never re-rendered as templates
(`HTTP-023`).

**Certificates and proxy.** Client certificates resolve by host and port with
wildcards; PEM, P12 and key files are read from disk and passed to the engine.
Proxy resolution follows the original: per-request setting, then the system
proxy; a PAC result carrying several entries uses the first, and an
unparseable proxy value falls back to a direct connection instead of failing.
This is M3 work; the contract exists in M1.

**Trust store.** The original enables the operating system CA store on every
request (`PLT-024`, P0), and a workspace CA is an *addition* to it, never a
replacement: libcurl gets the native store plus the extra bundle. Heric keeps
that rule. Also worth reproducing: the workspace CA applies only when the
certificate record is enabled, and a missing CA file must fail the request
instead of silently falling back to the system store. This is the requirement
ADR 0010 has to satisfy with whichever libcurl TLS backend the build spike
selects.

The matrix row `PLT-024` describes the same behaviour as "overridden by a
workspace CA", which contradicts the inventory's "in addition to
`CAINFO_BLOB`". The inventory wins, because it cites the libcurl option that
proves it: `CurlSslOpt.NativeCa` is set unconditionally. The matrix wording is
corrected as part of this branch rather than carried as a known discrepancy.

**Body building.** Multipart, urlencoded and file uploads are built by us, not
by libcurl's form API, because the original builds them itself to control the
boundary and the part ordering (`HTTP-004`, `HTTP-005`). The engine sends the
bytes it is given.

## 11. Rendering request fields

**Decision (ADR 0003):** rewrite the template engine in `heric-template`, with
Liquid semantics.

The original's engine is LiquidJS 10.27 configured to look like Nunjucks
(`docs/inventory/templating.md`). "Nunjucks" is the name the codebase uses, not
the language it implements: filters, `strictVariables`, JS truthiness, the dual
root/`_` context and the tag argument tokenizer all follow Liquid, and the
99-case conformance suite (`TPL-001`, `FIX-010`) is the definition of done. ADR
0003 is rewritten to say this instead of "evaluates Nunjucks"; the feature name
stays in the inventory so the two remain greppable together.

The public surface is small:

```rust
pub fn render(input: &str, context: &RenderContext) -> Result<String, RenderError>;
pub fn render_request(request: &mut Doc, context: &RenderContext) -> Result<(), RenderError>;
pub fn tags() -> &'static [TagDefinition];
```

`RenderContext` carries the layered variable scopes (global root and sub,
collection root and sub, folder ancestors, iteration, transient) with the
precedence table from `docs/inventory/templating.md`, plus a
`ConfidentialPolicy` that decides what may be revealed for a given purpose
(send, preview, script, export).

`TagHost` is the port for the tags that need the outside world:

```rust
pub trait TagHost {
    fn read_file(&self, path: &Path, purpose: ReadPurpose) -> Result<Vec<u8>, TagError>;
    fn prompt(&self, prompt: &PromptRequest) -> Result<PromptAnswer, TagError>;
    fn send(&self, request: PreparedRequest) -> Result<Execution, TagError>;
}
```

`{% prompt %}` must never block outside the send purpose (`TPL-017`): the
host returns a `NotAvailableInThisPurpose` error instead of waiting.

Error reporting is part of the contract, not an afterthought: every
`RenderError` carries the message, the template path, and line and column, and
distinguishes "undefined variable" from "evaluation error" — the original's UI
branches on exactly that distinction.

The renderer is synchronous and pure apart from the `TagHost` calls, so the
worker that renders a preview and the path that renders before a send share
the same code.

## 12. Authentication

`heric-auth` implements every scheme as a function of a rendered context, with
one exception:

```rust
pub trait AuthProvider: Send + Sync {
    fn scheme(&self) -> AuthScheme;
    fn apply(&self, input: &AuthInput) -> Result<AuthOutput, AuthError>;
}
```

`AuthInput` holds the rendered URL, headers and parameters plus a handle to
the HTTP engine — OAuth 2.0 and OAuth 1.0 have to make a token request, and
AWS SigV4 has to sign the body. `AuthOutput` returns headers, query
parameters, and *timeline entries*, because the original writes `[oauth2] ...`
entries into the same timeline the user reads.

The schemes, with the inventory's priorities: Basic, Bearer, API key (header,
cookie or query), OAuth 2.0, Digest and NTLM are P0; OAuth 1.0, AWS SigV4 and
`singleToken` are P1; Hawk, ASAP and Netrc are P2. `singleToken` has no HTTP
handler in the original — it applies to Socket.IO only — so Heric must
reproduce that omission rather than invent a header. Digest and NTLM are
delegated to libcurl, which is one more argument for the `curl` engine.

Two behaviours are easy to miss and are part of the contract: the auth header
is computed from the *rendered* request and is suppressed when the user has
already set an `Authorization` header, and OAuth 2.0 token errors become
timeline entries instead of failures.

Most of this is M3. The trait, its input and output types, and Basic and
Bearer are M1.

## 13. Protocols

`heric-protocols` owns gRPC, WebSocket, Socket.IO, SSE and GraphQL. They share
a connection model:

```rust
pub trait Connection: Send {
    fn state(&self) -> ConnectionState;
    fn send(&mut self, payload: Payload) -> Result<(), ProtocolError>;
    fn close(&mut self) -> Result<(), ProtocolError>;
}
```

and emit `ProtocolEvent`s over a channel: `StateChanged`, `Message`,
`Disconnected`, `Error`. The GUI subscribes; a pane that unmounts must not
take the connection with it (`PRO-034`), so the connection lives in
`heric-core` and is keyed by request id, not by widget.

gRPC uses `tonic` with `prost-reflect` for reflection and dynamic messages.
WebSocket uses `tokio-tungstenite`, which gives the header, subprotocol and
cookie-jar control the original has. SSE is the HTTP engine's streaming path,
not a separate stack. GraphQL is HTTP with a request builder and an
introspection helper.

All of this is M4.

## 14. Scripting

**Decision (ADR 0005):** `rquickjs`.

A script runs in a fresh context with a deadline, a memory cap and an
interrupt handler, mirroring the original's sandbox discipline. The host API
is a port:

```rust
pub trait ScriptHost {
    fn read_environment(&self, scope: Scope) -> Value;
    fn write_environment(&self, scope: Scope, patch: Value) -> Result<(), ScriptError>;
    fn read_request(&self) -> PreparedRequest;
    fn write_request(&self, patch: RequestPatch) -> Result<(), ScriptError>;
    fn read_response(&self) -> Option<Execution>;
    fn send_request(&self, request: PreparedRequest) -> Result<Execution, ScriptError>;
    fn console(&self, level: Level, text: &str);
}
```

`ScriptOutcome` returns the mutations to persist, the test results, the
console lines for the timeline, and any error with its category (security
policy, runtime, timeout), because the UI renders them differently.

This is M5.

## 15. Import, export, codegen, OpenAPI and Git

Each importer implements one trait, so the dispatcher is a list and adding a
format is a new file:

```rust
pub trait Importer {
    fn format(&self) -> Format;
    fn detect(&self, input: &ImportInput) -> DetectResult;
    fn import(&self, input: &ImportInput, sink: &mut dyn ImportSink)
        -> Result<ImportReport, ImportError>;
}
```

`ImportSink` is the port into the store: the importer never talks to SQLite.
`ImportReport` counts what was created and lists what was skipped and why,
which the UI shows and the CLI prints.

Export is the inverse, and the v5 YAML writer must reproduce the original's
normalisation and key ordering (`IO-001`, with the selected-request variant in
`IO-002`). Round-trip is the acceptance test: import a fixture, export it,
compare bytes.

The YAML library is a live constraint, checked on 2026-11-10: `serde_yaml` is
deprecated upstream and `serde_yml` announces itself as unmaintained and
redirects to `noyalib`. The two candidates worth an ADR line are `serde_norway`
0.9.42 (MIT OR Apache-2.0, MSRV 1.71, a maintained fork of the `serde_yaml`
line) and `saphyr` 0.3.3 (MIT OR Apache-2.0, MSRV 1.85). Whichever is chosen
must preserve key order and emit the same scalar style as the original, which
the round-trip test enforces; a library that reorders mappings is disqualified
regardless of its API.

Codegen takes a request description, not a `Doc`:

```rust
pub fn generate(request: &RequestSpec, target: Target) -> Result<String, CodegenError>;
```

so the GUI's snippet modal and the CLI share one path (`IO-016`, `IO-017`).

Git uses `gix` (ADR 0007), with path containment so a symlink cannot escape
the repository (`IO-022`, `IO-023`). Two collection layouts coexist and
`CollectionFs` handles both: the legacy `.insomnia/<Type>/<id>.yml` tree and
the current root-level `insomnia.<id>.yaml` sitting next to `.git/`
(`docs/inventory/storage.md`). The layout is detected per collection, not
configured. `STO-011` migrates a repository from the first to the second in M6
with an idempotent version stamp, and a converted repository stays usable by the
original until the user commits, because the migration is a Git operation and
the old tree survives in history.

OpenAPI parsing and linting are separate crates because design mode has its
own cadence (500 ms debounced autosave plus lint) and its own failure modes
(SSRF guard on remote references, ruleset bundling). The lint engine itself is
M6; the parse contract is available earlier because the OpenAPI importer needs
it in M5.

## 16. The GUI

### 16.1 Threads and channels

One egui thread. One tokio multi-thread runtime, created before
`eframe::run_native` and owned by `heric-core`. All I/O — SQLite, libcurl
transfers, scripts, git — runs on that runtime. The UI thread never awaits
anything.

```rust
impl Core {
    pub fn spawn(config: CoreConfig) -> Result<(CoreHandle, EventStream), CoreError>;
}

pub struct CoreHandle { /* Clone + Send */ }

impl CoreHandle {
    pub fn send(&self, command: Command) -> Result<(), CoreError>;
}

pub enum Command { /* ... */ }
pub enum Event { /* ... */ }
```

`CoreHandle::send` is non-blocking. `EventStream` is a channel consumer the
GUI drains once per frame. Events carry no borrowed data: everything is owned,
because the consumer outlives the producer.

Repaint: `CoreConfig` takes a `repaint: Arc<dyn Fn() + Send + Sync>`. The GUI
passes a closure that clones the `egui::Context` and calls
`request_repaint()`. The core calls it *after* pushing an event into the
queue, never before — the other order can lose frames when the UI is idle.
The GUI also requests a repaint while any operation is in flight, so the
progress indicator animates even when no event arrives.

Backpressure: unbounded. A bounded channel would make the producer await, and
the tokio task blocking on a full queue while the UI is asleep deadlocks the
repaint it is waiting for. The event volume is bounded in practice: timeline
chunks are coalesced per 100 ms before being emitted.

**Synchronous ports, one blocking pool.** `Store`, `BlobStore`, `HttpEngine`,
`StreamingEngine`, `TagHost`, `ScriptHost` and `Connection` are synchronous
traits. That is a decision, not shorthand for async: libcurl is a blocking C
API, SQLite is a blocking C API, and an rquickjs `Runtime` must not cross an
`.await` point. `heric-core` calls every one of them from
`tokio::task::spawn_blocking`, so a blocked port holds a blocking-pool thread
and never a runtime worker or the UI thread. Cancellation crosses the thread
boundary through libcurl's progress callback reading an `Arc<AtomicBool>`, not
through dropping a future.

`TagHost::prompt` is the one port that waits on a person, and it does so on the
send path, where the renderer already occupies a blocking thread. The host
posts a prompt event, blocks that thread on a oneshot channel, and returns
`Timeout` when the GUI never answers, so a headless CLI run or a closed window
cannot hang a send. `Store::changes()` returning a
`tokio::sync::broadcast::Receiver` is the one place tokio appears in a trait
signature; it is deliberate, because the consumer is a GUI frame loop and a
second channel type would earn nothing.

Commands and events are the real Phase 1 contract and the largest part of the
work in `heric-core`:

- `Command`: open workspace, list children, create/update/delete doc, send
  request, cancel request, switch environment, import, export, git
  clone/commit/push/pull, run script, open/close connection.
- `Event`: doc changed, execution started/step/finished/failed/aborted,
  timeline appended, stream chunk, protocol connect/message/disconnect, import
  progress and report, git status, error toast.

Every `Command` that can fail reports through an `Event`, never through a
`Result` the GUI has to poll — because the operation has already left the UI
thread by the time it can fail.

### 16.2 Layout

The panels are `egui`'s own: `Panel::top` for the toolbar, `Panel::left` for
the sidebar, `Panel::right` for the response pane and `CentralPanel` for the
request editor, with a tab strip per region drawn by Heric.
`egui_extras::TableBuilder` renders the key-value grids, header lists and
cookie tables. Layout state is persisted per workspace. No docking crate is
used; `egui_tiles` 0.18 is the approved replacement if draggable splits ever
become a requirement (ADR 0013).

```text
+-- toolbar ----------------------------------------------+
| workspace picker . environment picker . palette         |
+-- sidebar ---+-- request ----------+-- response --------+
| collections | method + URL         | status . time . sz |
| tree        | tabs: Params         | tabs: Preview      |
|             | Headers Body Auth    | Headers Cookies    |
|             | Scripts Docs         | Console Timeline   |
+-------------+----------------------+--------------------+
```

### 16.3 Large responses

Responses are stored as bytes in the store; the viewer never holds the whole
body as a `String` unless the user asked for text preview. Rendering is
virtualised: the table shows the byte range or line range for the visible
rows only. Above 5 MB the preview is truncated with an explicit banner and a
"save to file" action, matching the original's thresholds (`UI-003`).

A JSON response is parsed lazily into an index of line offsets; only the
visible window is syntax highlighted. This is what keeps a 100 MB response
from freezing the frame.

### 16.4 Shortcuts

One registry maps an action id to a key combination, with platform defaults
resolved at startup (`Cmd` on macOS, `Ctrl` elsewhere). The registry rejects
duplicates at load time and the settings screen shows conflicts (`UI-007`,
`UI-009`).

The shipped defaults are copied from the original's registry table in
`docs/inventory/ui.md`. Two of them have to be decided rather than copied: the
original ships `sidebar_focusFilter` and `beautifyRequestBody` on the same
combination, and `workspace_showSettings` is displayed in settings with no
handler behind it (`UI-009`, `UI-010`). Heric gives each real action a
distinct combination and drops the dead binding. Bindings are rebindable and
persisted per machine, not per collection.

### 16.5 UI translation

**Decision (owner):** English, pt-BR and Spanish from M1.

Strings live in the crates that own them as resource files with English as the
source. A missing key falls back to English and is reported in debug builds.
The three languages are part of the M1 definition of done, not a polish item.

The library is a builder call with one constraint: whatever is chosen must
compile the catalogs into the binary and must not generate translation code at
runtime. Two candidates were checked on 2026-11-10. `fluent` 0.17.0
(Apache-2.0 OR MIT, MSRV 1.67) has the better plural and gender handling but
its last release was May 2025. `rust-i18n` 4.2.4 (MIT) is actively released
and simpler, but treats messages as flat strings. Prefer `fluent` for the
message quality and fall back to `rust-i18n` if the maintenance gap becomes a
problem; either satisfies the constraint above.

The translation resources are separate from the documentation books in
`docs/book/`, which keep the ADR 0002 structure.

### 16.6 Panels and focus

Keyboard focus follows the original's model: the URL bar captures `Cmd+Enter`
globally to send, panes keep their own history, and closing the last tab
returns to the collection view rather than to an empty screen (`UI-005`).

### 16.7 Where UI state lives

The original splits UI state three ways: the database (`Settings`,
`WorkspaceMeta.activeRequestId`, `RequestMeta.previewMode`), the browser's
local storage (tab list, sidebar expansion, recent filters, panel sizes,
sidebar focus) and process memory. Heric keeps the split but moves the middle
group into one `ui-state.json` in the Heric data directory, written with the
same debounce as the store and never synced. Those values are per-machine
conveniences, and putting them in a synced collection would produce diffs for
every collaborator on every window resize.

The `RequestMeta` fields the original persists stay in the store, because its
filtering and sorting read them from there: `previewMode`, `responseFilter`
and its 10-entry history, `activeResponseId`, `pinned` and `lastActive`. Note
that `lastActive` is rewritten on every request open; it is a sort key, never
an event log, and the debounce is what keeps it from costing a write per
click.

## 17. Secrets

**Decision:** the `keyring` crate (Keychain, Credential Manager, Secret
Service). Vault secrets, OAuth tokens and Git credentials go to the OS
keychain, keyed by collection and entity id. Nothing secret is written to
SQLite or to the timeline in clear.

When the keychain is unavailable (headless Linux with no Secret Service), the
core reports a degraded mode at startup and the CLI refuses operations that
need a secret rather than falling back to plaintext files.

A secret imported from Insomnia in its encrypted form (section 9) has a fourth
state beside present, absent and degraded: **pending re-entry**. It is a
property of the `Environment` document, not of the keychain, so it survives
without the keychain being consulted, and it clears the moment the user saves a
new value.

## 18. Errors

- Libraries: one `thiserror` enum per crate, each variant carrying the context
  its caller needs. No catch-all `Any` variant.
- Binaries: `anyhow` for context chaining at the top level.
- The GUI never shows a raw `Debug` string. Every user-visible error has a
  message from the error enum plus an action hint.
- Localisation: a `thiserror` enum's `Display` is fixed English and cannot be
  translated at display time, so every error enum also exposes a stable message
  key and its arguments, which the GUI's Fluent layer resolves. The trait lives
  once, in `heric-model`, and all twelve crates implement it:

```rust
pub trait Diagnostic {
    fn message_key(&self) -> MessageKey;
    fn message_args(&self) -> Vec<(String, MessageValue)>;
    fn hint_key(&self) -> Option<MessageKey> { None }
}
```

  `Display` stays English for logs and for the CLI. A key missing from a catalog
  fails a test in debug builds and falls back to English in release.
- `unwrap`/`expect` are denied by lint. `?` everywhere; where a value is
  structurally guaranteed, the code returns an error instead of asserting.

## 19. Logging and diagnostics

`tracing` throughout, `tracing-subscriber` in the binaries, `RUST_LOG`
honoured. The GUI writes a rotating log under the data directory and exposes
"open log folder" in settings. Request-level diagnostics go to the timeline,
not the log, because the user reads the timeline.

## 20. Testing

- **Unit tests** per crate, next to the code.
- **Golden tests** from the original repository's own fixtures, copied into
  `tests/fixtures/` and never edited. The source material is already in place
  in the read-only checkout: `packages/insomnia/src/common/__fixtures__/`
  holds `curl`, `har`, `insomnia-v5-roundtrip` (14 files), `openapi`,
  `postman` and `prettify`; the render behaviour lives in the test tables of
  `common/__tests__/` (24 files), `common/templating/__tests__/` and
  `templating/__tests__/liquid-compat.test.ts`. Each crate ports the table
  that covers it, and a golden test fails when the Rust output differs from
  the recorded original output.
- **Wire tests** with `wiremock` for the HTTP pipeline: redirects, cookies,
  auth injection, timeline ordering.
- **Property tests** where the original's behaviour is a rule rather than a
  table: id creation, patch merging, path containment in Git.
- **A migration test** against a synthetic NeDB directory with one document of
  each of the 44 types plus a response with a body and a timeline, asserting the
  migrated database, the migrated blobs and a re-export.
- **GUI tests** with `egui_kittest` for the panes that carry logic (tab
  lifecycle, conflict reporting, the send button's state machine).
- **End-to-end** harness is P1 (`STR-002`), not M1.

No test reaches the network. Every HTTP test is against `wiremock` or a local
server.

## 21. Build, CI and packaging consequences

- `curl-sys` compiles libcurl with the `cc` crate, so it needs a C toolchain
  and nothing more: no cmake, no make, no autotools, no vcpkg. Every runner
  already has one. The one thing that would drag in nasm is the `rustls` or
  `static-ssl` feature, through `aws-lc-sys` or `openssl-src`; section 10
  argues against both. `protobuf-compiler` is already installed for `tonic`.
  The exact requirements per target live in ADR 0010, which is written after a
  spike that builds the workspace on all four targets.
- The `test` job runs four entries: Linux, Windows, macOS Apple Silicon and
  macOS Intel, each on a runner of that architecture and each running
  `cargo test`. No `build_only` entry remains. Intel macOS runners are the
  scarcest resource in the matrix and their image labels have churned as GitHub
  retires Intel hardware, so the image name is pinned in one place in the
  workflow and revisited when the image is deprecated.
- `cargo-deny` does not inspect the vendored libcurl tree; it reads crate
  metadata, which says MIT for both `curl` and `curl-sys`. The licence
  obligation therefore lands on the release artefacts, not on the `deny` job:
  a `THIRD-PARTY-NOTICES` file in the archives, generated in the release
  workflow. `cargo-deny` still checks the four release targets and stays as it
  is.
- `rquickjs` needs a C compiler, already present on every runner.
- MSRV 1.95 stays; if a dependency raises its floor, the ADR that introduces
  it records the new floor and the CI `msrv` job enforces it.
- Packaging (ADR 0001) is unchanged; a statically linked libcurl lives inside
  the binary, so nothing extra is installed at runtime. The one addition is the
  notice file above.

## 22. What is left to the builder

Deliberately not decided here, to be settled during implementation:

- The exact SQLite schema: one table per type versus a shared `documents`
  table with a `kind` discriminator. The migration test decides it; either is
  acceptable as long as `Store` is the only access path.
- The internal parser design for `heric-template` (hand-written recursive
  descent versus a parser-combinator crate) — only the public API above is
  fixed.
- What sits behind `BlobStore`: a file per response body and timeline, a
  BLOB column in SQLite, or a mix. The contract is the port; the medium is not,
  and `stream` is what keeps the choice from leaking into the viewer.
- The precise debounce and flush policy for autosave beyond the 1000 ms
  starting point.
- The visual design: colours, spacing, iconography. ADR-free; the owner
  reviews screenshots.
- The exact names of every `Command` and `Event` variant. The rule is the one
  in section 16.1; the variants grow with the milestones.

## 23. ADR map

| ADR | Decision | Status after this design |
|---|---|---|
| 0001 | Packaging with `cargo-packager` | accepted, unchanged |
| 0002 | Documentation i18n with three trees | accepted, unchanged |
| 0003 | Template engine rewritten in Rust | accepted |
| 0004 | HTTP client: the `curl` crate | rewritten, accepted |
| 0005 | Scripting sandbox: `rquickjs` | accepted |
| 0006 | Storage: SQLite plus NeDB migration | accepted |
| 0007 | Git: `gix` | accepted |
| 0008 | Code editor: `egui_extras` + `syntect` for responses | accepted |
| 0009 | GUI concurrency: one UI thread, tokio runtime, owned channels | new |
| 0010 | libcurl build and TLS backend | new |
| 0011 | Credentials in the OS keychain | new |
| 0012 | UI translation from M1 | new |
| 0013 | GUI layout: `egui` panels, `egui_tiles` approved for later docks | new |

## 24. Decision log

Decided by the owner:

1. Insomnia data is read in place and migrated into a Heric-owned directory;
   Heric never writes into the Insomnia directories.
2. The UI ships in English, pt-BR and Spanish from M1.
3. The HTTP engine is the `curl` crate, on the owner's acceptance of the
   recommendation in section 10.

Decided here, on the owner's behalf, and open to challenge:

1. Storage is SQLite with a NeDB migration layer (ADR 0006).
2. Templates are rendered by a Rust reimplementation, not an embedded JS
   engine (ADR 0003).
3. Scripts run on `rquickjs` (ADR 0005).
4. Git uses `gix` (ADR 0007).
5. Ports and adapters: leaf crates declare the traits they need and
   `heric-core` injects the implementations (section 7.1).
6. Documents keep unknown fields as JSON and expose typed views, instead of a
   44-variant enum (section 8.1).
7. Unbounded event channel plus a repaint callback called after enqueue.
8. Secrets live in the OS keychain with an explicit degraded mode, never in
   plaintext.
9. Backup copies the documents and small state files only; response blobs and
   Git repositories are read in place and never copied (section 9).
10. Response bodies and timelines migrate through a `BlobStore` port, and the
    medium behind it stays a builder choice (section 9, section 22).
11. All ports are synchronous and invoked from `spawn_blocking`; `TagHost::prompt`
    times out instead of hanging a send (section 16.1).
12. Library errors carry a message key and arguments beside their English
    `Display` (section 18).
13. Git supports both collection layouts and detects which one a repository
    uses; `STO-011` converts legacy repositories in M6 (section 15).
14. All four targets, including macOS Intel, run the full test suite on every
    pull request on dedicated architecture runners (section 3, section 21).
15. Secrets the original kept as Electron `safeStorage` ciphertext are copied
    verbatim and marked pending re-entry, never decrypted (section 9,
    section 17).
16. The GUI uses `egui`'s own panels and no docking crate, because
    `egui_dock` 0.21.1 pins `egui` 0.36 against a 0.37 workspace and docking
    has no parity row; `egui_tiles` 0.18 is the approved dock if draggable
    splits become a requirement (ADR 0013).
