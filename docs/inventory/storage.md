# Persistence

Scope: how Insomnia stores data, how writes reach disk, the files produced on disk, and every migration that changes the shape of that data. Paths are relative to the Insomnia repo root (`D:/Projects/Heric/Insomnia`). Priority: **P0** = blocks a correct Rust reimplementation; **P1** = required for real user data; **P2** = defensive or optional.

Baseline fact: **there is no database schema version field.** Searches for `dbVersion`, `__version`, `insomnia.db`, and `compactDatafile` return no application-code hits. The only version stamps are `schema_version` inside YAML exports, `repoMigrationVersion` on `GitRepository` documents, one electron-storage flag for git credentials, and `Stats.currentVersion`/`lastVersion`. The NeDB store is a schema-free document bag whose shape is enforced at read time by `initModel`.

## Storage engine

**Engine.** NeDB, `"@seald-io/nedb": "^4.1.1"` (`packages/insomnia-data/package.json:38`), resolved to 4.1.2 (`package-lock.json:6977-6990`). One datafile per collection; no indexes other than `_id`. **P0**

**Factory and closure state.** `createNedbDatabase(wrapper?)` at `packages/insomnia-data/node-src/database/database-nedb.ts:47`. Mutable closure state: `nedbBucket` (`:50`), `bufferingChanges = false` (`:55`), `bufferChangesId = 1` (`:56`), `changeBuffer = []` (`:58`), `changeListeners = []` (`:60`). `bufferChangesId` starts at 1, so flush id `0` is the "flush everything" sentinel. **P0**

**Datafile options.** Every datafile is created with `{ autoload: true, corruptAlertThreshold: 0.9, ...config }` (`database-nedb.ts:247-251`). Edge cases: more than 10% unparsable lines makes the datafile fail to load; up to 10% corrupt lines are silently discarded. `autoload: true` starts loading at construction and is never explicitly awaited before `repairDatabase()` runs (`:438-440`). **P0**

**Path resolution.** `dbPath = process.env.INSOMNIA_DATA_PATH || getTempPath('userData')` (`database-nedb.ts:253-255`); `getTempPath` yields `os.tmpdir()/insomnia-send-request` (`:34-36`). The temp path is a fallback for hosts that supply no path (templating worker); the Electron main process always supplies one. **P0**

**No custom serialization.** `DataStoreOptions` declares `timestampData`, `beforeDeserialization`, `afterSerialization`, `modes` (`packages/insomnia-data/src/database/types.ts:7-18`), but `init` never passes them, so documents are stored exactly as JS objects. The physical line format is NeDB's own (the library is not vendored in this repo), so it must be validated against a real datafile rather than inferred from app code. **P1**

**IoC layer.** `initDatabase(impl)` (`packages/insomnia-data/src/database/index.ts:17-20`) assigns a module-global; the exported `database` Proxy throws when uninitialized (`:30-34`). `packages/insomnia/src/common/database.ts:1` is a one-line re-export. Implementations:

- Main process: `packages/insomnia/src/main/database.main.ts`; `dbPath` from env or `app.getPath('userData')` (`:8`); registers the `database.invoke` IPC handler inside `init` (`:18-24`); `flushChanges` broadcasts `db.changes` to all windows (`:26-36`).
- Renderer: `packages/insomnia/src/ui/database.client.ts`; bridge over `database.invoke`; `init` and `onChange` are no-ops (`:60-70`) — the renderer never owns a store.
- Plugin window: `packages/insomnia/src/main/database.plugin-window.ts`; IPC proxy whose `flushChanges` returns `{ changes: [], deletedIds: [] }` (`:16-19`).
- Tests: `packages/insomnia-data/setup-vitest.ts` initializes an in-memory store.

**Collections.** 44 datafiles named `insomnia.<Type>.db`, declared inline at `database-nedb.ts:257-434` (first `insomnia.ApiSpec.db` at `:260`, last `insomnia.WorkspaceMeta.db` at `:432`). Full set: ApiSpec, CaCertificate, ClientCertificate, CloudCredential, CookieJar, Environment, GitCredentials, GitRepository, GrpcRequest, GrpcRequestMeta, MockRoute, MockServer, McpPayload, McpRequest, McpResponse, OAuth2Token, PluginData, Project, ProjectLintRuleset, ProtoDirectory, ProtoFile, Request, RequestGroup, RequestGroupMeta, RequestMeta, RequestVersion, Response, RunnerTestResult, Settings, SocketIOPayload, SocketIORequest, SocketIORequestMeta, SocketIOResponse, Stats, UnitTest, UnitTestResult, UnitTestSuite, UserSession, WebSocketPayload, WebSocketRequest, WebSocketRequestMeta, WebSocketResponse, Workspace, WorkspaceMeta. Unknown collection names log a warning and are skipped on read (`database-nedb.ts:217-234`), so a Rust reader must not treat the list as authoritative. **P0**

