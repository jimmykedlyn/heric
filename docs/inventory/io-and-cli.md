# Import, export, codegen, Git and CLI

Source of truth: `D:\Projects\Heric\Insomnia\packages\`. Package versions read from
`packages/insomnia-inso/package.json:4` and `packages/insomnia-vcs/package.json:3`
(both `13.3.1`). Paths below are relative to the Insomnia repo root; `src` means
`packages/insomnia/src`.

The five subsystems share one interchange primitive: the **Insomnia v5 YAML file**.
Export writes it, Git sync persists one such file per workspace, `inso` reads it as a
data source, and the importer/`scanResources` path reads it back. Anything built in
Heric that cannot round-trip this format breaks project export, Git sync and `inso`
simultaneously.

## Export formats and versions

### Insomnia v5 YAML (the only collection export the UI produces)

| Item | Value | Cite |
|---|---|---|
| Schema version constant | `INSOMNIA_SCHEMA_VERSION = '5.1'` | `src/common/insomnia-schema-migrations/schema-version.ts:19` |
| Export entry point | `getInsomniaV5DataExport({workspaceId, includePrivateEnvironments, requestIds?, db?})` | `src/common/insomnia-v5.ts:1043-1053` |
| Import entry points | `importInsomniaV5Data(rawData)` / `tryImportV5Data(rawData)` | `src/common/insomnia-v5.ts:1006`, `:990` |
| Parse pipeline | `importData()` → `migrateToLatestYaml` → `InsomniaFileSchema` | `src/common/insomnia-v5.ts:937`, import `:44`; migration `src/common/insomnia-schema-migrations/index.ts:60` |
| Empty-value stripping | `removeEmptyFields()` before `yaml.stringify` | `src/common/insomnia-v5.ts:365`, used `:1030`, `:1461`, `:1494`, `:1509`, `:1532`, `:1551` |
| Validate before write | `InsomniaFileSchema.parse(...)` on every branch | e.g. `src/common/insomnia-v5.ts:1440`, `:1492`, `:1528` |

One file is emitted **per workspace**. The top-level `type` is chosen by workspace scope
(`src/common/insomnia-v5.ts:1423-1552`):

| Workspace scope | `type` string | Sections | Cite |
|---|---|---|---|
| `collection` | `collection.insomnia.rest/5.0`, **or** `spec.insomnia.rest/5.0` when the collection contains an ApiSpec or UnitTests | `meta`, `collection`, `cookieJar`, `environments`, `spec`, `testSuites` | `:1423-1461` (type choice `:1429-1435`) |
| `design` | `spec.insomnia.rest/5.0` | same sections, spec always present | `:1462-1494` |
| `environment` | `environment.insomnia.rest/5.0` | `meta`, `environments` only | `:1495-1509` |
| `mock-server` | `mock.insomnia.rest/5.0` | `server`, `routes` | `:1510-1532` |
| `mcp` | `mcpClient.insomnia/5.0` (no `.rest`, unlike the others) | `mcpRequest`, `environments` | `:1533-1551` |

Envelope and field rules:

- `schema_version: 5.1` on every file; `meta` from `mapWorkspaceMeta()` (`:242`).
- `_type` values come from `AllExportTypes` (`src/common/import.ts:43-62`) mapped through
  `MODELS_BY_EXPORT_TYPE` (`:168-189`): `mock`→`MockServer`, `mock_route`→`MockRoute`,
  `api_spec`→`ApiSpec`, `cookie_jar`→`CookieJar`, `mcp_request`→`McpRequest`, etc.
- Private sub-environments are dropped unless `includePrivateEnvironments`
  (`src/common/insomnia-v5.ts:1247`). The UI prompts whenever any sub-environment has
  `isPrivate` (`src/ui/components/settings/import-export.tsx:158-164`, dialog `:77-91`).
- Secrets are never serialized into exports or Git; only their **names** are kept as
  `confidentialKeys` when the environment is KV-pair type (`src/common/insomnia-v5.ts:309-337`,
  `getConfidentialKeys()` `:315-327`). Import rebuilds secret flags from those names (`:337`).
- `requestIds` restricts export to selected requests while keeping every request group
  (`src/common/insomnia-v5.ts:1103-1111`); used by "Export selected requests"
  (`src/ui/components/settings/import-export.tsx:336-346`, invoked from
  `src/ui/components/modals/export-requests-modal.tsx:369`).
- Key order in the emitted YAML is deliberately stable so Git diffs stay small
  (comments `src/common/insomnia-v5.ts:81`, `:327`). MCP URLs get a dedicated builder
  `mcpUrlToInsomniaV5Yaml()` (`:1015`).
- No file-level `id` / `created` / `modified`; entity ids live inside each section and
  the importer regenerates them when writing into a new workspace.

### Export UI surface

`src/ui/components/settings/import-export.tsx`:

| Format | Value | Notes | Cite |
|---|---|---|---|
| Insomnia v5 YAML | `'yaml'` | one folder `insomnia-export.<project>.<epoch-ms>/`, one `<workspace-name>-<workspace-id>.yaml` per workspace | `:37`, `:187-206` |
| HAR | `'har'` | single `.har` file through the native save dialog | `:39`, `:168-183` |
| Legacy `'json'` | `'json'` | no longer offered; the stored preference is auto-migrated to `yaml` | `:55-59` |

- The format modal lists only v5 YAML and HAR (`:43-74`).
- Save dialog names the file `<project>_<yyyy-MM-dd-HH-mm-ss>.<ext>`
  (`showSaveExportedFileDialog` `:93-111`); the last directory is remembered in
  `localStorage['insomnia.lastExportPath']` (`:128-135`).
- Other exporters: `exportMockServerToFile` `:232`, `exportGlobalEnvironmentToFile`
  `:261`, `exportRequestsToFile` `:301`, `exportSpecificationToFile` `:367`.

### HAR export

`src/main/har.ts`:

| Function | Line | Behaviour |
|---|---|---|
| `exportWorkspacesHAR(workspaces, includePrivateDocs=false)` | `:32` | flattens descendants, keeps requests only, filters `isPrivate` docs unless opted in (`:22-29`) |
| `exportRequestsHAR(requests, includePrivateDocs=false)` | `:39` | `JSON.stringify(..., '\t')` (`:95`) |
| `exportHarCurrentRequest(request, response)` | `:103` | single request + response |
| `exportHar(exportRequests)` | `:129` | emits `log.version: '1.2'`, `creator.name: 'Insomnia REST Client'`, `insomnia.desktop.app:v<appVersion>` (`:178-186`) |
| `exportHarResponse(response?)` | `:190` | default empty response when none supplied |
| `exportHarRequest(requestId, envOrWorkspaceId, addContentLength=false, purpose?)` | `:222` | renders through templating |
| `exportHarWithRequest(request, environmentId?, addContentLength=false, purpose?)` | `:246` | main render bridge; `purpose` (`'preview'` for codegen) changes masking |
| `exportHarWithRenderedRequest(renderedRequest, addContentLength=false)` | `:292` | injects `Content-Length` when absent (`:294-306`); adds masked `Authorization` for basic/asap/oauth1/hawk (`getMaskedAuthHeader` `:266-290`); URL encoded with `encodeUrlKeepingMask` (`:293`) |

Caveats:
- Masked auth yields `Authorization: Basic <CONFIDENTIAL_MASK_VALUE>` or plain
  `<CONFIDENTIAL_MASK_VALUE>` (`src/main/har.ts:266-290`). The mask survives URL encoding
  through `encodeUrlKeepingMask` (`src/common/utils/url/confidential-mask.ts:47`).
- `Content-Length` injection is currently requested only for `node/native` codegen
  (`src/ui/components/modals/generate-code-modal.tsx:25-27`, applied `:87-89`), but the
  flag itself is generic in `exportHarWithRenderedRequest`.
- Covered by `src/common/__tests__/har.test.ts:39-507`, including a
  "Generate Code vs explicit export separation" block (`:446-506`) proving preview masking
  is setting-dependent while explicit export always masks.

### API-spec export (design documents)

`exportSpecificationToFile` (`src/ui/components/settings/import-export.tsx:367-418`):
YAML/JSON choice (`:386-395`), syntax detection `detectApiSpecSyntax`
(`src/common/api-specs.ts:46`), conversion `convertApiSpecSyntax` (`:59`); conversion
failure aborts with "The specification is not valid" (`:400-407`). `inso export spec`
reuses the same helpers without the dialogs
(`packages/insomnia-inso/src/commands/export-specification.ts:19-41`).

### Legacy formats 1-4

Still importable, never exported by the UI. The importer chain converts them to an
in-memory `__export_format: 4` envelope (`src/main/importers/convert.ts:56-64`), so
Insomnia's `convert()` remains a "to v4" normalizer, not a v5 producer.

## Importers

Entry point: `convert(importEntry, { importerId? })` in
`src/main/importers/convert.ts:21-70`. The first importer returning a non-null result
wins; `acceptFilePath === true` receives the full `ImportEntry` instead of `contentStr`
(`:35-37`); `convertErrorMessage` results are collected and the next importer is tried
(`:43-51`); results pass `setDefaults()` then `dotInKeyNameInvariant()`, which rejects any
key containing `.` before NeDB sees it (`:49`, definition `:72-89`). If nothing matches,
the thrown error is the joined importer messages or `'No importers found for file'` (`:69`).

Order (`src/main/importers/importers/index.ts:15-27`): insomnia1 → insomnia2 → insomnia3
→ insomnia4 → postman → postman-environment → har → curl → swagger2 → openapi3 → wsdl.
The comment at `:14` says cheap importers are intentionally later.

UI and routes: `src/routes/import.scan.tsx:16` (`scanImportResources`, client action
`:159`), `src/routes/import.resources.tsx:29` (`importScannedResources`, client action
`:52`), modal `src/ui/components/modals/import-modal/import-modal.tsx` (drag/drop,
Electron `getPathForFile` at `:93`, `:136`), paste-anywhere cURL hook
`src/ui/hooks/use-global-curl-paste-import.ts:41-49` gated by
`isCurlCommand` (`src/common/utils/curl.ts:4`).

| Format | Entry point | Supported versions | Quirks |
|---|---|---|---|
| Insomnia v1 | `src/main/importers/importers/insomnia-1.ts:136` | requires `__export_format === 1` (`:147`); JSON only | legacy `items[]` with `environments.base`; format map form/json/text/xml → MIME (`:50-55`); synthetic ids `__GRP_n__`/`__REQ_n__` |
| Insomnia v2 | `insomnia-2.ts:13` | requires `=== 2` (`:22`) | upgrades string `body` to `{mimeType, text}` |
| Insomnia v3 | `insomnia-3.ts:12` | requires `=== 3` (`:21`) | intermediate format, no extra migration |
| Insomnia v4 | `insomnia-4.ts:17` | requires `=== 4` (`:26`); YAML or JSON | main legacy target; produces `resources[]` |
| Insomnia v5 | **not in the chain**. `src/common/import.ts:203-213` tries `tryImportV5Data` whenever raw content `startsWith('type: ')`, then falls back to `convert()` | `schema_version` 5.x, migrated to 5.1 by `migrateToLatestYaml` | leading whitespace/BOM before `type:` defeats detection; invalid v5 surfaces at most 5 Zod errors (`src/common/import.ts:226-231`) |
| Postman collection | `postman.ts:964` | v2.0.0 and v2.1.0 schema URLs (`:121-126`), branch `:411-413` | shape differs per version; `{{$faker}}` → Nunjucks faker tags (`:56-66`); `{{var}}` → Nunjucks; description object unwrapped (`postmanDescriptionToString` `:71-88`); auth cases awsv4/basic/bearer/digest/oauth1/oauth2/apikey (`:554-606`); a top-level Postman folder becomes a **workspace** (`src/common/import.ts:353-356`) |
| Postman environment | `postman-env.ts:26` | `_postman_variable_scope` must be `globals` or `environment` (`:16-31`) | only `enabled` variables survive; one Environment `_id: '__ENV_1__'`; creates one `environment`-scope workspace per env (`src/common/import.ts:364-379`) |
| HAR | `har.ts:99` | HAR 1.2 (`log.entries`) or a bare request-shaped root (`:22-27`) | entry `comment` carried into the request name (`:28-33`); fallback name `HAR Import <n>` (`:87`); requests only, no folders |
| cURL | `curl.ts:468` | command-line text, `curl` or `$ curl` | `SUPPORTED_ARGS` whitelist (`:14`); `IGNORED_VALUE_ARGS` consumed so their values are not taken as the URL (`:40-100`); squished flags like `-XPOST` (`:119`); bearer header wins over `-u` basic; Cookie header merge; `--compressed` → `Accept-Encoding: deflate, gzip`; `-G` moves `-d` into the query; `@file` becomes a file param (`:426`); `--data-urlencode` encodes values; method defaults to POST when a body exists, else GET (`:278`) |
| OpenAPI 3 | `openapi-3.ts:801` | `/^3\.\d+\.\d+$/` (`:33`), else returns `null` (`:808`) | only `application/json` and `*/*` bodies (`:38`); first `servers[]` wins; a server variable without a default **throws** (`:93-97`); creates env vars `scheme`, `base_path`, `host` (`:149-159`) plus security vars; tags become folders with ids `fld___WORKSPACE_ID__<sha1-8>` (`:203-244`); URLs are `{{ _.base_url }}<path>` (`:283`); request ids hash operationId or `[method]path` (`:662-665`); body examples prefer `example` > `default`, skip readOnly, merge `allOf`, take the first `oneOf`/`anyOf`; supports http basic/bearer, apiKey header/query/cookie, oauth2 flows (`:56-57`, `:340-430`) |
| Swagger 2.0 | `swagger-2.ts:495` | `swagger === '2.0'` (`:10`, check `:500`) | MIME json/urlencoded/multipart (`:14`); URLs `{{ _.base_url }}...` (`:142`); basic/apiKey/oauth2 with flows implicit/password/application/accessCode mapped onto Insomnia oauth2 (`:190-265`) |
| WSDL | `wsdl.ts:102` (`acceptFilePath = true` `:19`) | WSDL 1.1 namespace `http://schemas.xmlsoap.org/wsdl/`, root `definitions` (`:130-137`) | uses the patched `apiconnect-wsdl@2.0.36` (comments `:2-3`) to emit a Postman v2.0 collection, then delegates to `postman.convert`; prefers `importEntry.oriFilePath` so `import`/`include` XSD resolve (`:114-116`); failures become `convertErrorMessage` (`:125`) |
| MCP URL | `mcpUrlToInsomniaV5Yaml` (`src/common/insomnia-v5.ts:1015`) | emits v5 `mcpClient.insomnia/5.0` directly | bypasses the importer chain |

After scanning, resources are written by `importResourcesToProject`
(`src/common/import.ts:335`), `importResourcesToWorkspace` (`:455`, merges the base
environment per `overrideBaseEnvironmentData`), or `importResourcesToNewWorkspace`
(`:590`, creates the workspace and ApiSpec before copying children). Names pass through
`sanitizeGitFileNameSegment` (`:716-719`), so an imported title cannot inject `/`, `\`,
`..` or NUL into a Git path. Duplicate detection:
`findExistingImportedWorkspace` (`:783`) and `findRequestInExistingWorkspace` (`:861`).

Deep links: `parseDeepLinkUrl` (`src/common/import-deep-link.ts:8`, normalizes
`insomniadev://` at `:21`) and `resolveImportDeepLink` (`:58`). Source precedence is
`uri > mcp > curl > clipboard > file`; value-bearing params win; `endpoint`/`operationId`
apply only to `uri`/`curl` (`:57-72`); selector names are case-insensitive while metadata
stays case-sensitive (comment `:49`). `fetchImportContentFromURI`
(`src/common/import.ts:82-96`) rewrites `github.com` to `raw.githubusercontent.com`, reads
`file://` via `insecureReadFile`, and otherwise `decodeURIComponent`s.

Import-modal behaviour worth replicating: auto-scan only for `uri`/`curl`/`mcp`
(`import-modal.tsx:218-236`); an existing workspace found during scan is navigated to
instead of duplicated (`:243-289`); importing two or more workspaces into ScratchPad is
blocked (`:395-410`); Postman `.zip` dumps are extracted by
`getFilesFromPostmanExportedDataDump` (`src/common/import.ts:109`).

## Code snippet generation

A thin layer over `httpsnippet ^3.0.10` (`packages/insomnia/package.json:122`).

| Piece | Cite |
|---|---|
| IPC `generateCodeSnippet({har,target,client})` | `src/main/ipc/main.ts:573-579` - `new HTTPSnippet(har).convert(target, client)` |
| IPC `getCodeSnippetTargets()` | `src/main/ipc/main.ts:580-582` - `availableTargets()` |
| Channel allow-list | `src/main/ipc/electron.ts:29-30` |
| HAR construction | `src/main/har.ts:246-290` via `exportHarWithRequest(..., purpose: 'preview')` |
| Masked URL preserved | `convertHarRequestKeepingMask` (`src/common/utils/url/confidential-mask.ts:53-70`) wraps the HTTPSnippet input; `encodeUrlKeepingMask` `:47` |
| Modal | `src/ui/components/modals/generate-code-modal.tsx` |
| Defaults | `shell` / `curl` (`:73-81`); persisted in `localStorage['insomnia::generateCode::target|client']` (`:52-56`, `:84-85`) |
| Editor mode map | `MODE_MAP` `:17-23` (c/java/csharp/objc→`clike`, node→`javascript`, ocaml→`mllike`) |
| Content-Length hack | only `node`/`native` (`:25-27`, applied `:87-89`) |
| Types | `src/types/code-snippet.ts` (`HTTPSnippetTarget`, `HTTPSnippetClient`) |

The mask placeholder is a random per-process token (`MASK_PLACEHOLDER`,
`confidential-mask.ts:12`), swapped in before conversion and restored afterwards, so
generated code never leaks the literal `CONFIDENTIAL_MASK_VALUE` and never receives a
fabricated value. The modal credits Kong/httpsnippet in its footer. Heric should keep the
same target/client keys to reuse stored preferences.
## Git sync

### On-disk model

- The clone root is `.` and Insomnia data lives under `.insomnia/`
  (`GIT_CLONE_DIR`, `GIT_INSOMNIA_DIR_NAME` at `src/sync/git/git-vcs.ts:104-108`).
- Layout: `.insomnia/<ModelType>/<id>.yml|yaml`, parsed by
  `src/sync/git/parse-git-path.ts:19-32` (strips `.json`/`.yml`).
- Workspaces are written at the repo **root** as `insomnia.<workspaceId>.yaml`
  (`src/sync/git/repo-file-watcher.ts:280-281`, `:524-525`;
  `src/main/git-service.ts:1279-1284`), because one workspace file carries its whole
  collection tree. `WorkspaceMeta.gitFilePath` records the path
  (`repo-file-watcher.ts:326`, rename detection `:535`).
- Auxiliary YAML names use `safeToUseInsomniaFileNameWithExt`
  (`src/sync/git/insomnia-filename.ts:1-14`: lowercase, non `[a-z0-9_-]` → `_`).

### VCS engine (`src/sync/git/git-vcs.ts`, 2283 lines, isomorphic-git)

| Method | Line |
|---|---|
| `init` / `initFromClone` | `:157` / `:223` |
| `getCurrentBranch`, `listBranches`, `listRemoteBranches`, `fetchRemoteBranches` | `:271`, `:301`, `:315`, `:324` |
| `fileStatus(file)` (raw HEAD/index blobs and oids) / `filesStatus()` | `:349` / `:485` |
| `diff`, `statusWithIntelligentDiff`, `readFileAtHead`, `status` | `:645`, `:941`, `:992`, `:1042` |
| `addRemote`, `listRemotes`, `getBranchTrackingRemote`, `getRemoteUrl`, `getBranchRemoteInfo` | `:1098`, `:1115`, `:1119`, `:1132`, `:1144` |
| `setAuthor`, `commit`, `canPush`, `push` | `:1155`, `:1185`, `:1198`, `:1230` |
| `pullWithConflictSupport`, `handleGitPullErrors`, `buildManualResolutionFromTrees`, `collectMergeConflicts`, `continueMerge`, `merge` | `:1291`, `:1399`, `:1486`, `:1701`, `:1858`, `:1933` |
| `fetch`, `log`, `branch`, `deleteBranch`, `checkout` | `:1985`, `:2009`, `:2033`, `:2048`, `:2052` |
| `stageChanges`, `unstageChanges`, `stagePartialContent`, `discardPartialContent`, `discardChanges`, `abortMerge` | `:2104`, `:2112`, `:2123`, `:2136`, `:2141`, `:2192` |

`pullWithConflictSupport` refuses to start with uncommitted changes
(`UncommittedChangesError`, `:1293-1296`), routes writes through a `WriteFileMap` and
reports `blockedWrites` for symlink escapes (`:1297-1329`). Typed errors live in
`src/sync/git/git-vcs-operation-errors.ts` (also `RequiredPullRemoteChangesError`,
`AuthenticationRequiredError`).

### Filesystem/DB bridge

`RepoFileWatcher` (`src/sync/git/repo-file-watcher.ts`, ~1430 lines) is a bidirectional
watcher driven by a serial `SyncQueue`:

| Concern | Cite |
|---|---|
| Poll interval 10 s, debounce 300 ms | `:56-57` |
| `create`, `stop`, `flushNow`, `importAllFiles` | `:159`, `:193`, `:229`, `:352` |
| DB change listener → debounced disk flush | `:398-460`, `:488` |
| project ruleset (`.spectral.yaml`) flush | `:592` |
| FS watch + polling fallback | `:650`, `:676` |
| `scheduleImport`, `importFile`, `readIfChanged` | `:725`, `:762`, `:808` |
| `parseAndValidate` | `:864` - the file's first line must contain a known v5 `type`; `<<<<<<<`/`>>>>>>>` are reported as a conflict |
| `deleteOrphans`, `upsertDocs`, `handleFileDeletion` | `:901`, `:920`, `:960` - upsert re-parents workspaces to the project and stamps `WorkspaceMeta.gitFilePath` / `gitFileLastSyncTime` |
| `flushWorkspaceToDisk` | `:1158` - path-traversal guard (`:1159-1163`), export via `getInsomniaV5DataExport(..., includePrivateEnvironments: false)` (`:1166-1169`), content hash recorded to ignore its own writes |
| `getProblems` | `:1243`; registry `RepoFileWatcherRegistry` `:1321-1420` |

Outbound writes never contain private environment values, so a repository cannot carry
secrets. Inbound mirrors this: private docs are unreadable (`readFile` throws; test
`src/sync/git/__tests__/ne-db-client.test.ts:179`) and `writeFile` refuses anything outside
`.insomnia` (`:121-124`), skips content containing `=======` (`:130-132`), validates path
id/type against the document (`:139-148`) and requires `models.canSync(doc)`
(`:149-152`), so a smuggled `Settings` document cannot overwrite global settings.

Path containment lives in `src/sync/git/fs-client.ts`: `resolveWithinBase` `:51`,
`resolveOrReport` `:73`, `isSafeSymlinkTarget` `:122`, `intoGitViaLink` `:146`; refused
writes are reported as `BlockedWrite` (`:25-29`). The real-FS client is `fsClient` (`:31`);
the NeDB-backed client is `ne-db-client.ts:47-290` (`mkdir`/`symlink` throw, `:230`,
`:290`). `routable-fs-client.ts:21` routes `.insomnia`/`.git` prefixes to different clients
and injects `.insomnia` into the root listing.

Canonical-file replay (`src/sync/git/canonical-file.ts`):
`getCanonicalInsomniaFileContent` `:24`, `getInsomniaFileFormatIssue` `:70`,
`canonicalizeInsomniaFileContent` `:88` (refuses when replay would invent entity ids),
`isUndiscardableChange` `:128` (distinguishes a change produced purely by Insomnia
re-serializing a committed file from a real edit). This is what prevents endless
"modified" files after clone.

Hydration happens against an in-memory DB (`src/sync/git/mem-client.ts`, tests
`__tests__/mem-client.test.ts`): `initGitRepoCloneAction` shallow-clones, migrates YAML,
validates with `InsomniaFileSchema.safeParse`, and only then touches the real DB
(`src/main/git-service.ts:1352-1444`).

Migrations: `migrateRepoStructureIfNeeded` (`src/sync/git/git-repo-migration.ts:200`)
moves legacy `git/` → `.git/`, `other/` → repo root, and DB-only YAML →
`insomnia.<id>.yaml`; idempotency uses `GitRepository.repoMigrationVersion` and
`CURRENT_MIGRATION_VERSION = 1` (`src/sync/git/git-migration-version.ts:5`). It rejects
non-absolute base directories (`:203-210`). Legacy `.insomnia/` folders are imported by
`importLegacyInsomniaFolder` (`src/main/git-service.ts:1188`), which validates `_id`/type,
skips models failing `canSync`, and deletes the folder after migration (tests
`src/main/__tests__/git-service-import-legacy-insomnia-folder.test.ts`).

### Service actions (`src/main/git-service.ts`, 4361 lines)

Clone/open/init: `loadGitRepository` `:800`, `initGitRepoCloneAction` `:1352`,
`cloneGitRepoAction` `:1445`, `openGitRepoAction` `:1923`, `cleanupGitRepoStorageAction`
`:2057`, `relocateGitRepoAction` `:2084`, `updateGitRepoAction` `:2234`,
`resetGitRepoAction` `:2335`, `checkGitRepoDirectoryAction` `:4100`,
`resolveGitRepoFolderPathAction` `:4142`.

Commit/push/pull: `commitToGitRepoAction` `:2378` (`flushNow` → `setAuthor` → `commit` →
`canPush`, then updates `hasUnpushedChanges`), `multipleCommitToGitRepoAction` `:2423`,
`commitAndPushToGitRepoAction` `:2496`, `continueMerge` `:3232`, `pushToGitRemoteAction`
`:2967`, `fetchGitRemoteBranches` `:3099`, `pullFromGitRemote` `:3133` (returns
`MergeConflictError.data`, maps 401/403 to `AuthenticationRequiredError`),
`discardChangesAction` `:3305`, `abortMergeAction` `:3346`, `gitStatusAction` `:3358`,
`stageChangesAction` `:3391`, `unstageChangesAction` `:3418`,
`stagePartialContentAction` `:3445`, `discardPartialContentAction` `:3475`,
`diffFileLoader` `:3580` (debug-mode-only diagnostics; `unknownFieldRanges` `:3648`),
`fileFormatIssueLoader` `:3667`.

Branches: `createNewGitBranchAction` `:2645`, `checkoutGitBranchAction` `:2728`,
`deleteGitBranchAction` `:2923`. Project-wide: `backfillAllManagedGitFolderSlugs` `:3968`,
`runAllGitRepoMigrations` `:4003` (path-traversal guard; failed migrations are converted
into local projects). Credential helpers: `validateGitRepositoryCredentials` `:751`,
`validateGitCredentialById` `:776`. AI commit-message generation runs in a
`utilityProcess` (`src/main/ipc/main.ts:814-830`; worker
`src/main/git-commit-generation-process.mjs`, calls `generateCommitsFromDiff` from a
configured `aiPluginName`).

Issue surfacing: `getProjectGitFileIssues` `:315` (maps watcher problems to workspaces)
and `getProjectRulesetImportIssue` `:344` (reports a rejected git-imported
`.spectral.yaml`, shown passively on the spec page).

### Remote providers (`src/sync/git/providers/`)

| Provider | OAuth | Auto-renew | Notes | Cite |
|---|---|---|---|---|
| `github` | yes | **no** - comment: GitHub OAuth Apps don't support refresh tokens yet | OAuth app URL `/oauth/github-app`; repo/email listing | `github.ts:76-81`, `:270`, `:302` |
| `gitlab` | yes | yes (`refresh_token`) | configurable `instanceUrl`; hostname pinning | `gitlab.ts:142-147`, `:160-168`, `:532-560` |
| `custom` | no | no | username + token (PAT); cancels on auth failure | `custom.ts:16-21`, `:80` |
| `native` | no | no | shells out to OS `git credential fill`, stores no secret | `native.ts:19-24`; author read from `git config user.name/user.email` (`src/sync/git/utils.ts:102-130`) |

Registry `GitRemoteProviderRegistry` (`registry.ts:3`) with `detectProviderForUrl` (`:84`);
all four registered in `providers/index.ts:initializeGitRemoteProviders`. OAuth expiry
helper `expiresAtFromOAuthExpiresIn` (`utils.ts:20`); isomorphic-git callbacks
`gitCallbacks` (`utils.ts:142-149`). Git HTTP traffic uses Electron `net` so the system
proxy and certificates apply (`src/sync/git/http-client.ts:5-13`). URL helpers
`addDotGit`, `isAzureDevOpsUrl` (dev.azure.com, `*.visualstudio.com`, `/_git/`),
`ensureGitRepoUrlSuffix` live in `src/sync/git/url-utils.ts` (re-exported `utils.ts:13`).

### Cloud VCS (`packages/insomnia-vcs`)

Separate from Git and out of scope for a local-first Heric, but its merge code is useful
reference.

- `VCS` (`src/vcs.ts`): `status` `:165`, `stage` `:247`, `unstage` `:284`, `fork` `:335`,
  `checkout` `:382`, `rollbackToLatest` `:442`, `rollback` `:453`, `getHistory` `:499`,
  `merge` `:545`, `takeSnapshot` `:551`, `pull` `:578`, `push` `:642`.
- Three-way merge helpers (`src/util.ts`): `threeWayMerge` `:67`, `compareBranches`
  `:241`, `stateDelta` `:284`, `getStagable` `:316`, `applyStageToState` `:374`,
  `preMergeCheck` `:416`, `hash` `:467`, `hashDocument` `:483`,
  `updateStateWithConflictResolutions` `:495`.
- Crypto (`src/crypt.ts`): RSA-OAEP-256 JWK encrypt/decrypt and AES-GCM; runtime deps are
  only `node-forge` and `clone` (`package.json:29-32`).
- Storage (`src/store/`): key/value `Store` with hooks (`index.ts:19`), gzip hook when the
  key has no extension (`hooks/compress.ts`), filesystem driver writing
  `<path>.<uuid>.tmp` then renaming, under `<userData>/version-control`
  (`drivers/file-system-driver.ts:10-49`).
- Project bootstrap: `src/sync/vcs/initialize-backend-project.ts:49` takes an
  "Initial Snapshot" and sets `pushSnapshotOnInitialize` (`:52`).
- Snapshot delta encoding: `src/sync/delta/diff.ts:22`, `src/sync/delta/patch.ts:3`.

## Design mode and OpenAPI lint

- A design document is an `ApiSpec` attached to a workspace. The old
  `...workspace.$workspaceId.spec` route now redirects to `.../debug`
  (`src/routes/organization.$organizationId.project.$projectId.workspace.$workspaceId.spec.tsx:9-18`).
  Saving goes through `...spec.update.tsx:11-40` (optionally calls
  `generateRequestCollection`, `:17`, `:33`), implemented in
  `...spec.generate-request-collection.tsx:12-62`: it runs `scanResources` on the spec and
  imports the result into the same workspace.
- Editor `src/ui/components/spec-view.tsx` registers a CodeMirror lint helper named
  `'openapi'` (`:228-277`) that calls
  `window.main.lintSpec({ documentContent, projectId, rulesetContent })` (`:231-235`);
  diagnostics map to CodeMirror positions (`:246-259`); autosave is debounced 500 ms
  (`:317`). Non-Git projects mirror `ProjectLintRuleset.contents` to
  `<userData>/projects/<projectId>/.spectral.yaml` (`:177-181`); Git projects lint against
  `gitSyncRulesetPath` (`:286-292`, `:458`).
- Empty state offers templates `petStoreSpec`, `todoSpec`, `blankSpec`
  (`src/ui/components/design-empty-state.tsx:10`, `:27`; sources
  `src/ui/components/example-openapi-specs.ts:1`, `:778`, `:920`), file import, and
  create request/folder.
- Ruleset routes: `...project.$projectId.update-ruleset.tsx`,
  `...refresh-ruleset.tsx`, `...delete-ruleset.tsx`.

Lint execution (`src/main/ipc/main.ts:496-570`):

1. `bundleSpectralRuleset(sourcePath)` (`:496-503`) flattens local `extends`
   (`src/main/bundle-spectral-ruleset.ts:278`); `compileSpectralRulesetFromContent`
   (`:294`) inlines remote rulesets, closing the validate-then-use race.
2. Compiled content is cached at `<userData>/projects/<projectId>/.spectral.yaml`
   (`src/main/__tests__/spectral-ruleset-cache.test.ts:47-105`).
3. A `utilityProcess.fork('main/lint-process.mjs')` performs the Spectral run (`:530`);
   only one worker is kept (the previous one is killed) and it is terminated after
   `LINT_WORKER_TIMEOUT_MS = 30_000` (`:535-545`).
4. Result is `{ diagnostics }`, `{ error }` or `{ cancelled }` (`:546-570`).

Worker safeguards (`src/main/lint-process.mjs`): https-only resolver (`:61`), explicit
private/loopback block list (`:17-60`), and a post-DNS re-resolution guard
`assertResolvesToPublicHost` (`:65-72`) applied to both http and https (`:93-95`). The
shared logic is `src/main/private-host.ts:10-37` (0/8, 127/8, 10/8, 172.16/12,
192.168/16, 169.254/16, `::1`, `fc00::/7`, `fe80::/10`); `inso` duplicates it in
`packages/insomnia-inso/src/commands/lint-specification.ts:15-60`.

Ruleset validator (`src/common/spectral-ruleset-validator.ts`): top level limited to
`rules` and `extends` (`:7`); `extends` limited to `spectral:oas|asyncapi|arazzo` (`:10`);
only a fixed list of built-in functions is allowed (`:13-29`);
`__proto__`/`prototype`/`constructor` rejected (`:31`, `:61`); `field` may not contain
`.`, `[`, `]`; `documentationUrl` must be https (`:124-125`); entry
`validateSpectralRuleset` `:141-170`.

## inso CLI commands and flags

Entry `packages/insomnia-inso/src/cli.ts` (1061 lines), commander-based, exit codes 0/1.
Configuration comes from `.insorc` via cosmiconfig (`tryToReadInsoConfigFile` `:73-93`);
**only** `workingDir`, `ci`, `verbose`, `printOptions` are honoured from it (`:85-93`).
`--verbose` raises the consola level (`:308`); `--ci` forces a basic reporter and
disables prompts (`:309`).

Global options (`:344-348`): `-w/--workingDir <dir>`, `--verbose`, `--ci`,
`--config <path>`, `--printOptions`.

| Command | Line | Flags |
|---|---|---|
| `run test [identifier]` | `:354` | `-e/--env`, `-t/--testNamePattern`, `-r/--reporter` (default `spec`), `-b/--bail`, `--keepFile`, `--requestTimeout`, `-k/--disableCertValidation`, `--httpsProxy`, `--httpProxy`, `--noProxy`, `-f/--dataFolders` (`:356-374`) |
| `run collection [identifier]` | `:494` | `-t/--requestNamePattern`, `-i/--item` (repeatable), `-e/--env`, `-g/--globals` (id or file), `--delay-request`, `--requestTimeout`, `--env-var` (repeatable `key=value`), `-n/--iteration-count`, `-d/--iteration-data` (JSON/CSV path or URL), `-r/--reporter`, `-b/--bail` (first non-200), `--disableCertValidation`, proxies, `-f/--dataFolders`, `--output <file>`, `--includeFullData <redact|plaintext>`, `--acceptRisk` (`:496-531`) |
| `lint spec [identifier]` | `:896` | `-r/--ruleset <path>` (`:897-900`); identifier is a spec file or a DB spec id/name |
| `export spec [identifier]` | `:962` | `-o/--output <path>`, `-s/--skipAnnotations` (`:964-965`) |
| `script <script-name>` | `:1008` | runs a command from `.insorc.scripts`; must start with `inso`; unknown options pass through (`:1012-1034`) |
| `generate-docs` | `:1041` | hidden; prints docs then exits 1 |

Behaviour details:

- Data-source resolution `loadDb` (`packages/insomnia-inso/src/db/index.ts:22-68`): export
  file (when the path is a file) → Git repo `.insomnia` folder → NeDB app-data directory
  → `emptyDb()` plus a warning listing the valid workingDir shapes (`:48-65`).
- Export-file adapter accepts **only** `__export_format` 4 and 5
  (`db/adapters/insomnia-adapter.ts:107-115`); v5 goes through `importInsomniaV5Data`
  (`:4`, `:97`).
- Git adapter reads `.insomnia/<Type>/*`, requires `obj.type === folder type`, and keeps
  only `models.getModel(t)?.canSync` types (`db/adapters/git-adapter.ts:41-57`).
- `run test` resolves suites via `loadTestSuites` (`db/models/unit-test-suite.ts:18-29`);
  the identifier may match an ApiSpec, a workspace, a suite id or a suite name. Without an
  identifier it prompts unless `--ci`, in which case no suites are found and it exits 1
  (`cli.ts:408-412`).
- `run collection --includeFullData` without `--acceptRisk` shows an interactive security
  warning and aborts when refused (`cli.ts:578-594`); the output path is checked for
  file-ness and writability first (`:561-576`).
- `lint spec`: file identifiers auto-detect a sibling `.spectral*` ruleset
  (`getRuleSetFileFromFolderByFilename`, `commands/lint-specification.ts:69-81`); DB
  identifiers resolve through `resolveSpecInDatabase` (`cli.ts:152-166`, matching id-ish,
  `fileName` or `name` - `db/models/api-spec.ts:10-17`). Custom rulesets are bundled and
  compiled (`commands/lint-specification.ts:92-112`), written to an `mkdtemp` directory,
  and removed after load. Diagnostics print as
  `line:col - SEVERITY - code - message - path` (`:130-137`); exit 1 only when at least one
  diagnostic has Error severity (`:140-143`).
- `export spec`: `x-kong-*` keys are removed recursively and the document is
  re-stringified as YAML (`commands/export-specification.ts:25-41`);
  `writeFileWithCliOptions` creates parent directories (`:8-17`). Without `--output` it
  prints to stdout (`cli.ts:982-988`).
- Iteration data: `pathToIterationData` (`cli.ts:207-218`) merges `--env-var` overrides
  into each row; `getListFromFileOrUrl` (`:219-246`) accepts a JSON array of objects or
  CSV parsed with Papa (quoted commas, doubled quotes, CRLF, embedded newlines, short rows
  padded with `''`, fewer than 2 rows throws).
- Reporters: `['dot','list','min','progress','spec','tap']` (`reporter/index.ts:4`); TAP
  emits `TAP version 13` (`:14-25`); summary printer `:81-105`.
  `RunCollectionResultReport` (`commands/run-collection/result-report.ts:38`) implements
  `redact`/`plaintext`, redacting sensitive headers, auth fields outside a whitelist, and
  environments (`:181-230`).

## Existing tests and fixtures usable as golden tests

All are in-repo and run on existing snapshot/fixture harnesses; they can be replayed
against Heric without re-authoring inputs.

| Area | Test / fixture | What it pins | Cite |
|---|---|---|---|
| Importer snapshots | `src/main/importers/importers/index.test.ts` | every `*-input.*` fixture through `convert()`, `Date.now` frozen, snapshot-compared | `:5-45` |
| Importer fixtures | `src/main/importers/importers/fixtures/{curl,har,insomnia-1..4,openapi3,postman,postman-env,swagger2,wsdl}` | 75 input files (curl 14, openapi3 14, postman 25, swagger2 7, har 5, wsdl 2, ...) | directory listing |
| cURL parser | `importers/curl.test.ts:5` (hundreds of cases), `:355` non-cURL input returns null, `:362` explicit Authorization beats `-u` | flag handling, method/body inference | |
| Postman | `importers/postman.test.ts:6-640` | Nunjucks transform, JSONPath normalization, headers, awsv4/basic/bearer/digest/oauth1/oauth2, URL, descriptions | |
| OpenAPI 3 | `importers/openapi-3.test.ts:7-300` | object query-param flattening, nested objects, allOf/oneOf/anyOf | |
| Script translation | `importers/translate-postman-script.test.ts:5-200` | `pm.*` → Insomnia script translation | |
| HAR export | `src/common/__tests__/har.test.ts:39-507` | single/multiple requests, response export, preview-vs-export masking | |
| Ruleset validation | `src/common/__tests__/spectral-ruleset-validator.test.ts` | empty/invalid YAML, disallowed keys, prototype pollution | |
| Ruleset bundling | `src/main/__tests__/bundle-spectral-ruleset.test.ts:68-240` | flattening, override order, cycles, depth, tuple format | |
| Ruleset cache | `src/main/__tests__/spectral-ruleset-cache.test.ts` | per-project path, skip-if-unchanged, recompile after external delete | |
| Git VCS | `src/sync/git/__tests__/git-vcs.test.ts:9-900` | stage/unstage, commit, branches, raw HEAD blobs, push upstream, undo pending changes, manual resolution, tracking remotes | |
| Git FS safety | `__tests__/fs-client.test.ts:9-270` | 19 symlink-containment cases (escape, chain, broken link, `.git` write) | |
| NeDB client | `__tests__/ne-db-client.test.ts:24-620` | read/write/readdir/stat/unlink, private-doc rejection, conflict markers, id/type mismatch, `canSync` restriction | |
| Watcher | `__tests__/repo-file-watcher.test.ts:36-330` | orphan reconciliation, folder resurrection, ruleset problems, symlink write containment | |
| Socket.IO payloads | `__tests__/repo-file-watcher-payload.test.ts:72-140` | payloads survive fs→db round-trips | |
| Canonical files | `__tests__/canonical-file.test.ts:87-320` | replay stability, `confidentialKeys` survival, undiscardable changes | |
| Repo migration | `__tests__/git-repo-migration.test.ts:35-215` | `git/`→`.git/`, `other/`→root, version stamp, symlink refusal | |
| Sync queue | `src/sync/git/sync-queue.test.ts:5-140` | FIFO, single-flight, `waitUntilDone`, error isolation | |
| Git service | `src/main/__tests__/git-service-{diff-diagnostics,import-legacy-insomnia-folder,repo-folder,clone-folder-naming}.test.ts` | debug-mode gating, legacy `.insomnia` migration, relocate/adopt rules | |
| Delta (cloud) | `src/sync/delta/__tests__/{diff,patch}.test.ts` | block map, operations, patch round-trip | |
| Cloud VCS | `packages/insomnia-vcs/src/__tests__/{vcs,util,crypt,backend-projects}.test.ts`, `store/__tests__/index.test.ts`, `store/hooks/__tests__/compress.test.ts` | staging, snapshots, fast-forward merge, three-way conflict matrix, AES/RSA, gzip hook | |
| inso CLI end-to-end | `packages/insomnia-inso/src/cli.test.ts:16-79` | literal command arrays for exit 0 and exit 1 covering lint/export/run test/run collection across NeDB, Git, v4 and v5 | |
| inso unit | `iteration-data.test.ts`, `commands/lint-specification.test.ts`, `commands/export-specification.test.ts`, `db/adapters/{git,insomnia,ne-db}-adapter.test.ts`, `reporter/index.test.ts`, `reporters/fancy-reporter.test.ts` | CSV edge cases, lint pass/fail, `x-kong-*` stripping, adapter shapes, reporter output | |
| inso fixtures | `src/db/fixtures/{nedb,git-repo,git-repo-malformed-spec,insomnia-v4,insomnia-v5,certs}`, `src/commands/fixtures/{openapi-spec.yaml,with-ruleset}`, `src/examples/*.yml` | DBs, YAML exports, malformed specs, certificates, 15 runnable example collections | |
| App export/import E2E | `packages/insomnia-smoke-test/tests/smoke/{export,export-openapi-spec,import,import-from-url,import-deep-link,paste-curl-import,custom-lint-rules}.test.ts` | UI flows; `export.test.ts:15-30` compares against `fixtures/export/*.yaml` | |
| App Git E2E | `tests/smoke/{git-sync,git-local-repos,git-repo-relocation,disable-git-sync}.test.ts` | branch/commit/merge/push against a bundled bare repo, folder relocation | |
| Fixtures | `packages/insomnia-smoke-test/fixtures/{export,import,files,git-repo/git-server.git,inso-nedb,insomnia-legacy-db,certificates}` | canonical v5 YAMLs, Postman input, valid/invalid `.spectral.yaml`, CSV/PDF/XML/TXT uploads, DB snapshots | |

Gaps: no unit-level golden test for the exported v5 YAML of a whole project (only the two
smoke-test fixtures), and no unit test for `generateCodeSnippet` itself - the HAR that
feeds it is tested, the `httpsnippet` call is not.

## Feature list

Priority = implementation importance for Heric parity, not open-source ordering. Each row
is read from the cited path.

| # | Feature | Where | Edge cases / caveats | P |
|---|---|---|---|---|
| 1 | v5 YAML export per workspace | `packages/insomnia/src/common/insomnia-v5.ts:1043` | scope→`type` map including the `mcpClient.insomnia/5.0` oddity; Zod validation; empty-field stripping | P0 |
| 2 | v5 YAML import + migration to 5.1 | `src/common/insomnia-v5.ts:1006`, `:990`; `src/common/insomnia-schema-migrations/index.ts:60` | `startsWith('type: ')` sniffing; at most 5 Zod errors reported | P0 |
| 3 | Project export folder (one YAML per workspace) | `src/ui/components/settings/import-export.tsx:187-206` | private-env prompt; folder name `insomnia-export.<project>.<ms>` | P0 |
| 4 | Selected-request export (`requestIds`) | `src/common/insomnia-v5.ts:1103-1111`; `import-export.tsx:336-346` | keeps all request groups | P1 |
| 5 | HAR 1.2 export (workspace/request/current) | `src/main/har.ts:32`, `:39`, `:103` | tab-indented JSON; `Content-Length` injection; masked auth | P0 |
| 6 | HAR import | `src/main/importers/importers/har.ts:99` | bare-request root accepted; comments become names; no folders | P0 |
| 7 | cURL import (text + clipboard paste) | `importers/curl.ts:468`; `src/ui/hooks/use-global-curl-paste-import.ts:41` | flag whitelist/ignored-value args; squished `-XPOST`; `@file`; `--data-urlencode` | P0 |
| 8 | Postman v2.0/v2.1 collection import | `importers/postman.ts:964` | faker/Nunjucks transform; description object; 7 auth types; folder→workspace | P0 |
| 9 | Postman environment import | `importers/postman-env.ts:26` | only `enabled` vars; one environment workspace each | P1 |
| 10 | OpenAPI 3 import to collection | `importers/openapi-3.ts:801` | server variable without default throws; tags→folders; `{{ _.base_url }}`; security→env vars | P0 |
| 11 | Swagger 2.0 import | `importers/swagger-2.ts:495` | oauth2 four flows; urlencoded/multipart bodies | P0 |
| 12 | WSDL import | `importers/wsdl.ts:102` | needs patched `apiconnect-wsdl@2.0.36`; file-path only; delegates to the Postman importer | P1 |
| 13 | Legacy Insomnia v1-v4 import | `importers/insomnia-{1,2,3,4}.ts:136/:13/:12/:17` | strict `__export_format` equality; v2 string-body upgrade | P2 (v1-v3), P1 (v4) |
| 14 | Postman data-dump `.zip` import | `src/common/import.ts:109`; `src/routes/import.scan.tsx:16` | extraction errors become `'Extract failed'` | P1 |
| 15 | Import deep links (`insomnia://`, `insomniadev://`) | `src/common/import-deep-link.ts:8`, `:58` | source precedence; `endpoint`/`operationId` only for uri/curl; case-sensitivity split | P1 |
| 16 | Import-to-existing-workspace / duplicate detection | `src/common/import.ts:783`, `:861`; `src/routes/import.resources.tsx:52` | design doc matched by `"title version"`; MCP by URL | P1 |
| 17 | Git-safe filename sanitization | `src/common/import.ts:716-719` | strips `/`, `\`, `..`, NUL; blank → `workspace` | P1 |
| 18 | Code snippet modal and targets | `src/ui/components/modals/generate-code-modal.tsx:52-110` | persisted target/client; clike/javascript mode mapping; node Content-Length | P0 |
| 19 | HAR-to-snippet IPC with mask preservation | `src/main/ipc/main.ts:573-582`; `src/common/utils/url/confidential-mask.ts:53` | random placeholder token; preview masking setting | P0 |
| 20 | Git clone/init into managed or user folder | `src/main/git-service.ts:1352`, `:1445`, `:1923` | shallow clone in MemClient; schema validation; legacy `.insomnia` detection | P0 |
| 21 | Git commit/push/pull with conflict flow | `git-service.ts:2378`, `:2496`, `:3133`; `src/sync/git/git-vcs.ts:1291`, `:1933` | uncommitted-changes guard; 401/403→auth error; blocked symlink writes | P0 |
| 22 | Branch create/checkout/delete | `git-service.ts:2645`, `:2728`, `:2923` | - | P0 |
| 23 | File watcher (FS↔DB, 10 s poll + 300 ms debounce) | `src/sync/git/repo-file-watcher.ts:56-57`, `:352`, `:488` | conflict markers; self-write hash skip; orphan cleanup; ruleset sync | P0 |
| 24 | Canonical YAML replay / undiscardable detection | `src/sync/git/canonical-file.ts:24`, `:70`, `:88`, `:128` | prevents phantom diffs after clone; refuses replay that invents ids | P0 |
| 25 | NeDB-backed VCS filesystem and write guards | `src/sync/git/ne-db-client.ts:116-152` | `.insomnia`-only, conflict-marker skip, id/type check, `canSync` allow-list | P0 |
| 26 | Path containment / blocked writes | `src/sync/git/fs-client.ts:51-170` | symlink escapes, `.git` writes, traversal segments | P0 |
| 27 | Repo layout migration (`git/`→`.git/`, root YAML) | `src/sync/git/git-repo-migration.ts:200` | absolute-path guard; idempotent version stamp | P1 |
| 28 | Legacy `.insomnia` folder import | `src/main/git-service.ts:1188` | rejects Settings smuggling; deletes folder after migration | P1 |
| 29 | Git credentials: GitHub/GitLab/Custom/Native | `src/sync/git/providers/{github,gitlab,custom,native}.ts` | GitHub has no refresh; GitLab instance-URL pinning; native shells out to `git credential fill` | P1 |
| 30 | Git diff / visual merge / partial stage | `git-service.ts:3580`, `:3445`; `src/ui/components/diff-view-editor.tsx`; `modals/visual-diff/entity-splice.ts` | debug-mode-only diagnostics; unknown-field ranges | P1 |
| 31 | Relocate / open folder / migration runner | `git-service.ts:2084`, `:1923`, `:4003` | refuses non-empty non-git targets; failed migrations become local projects | P1 |
| 32 | AI commit message / commit splitting | `src/main/ipc/main.ts:814`; `src/main/git-commit-generation-process.mjs` | requires an AI plugin; utilityProcess isolation | P2 |
| 33 | Design spec editor + autosave | `src/ui/components/spec-view.tsx:228-317`; route redirect `...spec.tsx:9-18` | 500 ms debounce; spec route merged into debug | P0 |
| 34 | Spectral lint (utility process, 30 s cap) | `src/main/ipc/main.ts:505-570`; `src/main/lint-process.mjs` | one worker at a time; cancelled/error/timeout states | P0 |
| 35 | Custom ruleset validation, bundling, SSRF guard | `src/common/spectral-ruleset-validator.ts:141`; `src/main/bundle-spectral-ruleset.ts:278`, `:294` | rules/extends only; proto keys; https-only docs; private-host DNS check | P0 |
| 36 | Ruleset CRUD (upload/refresh/delete) | `src/routes/...{update,refresh,delete}-ruleset.tsx`; `spec-view.tsx:177-181` | per-project `.spectral.yaml`; Git projects use the repo file | P1 |
| 37 | Generate collection from spec | `src/routes/...spec.generate-request-collection.tsx:12-62` | reuses `scanResources` + `importResourcesToWorkspace` | P1 |
| 38 | Design empty-state templates | `src/ui/components/design-empty-state.tsx:10`; `example-openapi-specs.ts:1`, `:778`, `:920` | petstore / todo / blank | P2 |
| 39 | `inso run test` (unit tests + reporter) | `packages/insomnia-inso/src/cli.ts:354-493` | identifier may be suite/spec/workspace; `--ci` disables prompts | P0 |
| 40 | `inso run collection` (filters, iterations, overrides, output) | `cli.ts:494-890` | `--includeFullData` requires `--acceptRisk`; reporters; repeatable `-i` | P0 |
| 41 | `inso lint spec` | `cli.ts:892-958`; `commands/lint-specification.ts:83-143` | file or DB id; sibling `.spectral*` auto-detect; fails only on Error severity | P0 |
| 42 | `inso export spec` | `cli.ts:960-1004`; `commands/export-specification.ts:19-41` | `--skipAnnotations` strips `x-kong-*` recursively | P1 |
| 43 | `inso` data-source resolution (file/Git/NeDB) | `packages/insomnia-inso/src/db/index.ts:22-68` | v4/v5 export files only; Git adapter enforces folder-type match and `canSync` | P0 |
| 44 | `inso` `.insorc` config and `script` command | `cli.ts:73-93`, `:1008-1034` | only four global keys honoured; script must start with `inso` | P2 |
| 45 | Iteration data (JSON/CSV) | `cli.ts:207-246`; `iteration-data.test.ts` | quoted commas/quotes, CRLF, short rows | P1 |
| 46 | `inso` output redaction | `commands/run-collection/result-report.ts:38-230` | header/auth-whitelist/environment redaction | P1 |
| 47 | Cloud VCS three-way merge algorithms (reference) | `packages/insomnia-vcs/src/util.ts:67-500`; `vcs.ts:545` | snapshot state maps, conflict resolution records | P2 (out of scope; local-first) |
| 48 | Cloud VCS crypto and storage drivers (reference) | `packages/insomnia-vcs/src/crypt.ts:1-245`; `store/drivers/file-system-driver.ts:10-49` | RSA-OAEP-256/AES-GCM; atomic tmp+rename | P2 |
| 49 | Delta diff/patch (cloud snapshots) | `src/sync/delta/diff.ts:22`; `patch.ts:3` | only if cloud sync is ever added | P2 |
| 50 | `generate-docs` hidden command | `packages/insomnia-inso/src/cli.ts:1041-1044` | internal doc generation; not user-facing | P2 |
