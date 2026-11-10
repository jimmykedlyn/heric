# Data models

Source of truth: `D:\Projects\Heric\Insomnia\packages\insomnia-data\src\models\`. There is **no** `packages/insomnia/src/models/` directory (verified: path does not exist). All model definitions, type guards, `init()` functions and reference rewriting live in `insomnia-data`.

Every model is a plain JS object persisted in NeDB; the zod schemas in the same files are used only for schema-compatibility checks, never for validation on read/write (see [Serialization quirks](#serialization-quirks)).

## Model hierarchy

The canonical parent→child map is `getAllDescendantMap()` at `packages/insomnia-data/src/models/index.ts:75-131`:

| Parent | Descendants (in map order) | Cite |
|---|---|---|
| `Project` | `Workspace` | `models/index.ts:77` |
| `Workspace` | `RequestGroup`, `Request`, `GrpcRequest`, `WebSocketRequest`, `SocketIORequest`, `CookieJar`, `Environment`, `ApiSpec`, `MockServer`, `UnitTestSuite`, `ProtoDirectory`, `ProtoFile`, `WorkspaceMeta`, `RunnerTestResult`, `CaCertificate`, `ClientCertificate`, `McpRequest` | `models/index.ts:78-96` |
| `RequestGroup` | `RequestGroup`, `Request`, `GrpcRequest`, `WebSocketRequest`, `SocketIORequest`, `RunnerTestResult`, `RequestGroupMeta`, `OAuth2Token` | `models/index.ts:97-106` |
| `Request` | `RequestMeta`, `Response`, `RequestVersion`, `OAuth2Token` | `models/index.ts:107-112` |
| `GrpcRequest` | `GrpcRequestMeta` | `models/index.ts:113` |
| `WebSocketRequest` | `WebSocketPayload`, `WebSocketResponse`, `RequestMeta` | `models/index.ts:114-118` |
| `SocketIORequest` | `SocketIOPayload`, `SocketIOResponse`, `RequestMeta` | `models/index.ts:119-123` |
| `McpRequest` | `McpPayload`, `McpResponse` | `models/index.ts:124` |
| `MockServer` | `MockRoute` | `models/index.ts:125` |
| `Environment` | `Environment` (sub-environments nest under the base env) | `models/index.ts:126` |
| `UnitTestSuite` | `UnitTest`, `UnitTestResult` | `models/index.ts:127` |
| `UnitTest` | `UnitTestResult` | `models/index.ts:128` |
| `ProtoDirectory` | `ProtoDirectory`, `ProtoFile` | `models/index.ts:129` |

Notes:
- `Settings`, `Stats`, `UserSession` have **no** parent (root singletons) — asserted in `models/base-types.ts:55-57` and absent from the descendant map.
- `Project` has no parent; `Workspace.parentId` is the owning project id (optional only for the scratchpad) — `models/base-types.ts:55-58`, `models/workspace.ts:12`.
- `ProjectLintRuleset`, `GitRepository`, `GitCredentials`, `CloudCredential`, `PluginData`, `OAuth2Token` are also absent from the map, but `ProjectLintRuleset.parentId` is a project id (`node-src/services/project-lint-ruleset.ts:6-7`), and `OAuth2Token.parentId` is a request/group id (`models/index.ts:105,111`).
- The map is expanded into a child→parent reverse map and a `generateDescendantMap(queryTypes)` used for partial deletes/duplication (`models/index.ts:133-177`).

### Base model

`baseModelSchema` (`models/base-types.ts:52-63`):

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `_id` | `string` | yes | generated at insert (`init-model/index.ts:37-39`) | `${prefix}_${uuidv4-without-dashes}` (`common-src/misc.ts:8-14`) |
| `type` | `AllTypes` enum (44 values) | yes | — | `models/base-types.ts:3-48` |
| `parentId` | `string` | yes (declared) | `null` in `initModel` defaults (`init-model/index.ts:23-33`) | schema says string but runtime default is `null`; several models redeclare it |
| `modified` | `number` | yes | `Date.now()` at init/update | `init-model/index.ts:29`, `database-nedb.ts:140` |
| `created` | `number` | yes | `Date.now()` at init | `init-model/index.ts:30` |
| `isPrivate` | `boolean` | yes | **no runtime default**: only present if the model's `init()` sets it (Request, GrpcRequest, CaCertificate, Environment, ClientCertificate) or a caller patches it; the zod `.default(false)` at `base-types.ts:70` never runs | gates sync (`models/index.ts:33-45`) |
| `name` | `string` | yes | `''` (schema default `base-types.ts:71`) | models override with their own `init()` |

`createModelSchema(type, prefix)` (`models/base-types.ts:67-73`) narrows `type` to a literal and makes `isPrivate`/`name` optional-with-default. **The `prefix` argument is unused** — ids are generated from the exported `prefix` constant by `initModel`, not from this argument.

44 types: `ApiSpec, CaCertificate, ClientCertificate, CloudCredential, CookieJar, Environment, GitCredentials, GitRepository, GrpcRequest, GrpcRequestMeta, MockRoute, MockServer, OAuth2Token, PluginData, Project, ProjectLintRuleset, ProtoDirectory, ProtoFile, Request, RequestGroup, RequestGroupMeta, RequestMeta, RequestVersion, Response, RunnerTestResult, Settings, SocketIOPayload, SocketIORequest, SocketIOResponse, SocketIORequestMeta, Stats, UnitTest, UnitTestResult, UnitTestSuite, UserSession, WebSocketPayload, WebSocketRequest, WebSocketResponse, WebSocketRequestMeta, McpRequest, McpResponse, McpPayload, Workspace, WorkspaceMeta` (`models/base-types.ts:3-48`). `db-models.ts:1-44` exports one namespace per model; `models/index.ts:26-28` derives `all()` / `types()` from it.

### Persistence layer

- Each type is a separate NeDB file `insomnia.<Type>.db` under `INSOMNIA_DATA_PATH` (or temp) — `node-src/database/database-nedb.ts:242-434`. 44 collections, e.g. `ApiSpec` at `:258-261`, `Workspace` at `:428`, `WorkspaceMeta` at `:430-433`.
- **No secondary indexes exist.** A repo-wide grep for `ensureIndex` / `createIndex` / `.index(` returns nothing; lookups rely on NeDB `_id` and full scans filtered by `parentId` (e.g. `database-nedb.ts:588`).
- Default sort when unspecified is `{ created: 1 }` for both `find` and `findOne` (`database-nedb.ts:205-234`).
- `find`/`findOne`/`insert`/`update` all run `initModel()` on the raw doc before returning, so migrations run on every read (`database-nedb.ts:214, 231, 444, 514`).
- Init defaults are `{_id:null, type, parentId:null, modified:Date.now(), created:Date.now()}` merged with `model.init()` then caller patches (`init-model/index.ts:22-34`). Keys not present in that merged object and not listed in `model.optionalKeys` are **deleted** (`init-model/index.ts:70-78`).
- `docCreate` forces `type` after patches (`database-nedb.ts:120-130`); `docUpdate` re-inits the original doc and restamps `modified` unless the patch carries one (`database-nedb.ts:132-145`).
- `duplicate` walks the descendant map, allocates a new prefixed id per doc, calls `models.rewriteReferences` on every doc, and re-parents children to the new ids (`database-nedb.ts:147-204`). Only child types with `canDuplicate === true` are copied (`database-nedb.ts:160`).
- `remove`/`removeWhere` delete a doc plus all descendants; `unsafeRemove` deletes only the doc (`database-nedb.ts:454-511`).

## Per-model reference

Field tables list model-specific fields only; every model also has the six base fields. "Required" reflects the zod schema where one exists, otherwise the TypeScript interface.

### Workspace
`packages/insomnia-data/src/models/workspace.ts`

Metadata: name `Workspace`, type `Workspace`, prefix `wrk`, `canDuplicate: true`, `canSync: true` (`:6-10`). Scratchpad id `wrk_scratchpad` (`:12`).

| Field | Type | Default / init | Required |
|---|---|---|---|
| `name` | string | `New API Collection` (`` `New ${strings.collection.singular}` ``, `:25,54`; string value `common-src/strings.ts:20-23`) | optional w/ default |
| `description` | string | `''` (`:26,55`) | optional w/ default |
| `certificates` | any (deprecated) | absent | optional (`:27`) |
| `scope` | `'design' \| 'collection' \| 'mock-server' \| 'environment' \| 'mcp'` (`:16-22,28`) | `'collection'` (`:56`) | required |
| `konnectServiceId` | string \| null | absent | optional, listed in `optionalKeys` (`:29,51`) |

Relations: parent of everything in the workspace row of the hierarchy table; parent is `Project` (`node-src/services/workspace.ts:16-20` enforces the prefix). `workspace-meta` child is created lazily, not by workspace creation.
Indexes: none.
Init: `models/workspace.ts:53-57`. Helpers: `isWorkspace` `:36`, `isWorkspaceId` `:38`, `isDesign/isCollection/isMockServer/isEnvironment/isMcp` `:40-49`, `isScratchpad` `:59-61`, `scopeToActivity` `:63-85` (design→`debug`, collection→`debug`, mockServer→`mock-server`, environment→`environment`, mcp→`mcp`).

### WorkspaceMeta
`models/workspace-meta.ts`

Metadata: type `WorkspaceMeta`, prefix `wrkm`, `canDuplicate: false`, `canSync: false` (`:3-7`).

| Field | Type | Default | Required |
|---|---|---|---|
| `activeActivity` | string \| null | null | yes |
| `activeEnvironmentId` | string \| null | null | yes |
| `activeGlobalEnvironmentId` | string \| null | null | yes |
| `activeRequestId` | string \| null | null | yes |
| `activeUnitTestSuiteId` | string \| null | null | yes |
| `gitRepositoryId` | string \| null | null | yes |
| `parentId` | string \| null | null | yes (overrides base type) |
| `pushSnapshotOnInitialize` | boolean | false | yes |
| `hasUncommittedChanges` | boolean | false | yes |
| `hasUnpushedChanges` | boolean | false | yes |
| `gitFilePath` | string \| null | null | yes |
| `gitFileLastSyncTime` | number \| null | null | yes |

(interface `:9-22`, init `:39-54`.) Relations: one per workspace (`node-src/services/workspace-meta.ts:29-36` `getOrCreateByParentId`). `rewriteReferences` remaps `activeEnvironmentId`, `activeGlobalEnvironmentId`, `activeRequestId`, `activeUnitTestSuiteId`, preserving unknown/null ids (`:28-37`, test `workspace-meta.test.ts:21-56`).

### Project
`models/project.ts`

Metadata: type `Project`, prefix `proj`, `canDuplicate: false`, `canSync: false` (`:6-10`). Constants: `proj_scratchpad` (`:12`), `EMPTY_GIT_PROJECT_ID = 'empty'` (`:15`), protected repo prefix `gr_` vs real `git_` (`:21-22`).

| Field | Type | Default | Notes |
|---|---|---|---|
| `remoteId` | `string \| null` | `null` (`:126`) | non-null ⇒ remote (cloud) project |
| `gitRepositoryId` | `string \| null` | `null` (`:127`) | `'empty'` ⇒ git project not yet connected; `gr_xxx` ⇒ protected encoding of `git_xxx` (`:21-70`) |
| `mcpStdioAccess` | boolean? | `false` (`:128`) | |
| `konnectControlPlaneId` | string \| null? | absent | `optionalKeys` `:121` |
| `konnectClusterType` | string \| null? | absent | `optionalKeys` `:121` |
| `konnectDeploymentType` | `'selfManaged'\|'serverless'\|'dedicatedCloud'\|'group'\|'k8sIngressController'` \| null? | absent | type `:89` |
| `konnectRegion` | string \| null? | absent | `optionalKeys` `:121` |

Relations: parent of workspaces. Type guards `:80-87`; id codec `decodeRepoId`/`toProtectedRepoId`/`getQueryableGitRepositoryIds` `:28-70`; `getEffectiveRepoId` `:43-47`; sort order default-org projects first then alphabetical (`:142-145`, test `project.test.ts:10-17`); storage-type resolution `:147-198`; label builder `:200-215`. Init `:123-130`; `createId` `:132-134`.

### Request
`models/request.ts`

Metadata: type `Request`, prefix `req`, `canDuplicate: true`, `canSync: true` (`:30-38`).

| Field | Type | Default | Required |
|---|---|---|---|
| `url` | string | `''` (`:370`) | optional w/ default |
| `name` | string | `''` schema, `New Request` init (`:371`, `:433`) | optional w/ default |
| `description` | string | `''` (`:372`) | optional w/ default |
| `method` | string | schema required; init `GET` (`:373`, `:435`) | required |
| `body` | `{mimeType?: string\|null, text?: string, fileName?: string, params?: RequestBodyParameter[]}` (`:352-358`) | `{}` (`:374`) | optional w/ default `{}` |
| `parameters` | `RequestParameter[]` (`:258-268`) | `[]` (`:375`) | optional w/ default `[]` |
| `headers` | `RequestHeader[]` (`:248-256`) | `[]` (`:376`) | optional w/ default `[]` |
| `authentication` | `RequestAuthentication` union or `{}` (`:214-246`) | `{}` (`:377`) | optional w/ default `{}` |
| `preRequestScript` | string? | `undefined` (`:440`) | optional |
| `afterResponseScript` | string? | `undefined` (`:444`) | optional |
| `metaSortKey` | number | `-Date.now()` (`:441`) | required |
| `pathParameters` | `{name,value}[]` (`:282-288`) | `undefined` (`:443`) | optional |
| `disableUserAgentHeader` | boolean? | absent | optional, `optionalKeys` `:424-428` |
| `konnectRouteKey` | string \| null? | absent | optional |
| `konnectManagedHeaderNames` | string[] \| null? | absent | optional |
| `settingStoreCookies` | boolean | `true` (`:362`) | optional w/ default |
| `settingSendCookies` | boolean | `true` (`:363`) | optional w/ default |
| `settingDisableRenderRequestBody` | boolean | `false` (`:364`) | optional w/ default |
| `settingEncodeUrl` | boolean | `true` (`:365`) | optional w/ default |
| `settingRebuildPath` | boolean | `true` (`:366`) | optional w/ default |
| `settingFollowRedirects` | `'global'\|'on'\|'off'` | `'global'` (`:367`) | optional w/ default |

Auth variants (`type` discriminator, all fields optional unless noted): `basic` (`useISO88591`; `:45-51`), `apikey` (`key`,`value`,`addTo`; `:57-63`), `oauth2` (`grantType` required from `authorization_code|client_credentials|password|implicit|refresh_token|mcp_auth_flow`; `accessTokenUrl`,`authorizationUrl`,`clientId`,`clientSecret`,`clientIdIssuedAt`,`clientSecretExpiresAt`,`audience`,`scope`,`resource`,`username`,`password`,`redirectUrl`,`useDefaultBrowser`,`credentialsInBody`,`state`,`code`,`accessToken`,`refreshToken`,`tokenPrefix`,`usePkce`,`pkceMethod`,`responseType`,`launchBrowserManually`,`origin`; `:70-108`), `hawk` (`id`,`key`,`algorithm` required; `:111-119`), `oauth1` (`signatureMethod` enum HMAC-SHA1/HMAC-SHA256/RSA-SHA1/PLAINTEXT; `:122-145`), `digest` (`:148-153`), `ntlm` (`:156-161`), `bearer` (`token`,`prefix`; `:164-169`), `iam` (`accessKeyId`,`secretAccessKey`,`sessionToken`,`region`,`service`; `:172-180`), `netrc` (`:183-186`), `asap` (`issuer`,`audience`,`privateKey`,`keyId` required; `:189-198`), `none` (`:201-204`), `singleToken` (`token`; `:207-211`). Union falls back to `z.object({})` so an arbitrary/old auth block is valid (`:229-246`).

Helpers: path-parameter regex `/\/:[^/?#:]+/g` (`:290`); `applyPathParametersToUrl` (`:293-305`) URL-encodes matched values, leaves unmatched/empty segments untouched; `getPathParametersFromUrl` dedupes (`:307-317`); `getCombinedPathParametersFromUrl` merges saved + unsaved (empty-value) params (`:319-350`); `getOperationType` parses GraphQL when `body.mimeType === 'application/graphql'` (`:402-420`); `isEventStreamRequest` when an `Accept: text/event-stream` header exists (`:399-400`); `isGraphqlSubscriptionRequest` (`:421-422`).
Relations: child of workspace/group; parent of request-meta, responses, request versions, oauth tokens (hierarchy table).
Init `:430-453`; `rewriteReferences` string-substitutes ids in `url`,`body`,`parameters`,`pathParameters`,`headers`,`authentication`,`preRequestScript`,`afterResponseScript` and nulls `konnectRouteKey` (`:455-474`).

### RequestGroup (Folder)
`models/request-group.ts`

Metadata shows as `name = 'Folder'`, type `RequestGroup`, prefix `fld`, duplicate/sync true (`:8-16`).

| Field | Type | Default | Required |
|---|---|---|---|
| `name` | string | `''` schema, `New Folder` init (`:25`, `:48`) | optional w/ default |
| `description` | string | `''` (`:26`) | optional w/ default |
| `authentication` | `RequestAuthentication`? | `undefined` init (`:54`) | optional |
| `environment` | `Record<string,any>` | `{}` (`:28,50`) | optional w/ default |
| `environmentPropertyOrder` | Record \| null? | absent | optional (`optionalKeys` `:18-23`) |
| `headers` | `RequestHeader[]`? | `undefined` init (`:55`) | optional |
| `kvPairData` | `EnvironmentKvPairData[]`? | absent | optional; e.g. env pairs for a folder-level environment |
| `environmentType` | `'json' \| 'kv'`? | absent | optional |
| `metaSortKey` | number | `-Date.now()` (`:51`) | required |
| `preRequestScript` | string? | `undefined` (`:52`) | optional |
| `afterResponseScript` | string? | `undefined` (`:53`) | optional |
| `konnectRouteId` | string \| null? | absent | optional; **always set to `null` on duplicate** (`:67`) |

Init `:46-57`; `rewriteReferences` substitutes ids in `authentication`,`headers`,`preRequestScript`,`afterResponseScript`,`environment`,`kvPairData` and nulls `konnectRouteId` (`:59-69`). `isRequestGroupId` uses `startsWith(prefix)` **without underscore** (`:44`) — unlike almost every other id guard (`req_`, `wrk_`, …), so any id starting `fld` matches.

### RequestMeta
`models/request-meta.ts`

Metadata: type `RequestMeta`, prefix `reqm`, duplicate/sync false (`:5-9`). `RequestAccordionKeys = 'OAuth2AdvancedOptions'` (`:11`).

| Field | Type | Default |
|---|---|---|
| `parentId` | string | null (`:34`) |
| `previewMode` | `'friendly'\|'source'\|'raw'` (`common-src/preview-mode.ts:12`) | `'friendly'` (`:35`) |
| `responseFilter` | string | `''` (`:36`) |
| `responseFilterHistory` | string[] | `[]` (`:37`) |
| `activeResponseId` | string \| null | null (`:38`) |
| `savedRequestBody` | Record<string,any> | `{}` (`:39`) |
| `pinned` | boolean | false (`:40`) |
| `lastActive` | number | 0 (`:41`) |
| `downloadPath` | string \| null | null (`:42`) |
| `expandedAccordionKeys` | Partial<Record<RequestAccordionKeys,boolean>> | `{}` (`:43`) |
| `activeMcpPrimitive` | string \| null? | null (`:44`) |
| `streamSummaryPath` | string \| null | null (`:45`) |
| `streamSummaryRenderMarkdown` | boolean | false (`:46`) |

Shared by HTTP, WebSocket and Socket.IO requests (hierarchy table); gRPC uses `GrpcRequestMeta`. Service auto-creates per parent (`node-src/services/request-meta.ts:22-45`).

### RequestGroupMeta
`models/request-group-meta.ts` — type `RequestGroupMeta`, prefix `fldm`, duplicate/sync false; single field `collapsed: boolean` default `false`; init also sets `parentId: null` (`:13-25`).

### Environment
`models/environment.ts`

Metadata: type `Environment`, prefix `env`, pair prefix `envPair`, duplicate/sync true (`:5-15`). Vault constants: `__insomnia_vault` (storage), `vault` (runtime), mask `'••••••'` (`:10-13`).

| Field | Type | Default | Required |
|---|---|---|---|
| `name` | string | `''` schema, `New Environment` init (`:42`, `:67`) | optional w/ default |
| `data` | Record<string,any> | `{}` (`:43`) | optional w/ default |
| `color` | string \| null? | null (`:69`) | optional |
| `dataPropertyOrder` | Record \| null? | absent | optional (`optionalKeys` `:17`) |
| `kvPairData` | `EnvironmentKvPairData[]`? | absent | optional |
| `metaSortKey` | number | `Date.now()` (`:47-50`) | optional w/ default `Date.now()` |
| `environmentType` | `'json' \| 'kv'`? | absent | optional |

`EnvironmentKvPairData` (`:30-38`): `id` (required string), `name` (`''`), `value` (`''`), `type` (`'json'|'str'|'secret'` required), `enabled?` boolean, `isConfidential?` boolean. Init `:65-72` sets `isPrivate: false` explicitly. Relations: workspace (base env) and environment (sub-env), circular nesting allowed. Deterministic base-env id `env_<sha1(parentId)>` from `getOrCreateForParentId` (`node-src/services/environment.ts:76-105`), with KV type default (`:88`), name `Base Environment` (`:86`). `listByParentId` sorts by `metaSortKey` (`:65-74`); `duplicate` computes midpoint sort key and appends ` (Copy)` (`:115-131`); `removeAllSecrets` strips `__insomnia_vault` and secret kv entries for private envs (`:36-63`).

### CookieJar
`models/cookie-jar.ts`

Metadata: type `CookieJar`, prefix `jar`, `canDuplicate: true`, `canSync: false` (`:6-14`).

| Field | Type | Default |
|---|---|---|
| `name` | string | `''` schema; `Default Jar` init (`:59`, `:71`) |
| `cookies` | `Cookie[]` | `[]` (`:60,72`) |

`Cookie` (`:16-56`): `id` (default `crypto.randomUUID()`, `:17-20`), `key` `''`, `value` `''`, `expires` (Date | `'Infinity'` | null; `'Infinity'` preprocessed to `null`, invalid Dates to `null`, `:23-40`), `domain` `''`, `path` `'/'`, `secure` false, `httpOnly` false, `extensions` JSON[]?, `creation` coerced Date?, `creationIndex` number?, `hostOnly` boolean?, `pathIsDefault` boolean?, `lastAccessed` coerced Date?, `source` `'manual' | 'response'?` (`:54`; `manual` cookies may contain template syntax, `response` cookies are literal — comment `:51-53`).
Relations: child of workspace. Deterministic id `jar_<sha1(parentId)>` from `getOrCreateForParentId` (`node-src/services/cookie-jar.ts:16-28`).

### Response
`models/response.ts`

Metadata: type `Response`, prefix `res`, duplicate/sync false (`:6-14`).

| Field | Type | Default | Notes |
|---|---|---|---|
| `environmentId` | string \| null | `'__LEGACY__'` (`:85`) | sentinel so pre-env-filter responses are hidden when `filterResponsesByEnv` on (`:83-85`) |
| `globalEnvironmentId` | string \| null | null (`:87`) | |
| `statusCode` | number | 0 | |
| `statusMessage` | string | `''` | |
| `httpVersion` | string | `''` | |
| `contentType` | string | `''` | |
| `url` | string | `''` | |
| `bytesRead` | number | 0 | |
| `bytesContent` | number | `-1` (`:69`) | `-1` = legacy field absent |
| `elapsedTime` | number | 0 | |
| `headers` | `{name,value}[]` (`:16-19`) | `[]` | |
| `bodyPath` | string | `''` | body stored on filesystem |
| `bodyBuffer` | Uint8Array? | absent | "if body is less than 5MB, it's stored in memory" (`:36-37`); no code enforcing 5MB was found in this package |
| `timelinePath` | string | `''` | timeline NDJSON on filesystem |
| `bodyCompression` | `'zip'\|null\|'__NEEDS_MIGRATION__'\|undefined` (`:21`) | `'__NEEDS_MIGRATION__'` (`:77`) | migrated to `'zip'` on read (`init-model/response.ts:12-18`) |
| `error` | string | `''` | |
| `requestVersionId` | string \| null | null | |
| `settingStoreCookies` | boolean \| null | null | |
| `settingSendCookies` | boolean \| null | null | |
| `requestTestResults` | `RequestTestResult[]` (`runner-test-result.ts:16-22`) | `[]` | |

Init `:60-88`. Creation policy: max 20 retained per request (or per env when filtering), older pruned, and a `RequestVersion` snapshot is created before insert (`node-src/services/response.ts:45-79`; `maxResponses = 20` also mirrors `Settings.maxHistoryResponses` default 20, `models/settings.ts:62`). Deleting responses unlinks `bodyPath` and `timelinePath` files (`node-src/services/helpers/response-operations.ts:51-56`); `getResponseBodyBuffer` gunzips when `bodyCompression === 'zip'`, returns empty buffer when `bodyPath` empty (`:186-203`).

### ApiSpec
`models/api-spec.ts`

Metadata: type `ApiSpec`, prefix `spc`, duplicate/sync true (`:6-14`).

| Field | Type | Default | Required |
|---|---|---|---|
| `fileName` | string | `New API Collection` (`` `New ${strings.document.singular}` `` `:30`; `strings.ts:28-31`) | required |
| `contentType` | `'json' \| 'yaml'` | `'yaml'` (`:32`) | required |
| `contents` | string | `''` (`:19,31`) | optional w/ default |

One per design workspace; `getOrCreateForParentId` (`node-src/services/api-spec.ts:10-20`); startup repair renames default filenames to the workspace name without restamping `modified` when unnecessary (`node-src/database/repair-database.ts:35-45`). Init `:28-34`.

### RequestVersion
`models/request-version.ts`

Metadata: type `RequestVersion`, prefix `rvr`, duplicate/sync false (`:8-16`). Single field `compressedRequest: string | null`, default `null` (`:18-29`). Semantics: snapshot of a request at send time (comment `:3-6`).
Service: gzip + base64 encode (`node-src/services/request-version.ts:41-44`), `decompressObject` (`:46-53`), `create` only writes a new version when the request changed, comparing all fields except `_id,type,created,modified,metaSortKey,description,parentId,name` (`:22-31,61-93`); reuses latest otherwise; `restore` re-applies the snapshot minus ignored fields (`:95-125`).

### UnitTestSuite
`models/unit-test-suite.ts` — type `UnitTestSuite`, prefix `uts`, duplicate/sync true (`:7-13`). Fields `name` (default `''`, init `My Test`) and `metaSortKey` (`-Date.now()`) (`:15-30`). Child of workspace; parent of `UnitTest` and `UnitTestResult`.

### UnitTest
`models/unit-test.ts`

Metadata: type `UnitTest`, prefix `ut`, duplicate/sync true (`:7-13`).

| Field | Type | Default |
|---|---|---|
| `name` | string | `''` schema, `My Test` init (`:16`, `:31`) |
| `requestId` | string \| null | `null` (`:17,30`) |
| `code` | string | `''` (`:18,32`) |
| `metaSortKey` | number | `-Date.now()` (`:19,33`) |

`rewriteReferences` remaps `requestId` via the id mapping (`:37-42`). Child of `UnitTestSuite`; parent of `UnitTestResult`.

### UnitTestResult
`models/unit-test-result.ts` — type `UnitTestResult`, prefix `utr`, duplicate/sync false (`:5-13`). Field `results: TestResults` where `TestResults = {failures,passes,pending,tests: TestResult[], stats: mocha Stats}` and `TestResult` has `id,title,fullTitle,file?,duration?,currentRetry,err` (`:15-49`). **Init sets `results: null`** despite the non-null type (`:55-59`) — consumers must tolerate null.

### MockServer
`models/mock-server.ts`

Metadata: type `MockServer`, prefix `mock`, duplicate/sync true (`:5-13`).

| Field | Type | Default |
|---|---|---|
| `parentId` | string | `''` (redeclared, `:16`, `:28`) |
| `name` | string | `''` schema, `New Mock` init (`:17,30`) |
| `url` | string | `http://localhost:8080` (`:18,31`) |
| `useInsomniaCloud` | boolean | `true` (`:19,32`) |

Child of a `mock-server`-scope workspace; one per workspace (`getOrCreateForParentId`, `node-src/services/mock-server.ts:15-25`); `findByProjectId` resolves via workspace list (`:38-41`).

### MockRoute
`models/mock-route.ts`

Metadata: type `MockRoute`, prefix `mock-route`, duplicate/sync true (`:6-14`).

| Field | Type | Default |
|---|---|---|
| `name` | string | `'/'` (`:17,40`) |
| `body` | string | `''` (`:18`) |
| `headers` | RequestHeader[] | `[]` (`:19`) |
| `method` | string | `'GET'` (`:21`) |
| `mimeType` | string | `application/json` (`:23`) |
| `statusCode` | number | `200` (`:24`) |
| `statusText` | string | `''` (`:25`) |
| `parentId` | string | `''` (`:26,37`) |

Init `:33-44`. Child of `MockServer`.

### gRPC request
`models/grpc-request.ts`

Metadata: type `GrpcRequest`, prefix `greq`, duplicate/sync true (`:6-10`).

| Field | Type | Default | Required |
|---|---|---|---|
| `url` | string | `''` (`:26`) | optional w/ default |
| `name` | string | `New gRPC Request` (`:71`) | optional w/ default |
| `description` | string | `''` (`:72`) | required (no default in schema, `:28`) |
| `protoFileId` | string? | `''` (`:73`) | optional |
| `protoMethodName` | string? | `''` (`:74`) | optional |
| `body` | `{text?: string}` | `{ text: '{}' }` (`:76-78`) | required object (`:31`) |
| `metadata` | `{name,value,description?,disabled?}[]` | `[]` (`:75`) | required |
| `metaSortKey` | number | `-Date.now()` (`:79`) | required |
| `reflectionApi` | `{enabled:boolean, url:string, apiKey:string, module:string}` | `{enabled:false, url:'https://buf.build', apiKey:'', module:'buf.build/connectrpc/eliza'}` (`:81-86`) | required |
| `disableUserAgentHeader` | boolean? | absent | optional (`optionalKeys` `:53-57`) |
| `konnectRouteKey` | string \| null? | absent | optional |
| `konnectManagedHeaderNames` | string[] \| null? | absent | optional |

Init `:68-87`. `rewriteReferences` remaps `protoFileId`, then `url`/`body`/`metadata` id substitution, nulls `konnectRouteKey` (`:59-66`). Child of workspace/group; parent of `GrpcRequestMeta`.

### GrpcRequestMeta
`models/grpc-request-meta.ts` — type `GrpcRequestMeta`, prefix `greqm`, duplicate/sync false; `pinned: false`, `lastActive: 0` (`:13-27`).

### WebSocket request
`models/websocket-request.ts`

Metadata: type `WebSocketRequest`, prefix `ws-req`, duplicate/sync true (`:7-15`).

| Field | Type | Default | Required |
|---|---|---|---|
| `url` | string | `''` (`:26`) | optional w/ default |
| `name` | string | `New WebSocket Request` (`:52`) | optional w/ default |
| `description` | string | `''` (`:63`) | optional w/ default |
| `metaSortKey` | number | `-Date.now()` (`:54`) | required |
| `authentication` | auth union | `{}` (`:56`) | optional w/ default |
| `headers` | RequestHeader[] | `[]` (`:55`) | optional w/ default |
| `parameters` | RequestParameter[] | `[]` (`:57`) | optional w/ default |
| `pathParameters` | `{name,value}[]?` | `undefined` (`:58`) | optional |
| `disableUserAgentHeader` | boolean? | absent | optional (`optionalKeys` `:67-72`) |
| `konnectRouteKey` | string \| null? | absent | optional |
| `konnectManagedHeaderNames` | string[] \| null? | absent | optional |
| `settingStoreCookies` | boolean | `true` (`:19,60`) | optional w/ default |
| `settingSendCookies` | boolean | `true` (`:20,61`) | optional w/ default |
| `settingEncodeUrl` | boolean | `true` (`:21,59`) | optional w/ default |
| `settingFollowRedirects` | `'global'\|'on'\|'off'` | `'global'` (`:22,62`) | optional w/ default |
| `settingUseProxy` | boolean? | absent (optional key only, `:23,68`) | optional |

Init `:51-64`; `rewriteReferences` substitutes ids in `url`,`headers`,`authentication`,`parameters`,`pathParameters`, nulls `konnectRouteKey` (`:74-80`).

### WebSocketRequestMeta
`models/websocket-request-meta.ts` — type `WebSocketRequestMeta`, prefix `ws-req-meta`, duplicate/sync false; `pinned: false` (`:13-26`).

### WebSocketPayload
`models/websocket-payload.ts` — type `WebSocketPayload`, prefix `ws-payload`, duplicate/sync **true** (`:4-12`). Fields `name` (`New Payload`), `value` (`''`), `mode` (`application/json`) (`:14-30`). `rewriteReferences` substitutes ids in `value` (`:32-34`). `getOrCreateByParentId`-style helpers exist as `create`/`getByParentId` in `node-src/services/websocket-payload.ts:6-36`.

### WebSocketResponse
`models/websocket-response.ts`

Metadata: type `WebSocketResponse`, prefix `ws-res`, duplicate/sync false (`:4-12`). Fields (`:14-31`): `environmentId` (null), `statusCode` 0, `statusMessage` `''`, `httpVersion` `''`, `contentType` `''`, `url` `''`, `elapsedTime` 0, `headers` `[]`, `eventLogPath` `''` (event log on filesystem), `timelinePath` `''`, `error` `''`, `requestVersionId` null, `settingStoreCookies` null, `settingSendCookies` null. Init `:37-54`. No body/timeline content in the doc.

### Socket.IO request
`models/socket-io-request.ts`

Metadata: type `SocketIORequest`, prefix `socketio-req`, duplicate/sync true (`:7-15`).

| Field | Type | Default |
|---|---|---|
| `url` | string | `''` (`:33`) |
| `name` | string | `New Socket.IO Request` (`:59`) |
| `description` | string | `''` (`:70`) |
| `metaSortKey` | number | `-Date.now()` (`:61`) |
| `authentication` | auth union | `{}` (`:63`) |
| `headers` | RequestHeader[] | `[]` (`:62`) |
| `parameters` | RequestParameter[] | `[]` (`:64`) |
| `pathParameters` | `{name,value}[]?` | `undefined` (`:65`) |
| `eventListeners` | `SocketIOEventListener[]` | `[]` (`:71`) |
| `disableUserAgentHeader` | boolean? | absent (`optionalKeys` `:74`) |
| `settingEncodeUrl` | boolean | `true` (`:27,66`) |
| `settingStoreCookies` | boolean | `true` (`:28,67`) |
| `settingSendCookies` | boolean | `true` (`:29,68`) |
| `settingPath` | string? | `undefined` (`:30,69`) |

`SocketIOEventListener` (`:17-23`): `id` required, `eventName` `''`, `desc` `''`, `isOpen` false. Init `:58-72`; `rewriteReferences` (`:76-81`) does not null any konnect field (Socket.IO has no konnect fields).

### SocketIORequestMeta
`models/socket-io-request-meta.ts` — type `SocketIORequestMeta`, prefix `socketio-req-meta`, duplicate/sync false; `pinned: false` (`:13-26`).

### SocketIOPayload
`models/socket-io-payload.ts` — type `SocketIOPayload`, prefix `socket-io-payload`, duplicate/sync **true** (`:7-15`). Fields (`:17-29`): `args: {id,value,mode}[]` init `[{ id: uuidv4(), value: '', mode: 'application/json' }]`, `eventName?` (`''`), `ack?` (`false`). Init `:35-41`; `rewriteReferences` substitutes ids in `args` (`:43-45`).

### SocketIOResponse
`models/socket-io-response.ts` — type `SocketIOResponse`, prefix `socketIO-res`, duplicate/sync false (`:3-11`). Fields (`:13-23`): `eventLogPath` `''`, `timelinePath` `''`, `requestVersionId` null, `environmentId` null, `elapsedTime` 0, `error` `''`, `url` `''`. Init `:29-38`.

### MCP request
`models/mcp-request.ts`

Metadata: type `McpRequest`, prefix `mcp-req`, duplicate/sync true (`:7-11`). Transports `stdio` and `streamable-http`, default HTTP (`:13-18,37`).

| Field | Type | Default |
|---|---|---|
| `url` | string | `''` (`:35,62`) |
| `description` | string | `''` (`:36`) |
| `transportType` | `'stdio' \| 'streamable-http'` | `'streamable-http'` (`:37,63`) |
| `headers` | RequestHeader[] | `[]` (`:38,65`) |
| `authentication` | auth union | `{}` (`:39,66`) |
| `env` | `EnvironmentKvPairData[]` | `[]` (`:40,67`) |
| `roots` | `{name?, uri}[]` (`:20-24`) | `[]` (`:41,69`) |
| `mcpStdioAccess` | boolean | false (`:27,68`) |
| `connected` | boolean | false (`:28,71`) |
| `subscribeResources` | string[] | `[]` (`:29,70`) |
| `sslValidation` | boolean | true (`:31,72`) |
| `disableUserAgentHeader` | boolean? | absent (`optionalKeys` `:76`) |

`McpServerPrimitiveTypes = 'tools'|'resources'|'prompts'|'resourceTemplates'` (`:49`). `name` comes only from the base model (no own default beyond `''`). Init `:60-74`. `clearResourceSubscriptions` (`node-src/services/mcp-request.ts:39-43`).

### McpPayload
`models/mcp-payload.ts` — type `McpPayload`, prefix `mcp-payload`, duplicate **true**, sync **false** (`:3-11`). Fields `params?` (string | Record, init `{}`), `url` (init `''`) (`:13-29`).

### McpResponse
`models/mcp-response.ts` — type `McpResponse`, prefix `mcp-response`, duplicate/sync false (`:5-9`). Fields (`:11-29`): `environmentId` null, `status` `''` (STDIO only), `statusCode` 0 (HTTP only), `statusMessage` `''`, `url` `''`, `elapsedTime` 0, `headers` `[]`, `eventLogPath` `''`, `timelinePath` `''`, `error` `''`, `errorType?` (`''`), `requestVersionId` null, `transportType` default `'streamable-http'` (`:49`). Init `:35-50`.

### ProtoFile / ProtoDirectory
`models/proto-file.ts`: type `ProtoFile`, prefix `pf`, duplicate/sync true; `name` `New Proto File`, `protoText` `''` (`:3-26`).
`models/proto-directory.ts`: type `ProtoDirectory`, prefix `pd`, duplicate/sync true; `name` `New Proto Directory`; `createId()` uses `generateId(prefix)` (`:3-31`). Directories nest under directories and hold proto files (`models/index.ts:129`).

### CA certificate / Client certificate
`models/ca-certificate.ts`: type `CaCertificate`, prefix `crt`, duplicate true, sync false (`:4-12`). Fields `parentId` (`''`), `path` (string|null, default null), `disabled` (false); init also sets `isPrivate:false` (`:14-31`).
`models/client-certificate.ts`: type `ClientCertificate`, **same prefix `crt`** (`:8`), duplicate true, sync false. Fields (`:15-25`): `parentId` `''`, `host` `''`, `passphrase` null, `cert` null, `key` null, `pfx` null, `disabled` false, `isPrivate` false. `filterClientCertificates` prefers host+port matches, falls back to host-only, always excludes `disabled` (`:44-56`, tests `client-certificate.test.ts:40-72`). Legacy workspace-embedded certificates are migrated into this model on read (`node-src/database/init-model/workspace.ts:21-45`).

### OAuth2Token
`models/o-auth-2-token.ts` — type `OAuth2Token`, prefix `oa2`, duplicate/sync false (`:3-11`). Fields: `refreshToken` `''`, `accessToken` `''`, `identityToken` `''`, `expiresAt` number|null (`null`; comment "Should be Date.now() if valid"), `xResponseId` null, `xError` null, `error` `''`, `errorDescription` `''`, `errorUri` `''` (`:13-46`). Parent is a request or request group (hierarchy table). `getOrCreateByParentId` (`node-src/services/o-auth-2-token.ts:26-38`).

### Settings
`models/settings.ts` — type `Settings`, prefix `set`, duplicate/sync false (`:15-19`). Type is `BaseModel & BaseSettings`; `BaseSettings` interface at `common-src/settings.ts:118-214`. Init `models/settings.ts:28-101` (defaults in that file):

`autoDetectColorScheme:false, autoHideMenuBar:false, autocompleteDelay:1200, clearOAuth2SessionOnRestart:true, darkTheme:'default', debugMode:false, deviceId:null, disableHtmlPreviewJs:false, disableResponsePreviewLinks:false, disableUpdateNotification:false, editorFontSize:11, editorIndentSize:2, editorIndentWithTabs:true, editorKeyMap:'default', enableKeyMapForInlineTextEditors:false, editorLineWrapping:true, enableAnalytics:true, showVariableSourceAndValue:false, filterResponsesByEnv:false, followRedirects:true, fontInterface:null, fontMonospace:null, fontSize:13, fontVariantLigatures:false, forceVerticalLayout:false (true when `PLAYWRIGHT_TEST` set, `:25-26`), hasKonnectPat:false, hideSecretValuesInPreviewAndConsole:true, konnectOrganizationId:null, hotKeyRegistry: newDefaultRegistry(), httpProxy:'', httpsProxy:'', lightTheme:'studio-light', maxHistoryResponses:20, maxRedirects:10, maxTimelineDataSizeKB:10, pluginNodeExtraCerts:'', noProxy:'', proxyScope:'requests', nunjucksPowerUserMode:false, pluginConfig:{}, pluginPath:'', preferredHttpVersion:'default', proxyEnabled:false, showPasswords:false, sidebarFocusForCollections:true, hasSeenSidebarFocusOnboarding:false, theme:'default', timeout:30000, updateAutomatically:true, updateChannel:'stable', useBulkHeaderEditor:false, useBulkParametersEditor:false, validateAuthSSL:true, validateSSL:true, saveVaultKeyLocally:true, enableVaultInScripts:false, saveVaultKeyToOSSecretManager:true, vaultSecretCacheDuration:30, dataFolders:[], scriptSandboxEnabled:true, scriptStrictModeEnabled:true, useQuickJsScriptSandbox:false, pluginSandboxEnabled:false, disabledSecurityRules:[], disabledBlockedProperties:[], disabledBlockedRoots:[], npmRegistryUrl:'', enableLegacyUnitTests:false`.

`lineWrapping?: boolean` exists in the interface but is **not** written by init (`common-src/settings.ts:160`) — it will be pruned by `initModel` on any save unless re-added. `ThemeSettings = Pick<Settings,'autoDetectColorScheme'|'lightTheme'|'darkTheme'|'theme'>` (`models/settings.ts:21`). Singleton: `getOrCreate` (`node-src/services/settings.ts:30-41`).

### Stats
`models/stats.ts` — type `Stats`, prefix `sta`, duplicate/sync false, no parent. Fields: `currentLaunch` null, `lastLaunch` null, `currentVersion` null, `lastVersion` null, `launches` 0, `createdRequests` 0, `deletedRequests` 0, `executedRequests` 0 (`:13-38`). Singleton; `incrementRequestStats` merges counts (`node-src/services/stats.ts:29-41`).

### UserSession
`models/user-session.ts` — type `UserSession`, prefix `usr`, duplicate/sync false (`:27-31`). Fields: `accountId` `''`, `id` `''`, `email` `''`, `firstName` `''`, `lastName` `''`, `symmetricKey` `{} as JsonWebKey`, `publicKey` `{} as JsonWebKey`, `encPrivateKey` `{} as AESMessage`, `vaultSalt?` `''`, `vaultKey?` `''`; plus `HashedUserSession.hashedAccountId` (`:3-26,33-46`). `AESMessage = {iv,t,d,ad}` (`:3-8`). Singleton (`node-src/services/user-session.ts:6-14`).

### RunnerTestResult
`models/runner-test-result.ts` — type `RunnerTestResult`, prefix `rtr`, duplicate/sync false (`:3-11`). Fields: `source:'runner'`, `iterations:0`, `duration:0`, `avgRespTime:0`, `iterationResults: RunnerResultPerRequest[][]` `[]`, `responsesInfo: {responseId,originalRequestName,originalRequestId}[]` `[]`, `version:'1'` (`:43-66`). `RequestTestResult` (`:16-22`) is embedded in `Response.requestTestResults`. Child of workspace/request group.

### PluginData, GitRepository, GitCredentials, CloudCredential, ProjectLintRuleset
`models/plugin-data.ts`: type `PluginData`, prefix `plg`, duplicate/sync false; `plugin` `''`, `key` `''`, `value` `''` (`:3-29`).
`models/git-repository.ts`: type `GitRepository`, prefix `git`, duplicate/sync false (`:7-15`). Fields (`:40-99`): `needsFullClone` false, `uri` `''`, `credentials` (deprecated) null, `credentialsId` null, `selectedAuthorEmail` null, `author {name,email}` `''`/`''` (deprecated), `hasUncommittedChanges` false, `cachedGitLastCommitTime` null, `cachedGitRepositoryBranch` null, `cachedGitLastAuthor` null, `hasUnpushedChanges` false, `uriNeedsMigration` true, `repoMigrationVersion` 0, `directory` null (null = app-managed folder), `folderSlug` null. `getGitRepoFolderName` builds `git_<slug>_<hex>` or falls back to `_id`; slug must match `/^[a-z0-9-]+$/` (`:108-121`, tests `git-repository.test.ts:4-38`).
`models/git-credentials.ts`: type `GitCredentials`, prefix `git_creds`, duplicate/sync false (`:14-22`); V1 legacy (token/provider/author) and V2 provider-specific (github/gitlab/custom/native) shapes (`:40-145`); guards `:150-187`.
`models/cloud-credential.ts`: type `CloudCredential`, prefix `cloudCred`, duplicate/sync false (`:117-121`); provider-discriminated credentials for aws/gcp/azure/hashicorp (`:5-115`).
`models/project-lint-ruleset.ts`: type `ProjectLintRuleset`, prefix `plr`, duplicate false, **sync true** (`:3-11`); single field `rulesetContent` `''` (`:13-26`). Parent is a project.

## Serialization quirks

1. **Zod defaults never run on load.** Schemas are only consumed by compatibility tooling (`packages/insomnia/src/common/zod/check-doc-compatibility.ts:20-42`, `check-schema-mismatch.ts:113-122`); all runtime defaulting is `model.init()` + `initModel`. Fields with `.default()` in zod but absent from `init()` rely on `optionalKeys` or are pruned.
2. **Unknown keys are pruned on every write.** `initModel` deletes any key not in the merged defaults and not listed in the model's `optionalKeys` (`node-src/database/init-model/index.ts:70-78`). Example: adding an experimental field requires adding it to `optionalKeys` or it disappears on the next `docUpdate`.
3. **`modified` is restamped on every `docUpdate`** unless the patch supplies it (`database-nedb.ts:138-143`); callers that want stable timestamps pass the old value explicitly (`node-src/services/project.ts:66-70`).
4. **Migrations run on every read**, because `find`/`findOne` call `initModel` (`database-nedb.ts:214,231`; comment `:228` "create a db init phase for migrations rather than doing it on every find"). Migrations:
   - Workspace: extract legacy `certificates[]` into `ClientCertificate` docs, fix non-string `name` → `My Workspace`, map legacy scopes `designer|spec → design` and unset → `collection` (`init-model/workspace.ts:9-83`).
   - Request: string body → body object (urlencoded bodies become `params`, others `{mimeType,text}`), non-string URL → `''`, auth with `username` but no `type` → `basic` (`init-model/request.ts:8-87`).
   - Response: `bodyCompression === '__NEEDS_MIGRATION__'` → `'zip'` (`init-model/response.ts:12-18`).
   - CookieJar: assigns missing cookie `id`s via `uuidv4`, defaults missing `source` to `'manual'` (`init-model/cookie-jar.ts:4-29`).
   - Settings: prunes hotkeys no longer in the default registry, re-adds defaults, resets `request_createHTTP` if it still conflicts with `sidebar_showCreateDropdown`, folds `templateTagSandboxEnabled` into `pluginSandboxEnabled` and deletes the old flag (`init-model/settings.ts:5-100`).
5. **`isPrivate` excludes a doc from sync** regardless of model `canSync` (`models/index.ts:33-45`; git sync skips such docs at `packages/insomnia/src/sync/git/ne-db-client.ts:82,221`). Only five `init()` functions set it: Request (`request.ts:442`), GrpcRequest (`grpc-request.ts:80`), CaCertificate (`ca-certificate.ts:29`), Environment (`environment.ts:70`), ClientCertificate (`client-certificate.ts:38`). For every other model, `isPrivate` is not in `objectDefaults` and not in any `optionalKeys` list (`grep` of `models/*.ts:17,51,53,67,74,76,121`), so the pruning loop at `init-model/index.ts:73-78` **deletes `isPrivate` from the in-memory doc on every read/write**, even if a row on disk has it. Pruning does not write back, so the stored row keeps the value; `database.find`/`findOne` results for e.g. `Workspace` or `RequestGroup` therefore return `isPrivate === undefined`. Callers such as `services.request.create({ isPrivate: true })` only persist the flag for the five models above (`packages/insomnia/src/routes/organization.$organizationId.project.$projectId.workspace.$workspaceId.debug.request.new-mock-send.tsx:32` targets a Request, so it works; a Workspace/RequestGroup private flag would be silently dropped).
6. **Duplication rewrites ids by string substitution**, serializing each listed field to JSON and `split(oldId).join(newId)` (`models/utils/replace-ids-in-fields.ts:5-34`). This is text-level and will rewrite ids inside template tags (`{% response 'body', 'req_x', ... %}`), scripts and URLs. `konnectRouteKey`/`konnectRouteId` are always cleared on duplicate (`request.ts:472`, `request-group.ts:67`, `grpc-request.ts:64`, `websocket-request.ts:78`), but Socket.IO requests have no such field.
7. **VCS hashing** deletes `modified` and resets `Workspace.parentId` / `ProjectLintRuleset.parentId` to `null` before hashing (`models/utils/ignore-keys.ts:5-57`); the schema checker compensates by making `modified` optional and `Workspace.parentId` nullable (`packages/insomnia/src/common/zod/check-doc-compatibility.ts:33-39`).
8. **Legacy sentinels:** `Response.environmentId = '__LEGACY__'` hides old responses when env filtering is on (`models/response.ts:83-85`); `Response.bytesContent = -1` marks a missing legacy field (`:68-69`); `Response.bodyCompression = '__NEEDS_MIGRATION__'` is an in-DB marker (`:77`).
9. **Base env / cookie jar ids are deterministic** (`env_<sha1(parentId)>`, `jar_<sha1(parentId)>`) to make sync idempotent (`node-src/services/environment.ts:80-90`, `cookie-jar.ts:22-24`).
10. **Git repo id is double-encoded on Project** (`git_xxx` → `gr_xxx`) so older app versions cannot touch the git folder; `getQueryableGitRepositoryIds` returns both forms for queries (`models/project.ts:17-70`).
11. **Request versions are gzip+base64 in a string column** (`node-src/services/request-version.ts:41-44`); they are deduplicated by deep comparison ignoring metadata fields (`:126-145`).
12. **`RequestGroup.isRequestGroupId` uses `startsWith('fld')` without the underscore** (`models/request-group.ts:44`), unlike every other id guard (`req_`, `wrk_`, `env_`, `greq_`, …). Verified: `'fldm_x'.startsWith('fld') === true`, so a `RequestGroupMeta` id (`fldm_…`) also satisfies `isRequestGroupId`, and a `Folder Meta` id would be routed to `services.requestGroup.getById` by callers such as `packages/insomnia/src/routes/organization.$organizationId.project.$projectId.workspace.$workspaceId.debug.reorder.tsx:12`, `packages/insomnia/src/ui/components/tabs/request-delete-fallback.ts:32` and `packages/insomnia/src/main/network/o-auth-2/get-token.ts:605`. In practice those callers only receive folder/request ids from the UI, so the bug is latent, not active.
13. **`MockServer` and `MockRoute` redeclare `parentId`** on top of the base model (`mock-server.ts:16`, `mock-route.ts:26`); the redeclaration is required-string and overrides the base's optional runtime default.
14. **`UnitTestResult.results` is `null` at init** although typed as `TestResults` (`unit-test-result.ts:47-58`).
15. **`Settings.lineWrapping`** is declared optional but not initialized, so it will be dropped on the next settings save unless a caller sets it (`common-src/settings.ts:160` vs `models/settings.ts:28-101`).
16. **`createModelSchema`'s `prefix` parameter is unused**; id prefixes come from the module-level `prefix` export consumed by `initModel` (`models/base-types.ts:67-73`, `init-model/index.ts:37-39`).
17. **`Workspace.name` default is "New API Collection", not "New Workspace"** because it uses `strings.collection.singular` (`models/workspace.ts:25`, `common-src/strings.ts:20-23`); `ApiSpec.fileName` similarly uses `strings.document.singular` (`api-spec.ts:30`, `strings.ts:28-31`).
18. **`McpPayload` syncs false while its parent `McpRequest` syncs true and `WebSocketPayload`/`SocketIOPayload` sync true** (`mcp-payload.ts:11`, `mcp-request.ts:11`, `websocket-payload.ts:12`, `socket-io-payload.ts:15`) — an asymmetry to preserve or deliberately fix.

## Feature list

Priorities: **P0** = required for any usable client (create/read/update/delete, sync-critical); **P1** = core per-protocol behavior; **P2** = secondary/admin.

### Core model + persistence
- **P0** Base model with `_id/type/parentId/modified/created/isPrivate/name` and 44-type enum — `models/base-types.ts:3-63`.
- **P0** `initModel` merges global defaults, model `init()`, then patches; generates prefixed uuid ids; runs type-specific migration; prunes unknown keys except `optionalKeys` — `node-src/database/init-model/index.ts:11-81`.
- **P0** Separate NeDB collection per type with `insomnia.<Type>.db` filenames, `autoload` and `corruptAlertThreshold: 0.9` — `node-src/database/database-nedb.ts:242-434`.
- **P0** `docCreate` / `docUpdate` / `insert` / `update` / `find` / `findOne` / `count` / `remove` / `removeWhere` / `unsafeRemove` contract — `src/database/types.ts:48-154`; implementation `database-nedb.ts:90-607`.
- **P0** Change buffering + listeners (`bufferChanges`, `flushChanges`, `onChange`; each write notifies `insert|update|remove` with patches) — `database-nedb.ts:55-113,610-625`.
- **P0** Descendant map, child→parent reverse map, `generateDescendantMap(queryTypes)` for partial operations — `models/index.ts:75-177`.
- **P0** Cascading duplicate that re-parents children, allocates new ids per prefix, and rewrites references; only `canDuplicate` child types copied — `database-nedb.ts:147-204`.
- **P0** Cascading remove (`remove`, `removeWhere`) vs `unsafeRemove` — `database-nedb.ts:454-511`.
- **P1** `withAncestors` leaf→root walk across selected types — `database-nedb.ts:521-551`.
- **P1** `getWithDescendants` using the type map, batched `parentId: {$in: …}` queries — `database-nedb.ts:559-607`.
- **P1** `canSync(doc)` = `!isPrivate && model.canSync` — `models/index.ts:33-45`.
- **P1** `getModel` / `mustGetModel` / `canDuplicate` / `rewriteReferences` dispatch — `models/index.ts:47-72`.
- **P1** Repo-wide DB repair at startup: clear stale sync flags for local projects, merge duplicate base environments, dedupe cookie jars, backfill API spec filenames, migrate old git URIs — `node-src/database/repair-database.ts:10-28`.
- **P2** Database abstracted behind `IDatabase` with `initDatabase()` and a proxy that throws before init — `src/database/index.ts:17-34`; same for services (`src/services/index.ts:10-48`).

### Workspace / project
- **P0** Workspace scopes `collection|design|mock-server|environment|mcp` with guards and scope→activity mapping — `models/workspace.ts:16-85`.
- **P0** Scratchpad workspace/project/organization sentinels (`wrk_scratchpad`, `proj_scratchpad`, `org_scratchpad`) — `workspace.ts:12`, `project.ts:12`, `organization.ts:3`.
- **P0** Workspace parent must be a project (create/update/upsert validation) — `node-src/services/workspace.ts:16-56`.
- **P0** Workspace migration: extract legacy certificates, repair name, map legacy scope — `init-model/workspace.ts:9-83`.
- **P1** WorkspaceMeta per-workspace active tab/request/env/unit-suite and git flags, lazily created — `models/workspace-meta.ts:9-54`; `node-src/services/workspace-meta.ts:33-36`.
- **P1** WorkspaceMeta reference rewriting on duplicate — `workspace-meta.ts:28-37`.
- **P1** Project variants Local/Remote/Git discriminated on `remoteId`/`gitRepositoryId`, guards and "connected git project" check — `models/project.ts:72-115`.
- **P1** Git repo id protection (`gr_` encoding), decoding, queryable-id set — `project.ts:17-70`.
- **P1** Project sort (default org first, then name) — `project.ts:142-145`.
- **P1** Storage-type resolution from `StorageRules` and human-readable label — `project.ts:147-215`.
- **P2** Clearing workspace sync flags when a project leaves cloud sync, preserving `modified` — `node-src/services/project.ts:37-70`.
- **P2** ProjectLintRuleset upsert per project — `models/project-lint-ruleset.ts:13-26`; `node-src/services/project-lint-ruleset.ts:6-24`.

### HTTP request / response
- **P0** Request model with body, params, headers, path params, scripts, six request settings — `models/request.ts:352-393`.
- **P0** Thirteen auth variants with discriminated-union schema plus empty-object fallback — `request.ts:45-246`.
- **P0** `init()` defaults (GET, empty body/auth/headers, sort key `-Date.now()`, `isPrivate:false`) — `request.ts:430-453`.
- **P0** Request migration: body string→object, bad URL reset, auth type inference — `init-model/request.ts:8-87`.
- **P0** Reference rewriting in URL/body/params/headers/auth/scripts on duplicate — `request.ts:455-474`.
- **P0** RequestMeta (preview mode, response filter + history, active response, saved body, pinned, download path, stream summary) — `models/request-meta.ts:13-49`.
- **P0** Response model with status, sizes, timings, headers, body/timeline file paths, compression, error, cookie settings, test results — `models/response.ts:23-88`.
- **P0** Response creation: snapshot request version, prune to `maxResponses` (default 20), optional env filtering — `node-src/services/response.ts:45-79`.
- **P1** Latest response lookup with optional env filter sorted by `modified: -1` — `response.ts:27-43`.
- **P1** Response body read with gunzip and file-ownership check before overwrite — `helpers/response-operations.ts:138-203`, `response.ts:14-17`.
- **P1** Delete response history including on-disk body/timeline/event-log files; "clear all response history" tolerates only ENOENT — `helpers/response-operations.ts:20-80`.
- **P1** RequestVersion snapshot with change detection, compression, and restore that skips `_id/type/created/modified/metaSortKey/description/parentId/name` — `node-src/services/request-version.ts:22-145`.
- **P1** Path-parameter extraction, URL encoding, and merge of saved/unsaved params — `models/request.ts:290-350`.
- **P1** GraphQL operation detection and subscription detection — `request.ts:402-422`.
- **P2** SSE detection via `Accept: text/event-stream` — `request.ts:399-400`.

### Requests organization
- **P0** RequestGroup (folder) with own env/data, auth, headers, scripts, nested groups and requests — `models/request-group.ts:24-57`.
- **P0** Middle-point `metaSortKey` insertion on duplicate for requests, groups, environments, gRPC/WS/Socket.IO — `node-src/services/request.ts:39-58`, `request-group.ts:39-56`, `environment.ts:115-131`, `grpc-request.ts:44-62`, `websocket-request.ts:28-46`, `socket-io-request.ts:32-50`.
- **P0** `" (Copy)"` name suffix rules — same cites.
- **P1** RequestGroupMeta `collapsed` per folder — `models/request-group-meta.ts:13-25`.
- **P1** Group reference rewriting including `environment` and `kvPairData`; clears `konnectRouteId` — `request-group.ts:59-69`.
- **P2** Folder name displays as "Folder" in command search/model metadata — `request-group.ts:8`.

### Environment
- **P0** Environment with both JSON `data` and KV `kvPairData` representations plus `environmentType` discriminator — `models/environment.ts:19-53`.
- **P0** Nested sub-environments (parent is another Environment) — `models/index.ts:126`.
- **P0** Deterministic single base environment per parent with KV default and name `Base Environment` — `node-src/services/environment.ts:76-105`.
- **P1** KV pair data types `json|str|secret` with `enabled`/`isConfidential` — `models/environment.ts:24-38`.
- **P1** Vault path constants and secret-masking value; secret removal on vault-key reset — `models/environment.ts:10-13`, `node-src/services/environment.ts:36-63`.
- **P1** Sort by `metaSortKey`; duplicate with midpoint key — `environment.ts:65-74,115-131`.
- **P2** `UserUploadEnvironment` (data/order/name subset for CSV/JSON iteration data) — `models/environment.ts:59`.

### Cookie jar
- **P0** CookieJar with `Cookie[]` and the full cookie attribute set (expires/domain/path/secure/httpOnly/hostOnly/creation/lastAccessed/extensions) — `models/cookie-jar.ts:16-61`.
- **P0** Deterministic one-jar-per-parent creation — `node-src/services/cookie-jar.ts:16-28`.
- **P1** Cookie `expires` normalization: `'Infinity'`→null, invalid Date→null, numeric coercion — `cookie-jar.ts:23-40`.
- **P1** Cookie `id` backfill and `source` defaulting to `manual` — `init-model/cookie-jar.ts:4-29`.
- **P1** `manual` vs `response` cookie origin (template rendering vs literal) — `cookie-jar.ts:51-54`.

### gRPC / WebSocket / Socket.IO / MCP
- **P1** gRPC request: proto file + method, metadata headers, JSON body text, reflection API config with buf.build defaults — `models/grpc-request.ts:12-87`.
- **P1** gRPC reference rewriting remaps `protoFileId` and body/metadata ids — `grpc-request.ts:59-66`.
- **P1** GrpcRequestMeta (pinned, lastActive) — `models/grpc-request-meta.ts:13-27`.
- **P1** WebSocket request with cookie/encode/redirect/proxy settings and shared auth model — `models/websocket-request.ts:17-64`.
- **P1** WebSocketPayload `{name,value,mode}` with `mode` defaulting to `application/json` and id rewriting in `value` — `websocket-payload.ts:14-34`.
- **P1** WebSocketResponse with event-log file path + timeline path — `websocket-response.ts:14-54`.
- **P1** WebSocketRequestMeta (pinned) — `websocket-request-meta.ts:13-26`.
- **P1** Socket.IO request with `eventListeners[{id,eventName,desc,isOpen}]` and `settingPath` — `models/socket-io-request.ts:17-72`.
- **P1** SocketIOPayload args array seeded with one JSON arg with a uuid — `socket-io-payload.ts:17-45`.
- **P1** SocketIOResponse minimal (event log, timeline, error, elapsed) — `socket-io-response.ts:13-38`.
- **P2** MCP request with stdio/streamable-http transports, env KV pairs, roots, resource subscriptions, SSL validation toggle — `models/mcp-request.ts:13-74`.
- **P2** MCP payload/response docs (payload sync off) — `mcp-payload.ts:13-29`, `mcp-response.ts:11-50`.
- **P2** Clearing MCP resource subscriptions — `node-src/services/mcp-request.ts:39-43`.

### API spec, tests, mocks, protos
- **P1** ApiSpec `{fileName,contentType yaml|json,contents}` with get-or-create per workspace — `models/api-spec.ts:16-34`, `node-src/services/api-spec.ts:10-25`.
- **P1** Startup backfill of ApiSpec filename from workspace name, avoiding no-op writes — `repair-database.ts:35-45`.
- **P1** UnitTestSuite/UnitTest tree with `requestId` link and code; id rewriting of `requestId` — `models/unit-test-suite.ts:15-30`, `unit-test.ts:15-42`.
- **P2** UnitTestResult storing mocha `TestResults` (null at init) — `models/unit-test-result.ts:15-59`.
- **P2** RunnerTestResult with per-iteration results, response info, versioned `'1'` — `models/runner-test-result.ts:13-66`.
- **P2** MockServer `{url,useInsomniaCloud}` per mock workspace, `findByProjectId` — `models/mock-server.ts:15-33`, `node-src/services/mock-server.ts:38-41`.
- **P2** MockRoute `{name,body,headers,method,mimeType,statusCode,statusText}` defaults (`/`, GET, 200, application/json) — `models/mock-route.ts:16-44`.
- **P2** ProtoDirectory/ProtoFile tree (`pd`/`pf` prefixes) — `models/proto-directory.ts`, `models/proto-file.ts`.

### Platform/admin
- **P2** Settings singleton with 60+ defaults, hotkey registry migration and conflict repair — `models/settings.ts:28-101`, `init-model/settings.ts:5-100`, `node-src/services/settings.ts:30-41`.
- **P2** Stats counters and increment helper — `models/stats.ts:13-38`, `node-src/services/stats.ts:29-41`.
- **P2** UserSession with encrypted key material, vault salt/key, hashed account id — `models/user-session.ts:3-46`.
- **P2** OAuth2 token cache per request/group with debug `xResponseId`/`xError` and error fields — `models/o-auth-2-token.ts:13-46`.
- **P2** PluginData `{plugin,key,value}` key/value store — `models/plugin-data.ts:13-29`.
- **P2** GitRepository cache fields, migration version, user-chosen directory, path-safe folder slug — `models/git-repository.ts:40-121`.
- **P2** GitCredentials V1/V2 compatibility guards, native credential, renewal support — `models/git-credentials.ts:40-187`.
- **P2** CloudCredentials for AWS (temporary/file/SSO), GCP, Azure, HashiCorp (HCP/on-prem, token/appRole) — `models/cloud-credential.ts:5-133`.
- **P2** CA cert and client cert models with host-matching filter (exact host+port preferred) — `models/ca-certificate.ts:14-31`, `models/client-certificate.ts:15-56`.
- **P2** Schema compatibility checker reporting `match`/`strict-match`/`match-with-unknown-fields`/`mismatch`, with union-branch scoring — `packages/insomnia/src/common/zod/check-doc-compatibility.ts:7-64`, `check-schema-mismatch.ts:29-122`. Only 15 of 44 models export a zod `schema` (api-spec, ca-certificate, cookie-jar, environment, grpc-request, mcp-request, mock-route, mock-server, request-group, request, socket-io-request, unit-test-suite, unit-test, websocket-request, workspace), so the rest are never schema-checked (`check-doc-compatibility.ts:29-31`).
- **P2** VCS ignore/reset rules (`modified` deleted; `Workspace.parentId`/`ProjectLintRuleset.parentId` reset to null) — `models/utils/ignore-keys.ts:5-57`.