**Query surface.** `Query`/`SpecificQuery` support `$exists` and `$ne` (`packages/insomnia-data/src/database/types.ts:25-36`); the Konnect detector relies on the pair (`packages/insomnia/src/konnect/migrate-konnect-organization.ts:44-47`). Semantics to match: `find` defaults to sort `{created: 1}` (`database-nedb.ts:217-234`), `findOne` defaults to sort `{created: 1}` (`:205-215`), `update` is an upsert keyed on `_id` (`:513-518`).

## On-disk layout per platform

**Data directory.** `packages/insomnia/src/entry.main.ts:54-58`: `INSOMNIA_DATA_PATH` overrides when set; otherwise `app.setPath('userData', path.join(app.getPath('userData'), '../', dev ? 'insomnia-app' : 'Insomnia'))`. Electron's `userData` is `<platformAppData>/<appName>`, so joining `..` plus the folder name lands directly in the platform app-data root:

- Windows: `%APPDATA%/Insomnia` (dev: `%APPDATA%/insomnia-app`). **P0**
- macOS: `~/Library/Application Support/Insomnia` (dev: `.../insomnia-app`). **P0**
- Linux: `~/.config/Insomnia` (dev: `~/.config/insomnia-app`). **P0**

`packages/insomnia/config/config.json` sets `userDataFolder: "Insomnia"` and `appId com.insomnia.app`; `packages/insomnia/electron-builder.config.js:11` repeats the appId for packaged builds, so packaged and dev data directories differ by one folder name. **P1**

**Layout inside the data directory.**

- `<dataPath>/insomnia.<Type>.db` — the 44 NeDB datafiles. **P0**
- `<dataPath>/localStorage/<key>` — one small file per key (see electron-storage below). **P1**
- `<dataPath>/responses/<id>.response` — raw HTTP bodies, written by `packages/insomnia/src/main/network/libcurl-promise.ts:111`. **P1**
- `<dataPath>/responses/<id>.timeline` — timeline JSON, via `packages/insomnia/src/main/ipc/main.ts:157-220`. **P2**
- `<dataPath>/backups/<version>/insomnia.*.db` — pre-upgrade snapshots (`packages/insomnia/src/main/backup.ts:39-61`). **P1**
- `<dataPath>/version-control/git/<repoSlug>/` — managed git repos; working tree at repo root, git metadata in `.git/` (`packages/insomnia/src/main/git-service.ts:400-402`, `:438-471`). **P1**
- Feature folders created lazily via `readOrCreateDataDir` (`packages/insomnia/src/main/ipc/main.ts:652-660`, which `mkdir -p`s under `app.getPath('userData')`); used for plugins and LLM configs. **P2**

**Documents vs side files.** `Response` documents carry `bodyPath`/`timelinePath` pointers, with `bodyBuffer` as fallback; the bytes live under `responses/`. Retention prunes old response documents with a `$nin` query when a request exceeds `maxResponses` (default 20; `packages/insomnia-data/node-src/services/response.ts:45-79`). Deleting rows does not delete body files, so orphans are expected; `responses/` must be treated as an ID-keyed blob store. **P1**

**Git working tree.** Root holds `insomnia.<id>.yaml` (one per workspace), `.spectral.yaml` (project lint ruleset), and arbitrary user files; `.git/` holds git data. The workspace file name comes from `WorkspaceMeta.gitFilePath` when set, else `insomnia.<workspaceId>.yaml` (`packages/insomnia/src/sync/git/repo-file-watcher.ts:277-281`). Import-generated names are `<sanitizedName>-<workspaceId>.yaml` (`packages/insomnia/src/common/import.ts:695-699`), with path separators, `..`, and NUL stripped by `sanitizeGitFileNameSegment` (`import.ts:710-719`). **P1**

**Legacy git content.** Older builds kept a `.insomnia/` directory of per-type YAML (`packages/insomnia/src/sync/git/git-vcs.ts:106`, `GIT_INSOMNIA_DIR_NAME = '.insomnia'`); the overlay is `packages/insomnia/src/sync/git/ne-db-client.ts:190-228`, enumerating 15 syncable types as `<Type>/<id>.yml`. A reader opening an old repo must handle both layouts; conversion is `importLegacyInsomniaFolder` (`git-service.ts:1188-1311`), with layout notes at `git-service.ts:1165-1186`. **P0**

**Read guards.** `packages/insomnia/src/main/secure-read-file.ts:50` refuses paths matching `/^insomnia\..+\.db.*$/i` (templates and plugins cannot read the datafiles); only `responses` and `version-control` subdirectories are allowed (`:36`); hard-link aliases are rejected by comparing device plus inode (`:61-76`). **P1**

**Write guards.** `projectRoutableFSClient` routes `.git/` to the git-data client and everything else to the disk working tree, with a repo-relative traversal guard (`packages/insomnia/src/sync/git/project-routable-fs-client.ts:31-53`; wiring `git-service.ts:685-708`). `isPathOutsideRepo` rejects only true `..` segments, not names that merely begin with `..` (`repo-file-watcher.ts:86-97`). All watcher writes go through `writeFileWithinDir` (`repo-file-watcher.ts:53`, `:314`). **P0**

## Write path and buffering

**Single write owner.** All persistence is owned by the main process. The renderer performs every read and write through `database.invoke` IPC (`packages/insomnia/src/ui/database.client.ts:60-70`); the plugin window is an IPC proxy (`packages/insomnia/src/main/database.plugin-window.ts:16-19`). A Rust implementation that embeds storage in-process can skip the IPC layer but must reproduce ordering. **P0**

**Change buffering.** `bufferChanges(millis = 1000)` (`database-nedb.ts:103-107`) starts a `setTimeout(database.flushChanges, millis)`. The callback is unbound and receives no arguments, so `id` defaults to `0`, which is the "flush everything" sentinel, not the buffer's own id. `bufferChangesIndefinitely` (`:110-113`) starts no timer. **P1**

**Flush.** `flushChangesImpl(id = 0, fake = false)` (`database-nedb.ts:63-88`): returns early when `id !== 0 && bufferChangesId !== id`; sets `bufferingChanges = false`; copies and clears `changeBuffer`; when `fake` is true logs and drops the events without notifying; otherwise awaits every change listener sequentially and returns the collected changes. In the main process the listener broadcasts them to all windows as `db.changes` (`packages/insomnia/src/main/database.main.ts:26-36`). **P1**

**Immediate notifications.** `notifyOfChange` (`database-nedb.ts:612-622`) flushes immediately when `bufferingChanges` is false; otherwise events accumulate. Individual operations also call `onChange` listeners directly for inserts and removals (`:450-452`, `:455-505`). **P1**

**Mutation primitives.**
- `insert` (`:443-448`).
- `update` (`:513-518`) calls `updateAsync({ _id }, doc, { upsert: true })`: whole-document replacement, so omitted fields disappear.
- `docUpdate` (`:132-145`) injects `{ modified: Date.now() }` into the patch before applying it, restamping the document even when only one field changed.
- `docCreate` (`:120-130`).
- `duplicate` (`:148-204`) buffers changes indefinitely, recurses into descendants, rewrites references, and inserts plus notifies per document.
- `remove` (`:455-478`) and `removeWhere` (`:480-505`) call NeDB `remove` **without awaiting it**, then notify per document; `unsafeRemove` (`:508-511`) exists for bypassing listeners.
- `getWithDescendants` (`:559-607`) and `withAncestors` (`:521-551`) walk the parent tree in memory.

**Timestamps.** Defaults are `{ ... modified: Date.now(), created: Date.now() }` plus model `init()` (`packages/insomnia-data/node-src/database/init-model/index.ts:23-34`). `modified` is load-bearing for git sync: it survives migration and drives the "DB newer than disk" check. **P1**

**Startup init.** `init` (`database-nedb.ts:242-441`) clears listeners and the bucket on `forceReset`, constructs all datafiles (`:257-434`), and then runs `await repairDatabase()` unless `inMemoryOnly` (`:438-440`). The source carries a NOTE/TODO that this can hang tests; it is not gated on the autoload promise. **P1**

**No compaction.** App code never calls NeDB compaction APIs and never supplies persistence hooks; datafiles grow append-only between NeDB-internal rewrites. Any Rust writer that appends lines must be validated against real datafiles rather than assumed. **P1**

**Electron key-value store.** `packages/insomnia/src/main/electron-storage.ts` stores one file per key under `<dataPath>/localStorage`, validates keys (`:87-93`), debounces writes per key by 100 ms (`:39-44`), and writes synchronously (`:95-115`). It backs window bounds/zoom, the git-credentials migration flag, and secrets. `packages/insomnia/src/main/ipc/secret-storage.ts:54-72` encrypts via Electron `safeStorage` to hex with a plaintext fallback when encryption is unavailable. **P1**

**Backup and restore.** `packages/insomnia/src/main/backup.ts:13-37` compares the running version with `<dataPath>/backups`; `backup()` (`:39-61`) copies only `*.db` files into `backups/<version>`. `restoreBackup(version)` (`:63-90`) validates the path with `assertPathWithinDir`, copies files via an un-awaited `files.forEach(async ...)`, then relaunches. Edge cases: `responses/`, `localStorage/`, and `version-control/` are not backed up or restored; the relaunch can occur before the copy finishes. **P1**

**Git sync write path.** `RepoFileWatcher` (header comment `packages/insomnia/src/sync/git/repo-file-watcher.ts:1-38`) runs one serial queue with two pipelines: FS→DB (`fs.watch` plus 10 s polling, `:56-57`) and DB→FS (`db.onChange`, 300 ms debounce). Loop prevention uses SHA-256 of written content in `lastWrittenHash` plus an mtime fast path (`:26-38`). On startup, `flushNewerDbWorkspacesToDisk` (`:272-330`) compares the max `modified` of each workspace tree against the YAML file mtime and rewrites disk when the DB is newer, recording the hash so the subsequent import scan skips it. Edge cases: the comparison is clock-based, and file mtime is recorded as `Date.now() + 1` (`:319`). **P1**

**Inbound import.** `upsertDocs` (`repo-file-watcher.ts:920-949`) buffers changes, forces the workspace `parentId` to the project id (`:933`), updates `WorkspaceMeta.gitFilePath` and `gitFileLastSyncTime` (`:934-938`), then updates each document and flushes. `deleteOrphans` (`:901-917`) removes documents present in the DB but absent from the YAML, restricted to `models.canSync`. Deleting a YAML file removes the mapped workspace and all descendants (`:960-1001`). **P1**

**Identifier invariants.** Base environments get a deterministic id `${prefix}_${sha1(parentId)}` (`packages/insomnia-data/node-src/services/environment.ts:76-105`) so sync never duplicates them. Special ids exist for scratchpads and organizations: `wrk_scratchpad` (`models/workspace.ts`), `proj_scratchpad` and `EMPTY_GIT_PROJECT_ID = 'empty'` (`models/project.ts`), `org_scratchpad` and `org_konnect_<accountId>` (`models/organization.ts`). `gr_`-prefixed git repository ids are encoded/decoded by helpers in `models/project.ts`. **P1**

## Migrations

Ordered by when they take effect: per-read document migrations first, then startup repairs, then flag/layout migrations, then format-level import conversions.

| Version | What it does | Source location |
|---|---|---|
| none (every read/write) | `initModel` builds defaults, generates `_id`, dispatches per-type `migrate`, then prunes any key not in defaults or `optionalKeys` | `packages/insomnia-data/node-src/database/init-model/index.ts:23-34`, `:37-39`, `:44-68`, `:72-78` |
| none | Workspace: extract legacy `certificates[]` into ClientCertificate docs and delete the field | `packages/insomnia-data/node-src/database/init-model/workspace.ts:21-45` |
| none | Workspace: force a string `name` ("My Workspace" when invalid) | `workspace.ts:52-58` |
| none | Workspace: scope `designer`/`spec` to `design`, unset/other to `collection` | `workspace.ts:67-83` |
| none | Request: string body to object body, including `x-www-form-urlencoded` param conversion | `packages/insomnia-data/node-src/database/init-model/request.ts:28-59` |
| none | Request: non-string `url` becomes `''` | `request.ts:65-73` |
| none | Request: legacy auth with `username` gets `authentication.type = 'basic'` | `request.ts:79-87` |
| none | Response: `bodyCompression === '__NEEDS_MIGRATION__'` becomes `'zip'` | `packages/insomnia-data/node-src/database/init-model/response.ts:12-18` |
| none | Settings: fold `templateTagSandboxEnabled` into `pluginSandboxEnabled`, then delete the old flag | `packages/insomnia-data/node-src/database/init-model/settings.ts:22-29` |
| none | Settings: reconcile `hotKeyRegistry` with the default registry (drop removed, add new) | `settings.ts:34-51` |
| none | Settings: reset `request_createHTTP` when it still uses Cmd/Ctrl+N | `settings.ts:58-100` |
| none | CookieJar: assign missing cookie ids; grandfather missing `source` as `'manual'` | `packages/insomnia-data/node-src/database/init-model/cookie-jar.ts:5-29` |
| none (startup) | `repairDatabase`: project sync flags, ApiSpec-name fix (skips no-op writes to avoid restamping `modified`), base-environment repair, multiple cookie jars, old git URIs | `packages/insomnia-data/node-src/database/repair-database.ts:13-17`, `:35-45`, `:52-87`, `:94-126`, `:129-141` |
| none (startup) | Git credentials unification (GitHub/GitLab/custom into one model), idempotent, gated by an electron-storage migration flag | `packages/insomnia/src/main/git/migrations.ts:35-44`, `:46-204`, `:210-228` |
| none (startup) | Pre-upgrade backup at `backups/<version>` when the app version is newer than the newest backup | `packages/insomnia/src/main/backup.ts:13-61`; sequence in `packages/insomnia/src/entry.main.ts:143-168` |
| repoMigrationVersion = 1 | Git repo layout: `.insomnia/<Type>/<id>.yml` plus `git/` to root-level YAML plus `.git/`; stamp stored on `GitRepository`, with an on-disk override for already-migrated repos | `packages/insomnia/src/sync/git/git-migration-version.ts:5`; `packages/insomnia/src/sync/git/git-repo-migration.ts:56-66`, `:70`, `:200-285`, `:292-390`; gate `packages/insomnia/src/ui/utils/router.ts:105`; batch runner `packages/insomnia/src/main/git-service.ts:4003-4092` |
| none (post-migration) | Backfill managed git folder slugs for existing repos | `packages/insomnia/src/main/git-service.ts:518`, `:3968-3995` |
| none | Import legacy `.insomnia` folder layout into the new repo structure | `packages/insomnia/src/main/git-service.ts:1188-1311`; detection `:1313-1325`; layout notes `:1165-1186` |
| 5.0 → 5.1 | YAML schema upgrade applied before Zod parse: `cleanHeadersAndParameters` and script normalization; property order normalized | `packages/insomnia/src/common/insomnia-schema-migrations/schema-version.ts:19`; `.../index.ts:16-26`, `:32-40`, `:43-49`, `:60-96`, `:102-120`, `:130-187`; `.../v5.1.ts:30-53`, `:67-166`; applied at `packages/insomnia/src/common/insomnia-v5.ts:937-982` |
| __export_format 1-4 | Legacy export JSON importers convert to current documents; detected per format | `packages/insomnia/src/main/importers/importers/insomnia-1.ts:147-150`; `insomnia-2.ts:13-26`; `insomnia-3.ts:12-25`; `insomnia-4.ts:17-30`; dispatch loop `packages/insomnia/src/main/importers/convert.ts:21-69`; registry `packages/insomnia/src/main/importers/importers/index.ts:15-27` |
| none | VCS projects merged into an organization | `packages/insomnia/src/sync/vcs/migrate-projects-into-organization.ts:17-30`, `:32-72` |
| none | Konnect organization migration, buffered, query needs `$exists` plus `$ne` | `packages/insomnia/src/konnect/migrate-konnect-organization.ts:44-47`, `:125-131` |
| none | Stats tracking `currentVersion`/`lastVersion` (telemetry, not data shape) | `packages/insomnia/src/entry.main.ts:405-468` |

YAML exports stamp `schema_version: '5.1'` at `packages/insomnia/src/common/insomnia-v5.ts:1021`, `:1436`, `:1465`, `:1498`, `:1515`, `:1538`. The v5 file discriminator is a union of five file types (`packages/insomnia/src/common/import-v5-parser.ts:488-495`); per-file schemas default `schema_version` to `5.0` (`:437`, `:450`, `:463`, `:472`, `:481`), and `mcpClient` deliberately uses `mcpClient.insomnia/5.0` (`:478-486`). Diffing and change reports ignore or relegate `schema_version` (`common/significant-diff-detection.ts`, `common/insomnia-v5-change-report.ts`).

## Import/migration risks for a Rust reader

**Migrations are read-time and in-memory, not persisted.** `initModel` transforms every document returned by `find`/`findOne` (`database-nedb.ts:217-234`, `:205-215`) but only writes back if the caller saves. A Rust reader must reproduce all per-type migrations or it will observe different data than Insomnia for the same file. **P0**

**Silent field pruning.** Any key not in the model defaults and not listed in `optionalKeys` is deleted on read (`init-model/index.ts:72-78`). If Heric writes a document back with a stale model definition, newer Insomnia fields are lost. Conversely, Heric must accept `optionalKeys` fields without pruning them. **P0**

**`modified` is semantic.** `docUpdate` restamps `modified` (`database-nedb.ts:132-145`); the watcher compares max `modified` against file mtime to decide which side wins (`repo-file-watcher.ts:272-330`). A Rust writer that touches `modified` gratuitously causes spurious YAML rewrites and git diffs; one that fails to set it can lose edits on upgrade/downgrade. **P1**

**Whole-document update.** `update` upserts the full object (`database-nedb.ts:513-518`); partial updates only happen through `docUpdate`. A Rust implementation using SQL-style partial updates must not assume merge semantics. **P1**

**Deletes are fire-and-forget.** `remove`/`removeWhere` do not await NeDB removal (`database-nedb.ts:455-505`), so a crash or immediate read can observe the document. Deletion is also how YAML removal propagates: deleting a workspace YAML deletes the workspace and descendants (`repo-file-watcher.ts:960-1001`), and inbound import removes any syncable doc missing from the YAML (`:901-917`). **P1**

**Unbound buffer timer.** `bufferChanges` schedules `setTimeout(database.flushChanges, millis)` (`database-nedb.ts:103-107`), so the flush id is `0` and flushes every buffer. Nested batching does not isolate; only explicit `flushChanges(id)` does. **P2**

**Repair races autoload.** `repairDatabase` runs right after datafile construction without awaiting load completion (`database-nedb.ts:438-440`); on a slow or corrupt datafile, repairs can run against pending state. **P1**

**Corruption tolerance.** `corruptAlertThreshold: 0.9` (`database-nedb.ts:247-251`) means up to 10% bad lines vanish silently; a Rust reader should reproduce or tighten this and never assume every historical write survived. **P1**

**Backups are narrow and racy.** Only `*.db` files are backed up, and restore copies files without awaiting before relaunch (`packages/insomnia/src/main/backup.ts:39-90`). `responses/`, `localStorage/`, and git repos are not covered; a restore can leave the app running with a partial database. **P1**

**Two git layouts coexist.** Old repos use `.insomnia/<Type>/<id>.yml` (`git-vcs.ts:106`, `ne-db-client.ts:190-228`); new repos use root-level `insomnia.*.yaml` plus `.git/` (`git-repo-migration.ts:200-285`). `repoMigrationVersion` may be absent, `0`, or `1`; the migration also honors an on-disk marker (`git-repo-migration.ts:56-66`). **P0**

**Workspace file naming is metadata-driven.** `WorkspaceMeta.gitFilePath` overrides the default `insomnia.<id>.yaml` (`repo-file-watcher.ts:277-281`); imports produce `<name>-<id>.yaml` with a sanitizer (`import.ts:695-719`). A Rust reader must join this value safely against the repo root and treat it as authoritative for rename/delete detection. **P0**

**YAML schema is versioned and reordered.** Files declare `schema_version`; missing values default to `5.0` (`import-v5-parser.ts:437-481`); `5.1` normalization changes header/parameter and script shapes and sorts property order (`insomnia-schema-migrations/index.ts:130-187`). Writing without the same normalization produces noisy diffs and may fail Zod validation on re-import. **P1**

**Importers are versioned by `__export_format`.** Only formats 1-4 plus v5 YAML are recognized (`packages/insomnia/src/main/importers/convert.ts:29-68`); unknown formats must be rejected rather than guessed. **P1**

**ID derivations must match.** Base environments use `sha1(parentId)` (`services/environment.ts:76-105`), and special ids (`wrk_scratchpad`, `proj_scratchpad`, `org_scratchpad`, `org_konnect_*`, `gr_`-encoded repo ids) are referenced from settings and projects. Generating different ids creates duplicate environments during sync. **P1**

**Blob pointer integrity.** `Response.bodyPath`/`timelinePath` point into `responses/`, which is never pruned with its rows (`services/response.ts:45-79`). Expect orphan bodies and missing bodies; readers must degrade gracefully. **P2**

## Feature list

| Feature | Behavior summary | Priority | Source |
|---|---|---|---|
| NeDB document store | 44 file-per-type collections, autoload, 10% corruption tolerance | P0 | `database-nedb.ts:242-441` |
| Pluggable database backend | Main-process NeDB, renderer/plugin IPC bridges, in-memory tests | P0 | `src/database/index.ts:17-34`; `database.main.ts`; `database.client.ts:60-70` |
| CRUD primitives | insert, update (full-doc upsert), docCreate, docUpdate, duplicate, remove, unsafeRemove | P0 | `database-nedb.ts:120-204`, `:443-518` |
| Hierarchy traversal | ancestors/descendants walks used by sync and delete | P1 | `database-nedb.ts:521-607` |
| Change buffering and broadcast | buffer/indefinite/flush/fake flush, `db.changes` to all windows | P1 | `database-nedb.ts:63-113`, `:612-622`; `database.main.ts:26-36` |
| Read-time document migrations | per-type migrations plus key pruning on every read | P0 | `init-model/*.ts`; `init-model/index.ts:44-78` |
| Startup repairs | project flags, ApiSpec name, base envs, cookie jars, git URIs | P1 | `repair-database.ts:13-141` |
| YAML v5 export/import | `schema_version` 5.0/5.1, five file types, Zod validation, normalization | P0 | `common/insomnia-v5.ts`; `common/import-v5-parser.ts:420-495` |
| YAML schema migration | 5.0 to 5.1 header/parameter/script cleanup and ordering | P1 | `common/insomnia-schema-migrations/*` |
| Legacy export importers | `__export_format` 1-4 conversion | P1 | `main/importers/importers/insomnia-1..4.ts` |
| Git repo structure migration | v1 layout, per-repo stamp, disk override, batch runner, slug backfill | P0 | `sync/git/git-repo-migration.ts`; `main/git-service.ts:4003-4092` |
| Legacy `.insomnia` folder import | old per-type YAML layout converted into current repos | P0 | `main/git-service.ts:1188-1325` |
| Bidirectional repo sync | watcher, poll plus fs.watch, hash/mtime loop prevention, orphan deletion | P1 | `sync/git/repo-file-watcher.ts:1-100`, `:272-330`, `:901-1001` |
| Backup and restore | per-version DB snapshots, guarded restore, relaunch | P1 | `main/backup.ts:13-90` |
| Response blob storage | `responses/<id>.response` and `.timeline`, 20-response retention | P1 | `main/network/libcurl-promise.ts:111`; `main/ipc/main.ts:157-220`; `services/response.ts:45-79` |
| Electron key-value storage | one file per key under `localStorage/`, debounced sync writes | P1 | `main/electron-storage.ts:39-115` |
| Secret storage | `safeStorage` encryption to hex with plaintext fallback | P2 | `main/ipc/secret-storage.ts:54-72` |
| Git credential migration | one-time unification gated by a stored flag | P1 | `main/git/migrations.ts:35-228` |
| Organization migrations | VCS projects into orgs; Konnect org migration | P2 | `sync/vcs/migrate-projects-into-organization.ts`; `konnect/migrate-konnect-organization.ts` |
| Datafile read protection | block `insomnia.*.db` from templates/plugins, symlink/hard-link guards | P1 | `main/secure-read-file.ts:36-76` |
| Deterministic and special ids | `sha1(parentId)` base envs; scratchpad and `gr_` ids | P1 | `services/environment.ts:76-105`; `models/project.ts`; `models/workspace.ts` |
| Version telemetry | `Stats.currentVersion`/`lastVersion` written on startup | P2 | `entry.main.ts:405-468` |
