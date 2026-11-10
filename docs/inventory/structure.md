# Structure

## Overview

Insomnia is an npm-workspaces monorepo (`package.json:17-28`) whose deliverable is an Electron desktop app plus a Node CLI. The root package is private and only orchestrates workspaces (`package.json:2-4`); all source lives under `packages/`. Ten workspaces are declared; `packages/insomnia-component-docs` exists on disk but is deliberately not a workspace (it has its own `package-lock.json` and Docusaurus toolchain, `packages/insomnia-component-docs/package.json:1-6`).

Runtime process model:

- **Main process** — `packages/insomnia/src/entry.main.ts` (468 lines), bundled by esbuild to `entry.main.min.js`, declared as the Electron `main` in `packages/insomnia/package.json:18`.
- **Preload** — `packages/insomnia/src/entry.preload.ts` (549 lines), the only bridge between main and the app renderer (`packages/insomnia/esbuild.entrypoints.ts:48-55`).
- **Renderer** — React Router 7 SPA, `ssr: false` (`packages/insomnia/react-router.config.ts:5`), hydrated by `packages/insomnia/src/entry.client.tsx:191`; 175 route files under `packages/insomnia/src/routes/`.
- **Two auxiliary renderer windows** — a hidden window for script execution (`packages/insomnia/src/entry.hidden-window.ts`, preload `entry.hidden-window-preload.ts`) and a plugin window (`entry.plugin-window.ts`, preload `entry.plugin-window-preload.ts`).
- **CLI** — `packages/insomnia-inso` (`inso`), a Commander program bundled with esbuild to CJS and optionally packaged with `@yao-pkg/pkg` (`packages/insomnia-inso/package.json:17-21,39-43`).

Shared logic is factored into libraries: `insomnia-data` (runtime-agnostic model/service contracts plus NeDB and Node implementations), `insomnia-vcs` (snapshot store), `insomnia-api` (Kong cloud HTTP client), `insomnia-analytics` (Segment), `insomnia-testing` (Mocha-based test generation/execution), `insomnia-scripting-environment` (the `insomnia.*` script object model), and the E2E harnesses `insomnia-smoke-test` and `insomnia-test`.

## Packages

| Package | Purpose | Key source dirs |
|---|---|---|
| `insomnia` (v13.3.1) | Electron app: main, preload, React Router renderer, network engine, templating, plugins, git sync | `packages/insomnia/src/{main,ui,routes,network,common,sync,templating,scripting,runtimes,plugins,konnect}`; build `esbuild.entrypoints.ts`, `vite.config.ts` |
| `insomnia-data` | Data layer: `IDatabase`/`Services` contracts, 54 model modules, NeDB implementation, migrations | `packages/insomnia-data/src/{models,services,database}`, `node-src/{database,services}`, `common-src/` |
| `insomnia-vcs` | Version-control snapshot store used by cloud sync and Git sync; 64 async operations on `VCS` | `packages/insomnia-vcs/src/{vcs.ts,store/,backend-projects.ts,crypt.ts}` |
| `insomnia-api` | Typed client for Insomnia/Kong cloud APIs (user, orgs, projects, vault, trial, VCS) | `packages/insomnia-api/src/*.ts` |
| `insomnia-analytics` | Segment client and event enums shared by app and CLI | `packages/insomnia-analytics/src/{analytics.ts,events.ts}` |
| `insomnia-inso` | CLI: run collections/tests, lint/export specs, scripts from `.insorc` | `packages/insomnia-inso/src/{cli.ts,db/,commands/,reporters/}` |
| `insomnia-testing` | Generates Mocha test files from unit-test suites and runs them with a fake `insomnia` global | `packages/insomnia-testing/src/{generate,run}` |
| `insomnia-scripting-environment` | Objects injected into pre-request/after-response scripts (`insomnia`, request/response, store) | `packages/insomnia-scripting-environment/src/objects/` |
| `insomnia-smoke-test` | Playwright E2E against dev/build/packaged app, with mock servers | `packages/insomnia-smoke-test/{playwright,tests,server,fixtures}` |
| `insomnia-test` | Second-generation Playwright harness (page objects, flows, single-worker) | `packages/insomnia-test/{components,pages,models,flows,tests,misc}` |
| `insomnia-component-docs` (not a workspace) | Docusaurus site documenting UI components | `packages/insomnia-component-docs/{docs,src}` |

Package export shapes: `insomnia-data` exposes three entry points — `.`, `./node`, `./common` (`packages/insomnia-data/package.json:17-30`); `insomnia-analytics` exposes `.` and `./events` (`packages/insomnia-analytics/package.json:18-26`); `insomnia-vcs`, `insomnia-api` expose `.` only (`packages/insomnia-vcs/package.json:18`). Everything is consumed as TypeScript source through workspace symlinks (e.g. `packages/insomnia-inso/src/cli.ts:14-25` imports `insomnia/src/...` and `insomnia-data/...` directly).

## Build and configuration

- **Toolchain versions**: Node `>=24.18.0`, npm `>=11` (`package.json:13-16`), `.nvmrc` = `24.18.0`; TypeScript 5.8.3, Vite ^7.1.3, esbuild ^0.25.10, Vitest ^3.2.4 (`package.json:52-80`).
- **Native dependency**: `@getinsomnia/node-libcurl` 3.3.0 is a root dependency (`package.json:90`) and must be rebuilt per runtime: `install-libcurl-electron` (Electron 43.2.0) and `install-libcurl-node` (Node 24.18.0) (`package.json:36-37`). `postinstall` runs `patch-package`, bundle-plugin verification and the Electron libcurl install (`package.json:48`).
- **Root scripts**: `dev` (`package.json:30`), `watch:app` = build electron entrypoints then `start:dev-server` (`package.json:40`), `app-build`/`app-package` (`package.json:41-42`), `inso-package` (`package.json:39`), smoke suites (`package.json:43-47`), circular-reference check (`package.json:49`).
- **Vite (renderer)**: plugins `reactRouter()`, `tailwindcss()`, and a custom `writePortFile` that writes the live dev port to `.vite-port` (`packages/insomnia/vite.config.ts:10-33,89`). Dev server port comes from `pkg.dev['dev-server-port']` = 3334 (`packages/insomnia/vite.config.ts:46`; `packages/insomnia/package.json` `"dev"` block). Aliases: `~` → `src`, plus shims for `path`, `events`, `util`, `buffer` (`packages/insomnia/vite.config.ts:77-87`). Worker format is `es` (`:90`). SSR resolution prepends the `browser` condition so packages with a `browser` export don't pull Node code into the server bundle (`:99-102`). `node-libcurl` is external in build and excluded from dep optimization.
- **React Router**: `appDirectory: 'src'`, `ssr: false`, `serverModuleFormat: 'cjs'`, `v8_middleware: true` (`packages/insomnia/react-router.config.ts:4-9`). Routes are discovered by `flatRoutes` with `ignoredRouteFiles: ['**/*.test.ts']` (`packages/insomnia/src/routes.ts:10-12`).
- **esbuild (main/preload/aux windows)**: six entry points built as CJS for `platform: 'node'` — main, preload, hidden window + preload, plugin window + preload (`packages/insomnia/esbuild.entrypoints.ts:48,62,79,97,114,127`). Output goes to `src/` in development and `build/` in production (`:33`). A `rendererToNodePlugin` rewrites `*.renderer` imports to `*.node` for main/CLI builds (`:9-14`). The main build keeps native/wasm modules external (node-libcurl, quickjs-emscripten, node-llama-cpp, reflink, `apiconnect-wsdl`, all Node builtins) (`:127` onward). In development the script deletes `.vite-port`, and `--autoRestart` watches all six builds and restarts Electron after the sixth build (`:180-260`).
- **Production build** (`npm run build`): `react-router build` then `scripts/build.ts`, which requires Node 24.x, builds the entry points, and copies `bin/`, `src/static`, `src/icons`, the four `.mjs` utility-process scripts (lint, mock generation, git commit generation, MCP sampling), `hidden-window.html` and `plugin-window.html` into `.. /build` (`packages/insomnia/scripts/build.ts:20-80`).
- **Packaging**: electron-builder config declares `appId: com.insomnia.app`, the `insomnia://` protocol (`packages/insomnia/electron-builder.config.js:11-17`), ships only `./build` plus `package.json` and overwrites `main` to `entry.main.min.js` (`:19-38`), notarizes macOS universal dmg/zip (`:46-72`), unpacks native modules from asar (`:73-114`), builds Windows nsis+squirrel x64 with a custom signer (`:117-142`), and Linux AppImage/deb/tar.gz/rpm/snap (`:172-190`). `BUILD_TARGETS` env narrows the target matrix (`:193-204`).
- **App config** `packages/insomnia/config/config.json`: user-data folder name, bundled plugins (`@kong/insomnia-plugin-external-vault`, `@kong/insomnia-plugin-ai`), Segment keys, Customer.io keys, Sentry DSN.
- **Tests**: Vitest with `__IS_RENDERER__: false` and `INSOMNIA_DATA_PATH` pointed at the temp dir; route `.tsx` files excluded (`packages/insomnia/vitest.config.ts:5-28`). A separate config runs the large sandbox-vendored regression suites (`packages/insomnia/vitest.sandbox-vendored-regression.config.ts:10-25`). Type-check requires `react-router typegen` first (`packages/insomnia/package.json`, `type-check` script). TS is `strict` with `verbatimModuleSyntax`, path alias `~/*` (`packages/insomnia/tsconfig.json`).
- **CLI build**: esbuild bundles `src/index.ts` to CJS `dist/index.js`, target `node22`, alias `electron` → `../insomnia/send-request/electron`, externals node-libcurl/fsevents/mocha (`packages/insomnia-inso/esbuild.ts:21-58`); `bin/inso` is a Node shim calling `go()` (`packages/insomnia-inso/bin/inso:4`); packaging uses `pkg` (`packages/insomnia-inso/package.json:43`).
- **CI** (12 workflows): `test.yml` runs lint, type-check, unit tests and two freshness gates (autocomplete snippets, JSON schema) on Ubuntu (`(.github/workflows/test.yml:20-70)`); `test-e2e.yml` has separate `build` and `test` jobs (`:20,61`); `test-cli.yml` (`:20`); release workflows `release-start/build/publish/recurring`, `homebrew.yml`, `insomnia-test.yml`, `sast.yml`, `update-changelog.yml`, `ksai.yml`.
- **Patches**: `patches/apiconnect-wsdl+2.0.36.patch`, `json-order`, `tinykeys`; schema `schemas/insomnia.schema.5.1.json`; circular-dependency baseline in `scripts/circular-references/`.

## Entrypoints

### Main process — `packages/insomnia/src/entry.main.ts`

Startup order is load-bearing:

1. `app.setPath('userData', ...)` before Sentry init; `INSOMNIA_DATA_PATH` overrides the location (`:54-58`).
2. Logging, electron-storage, Sentry, privileged protocol schemes, fetch wiring (`insomniaFetch` via `configureFetch`, V3 client defaults, `net.fetch` as the implementation so proxy/certs apply) (`:60-76`).
3. Squirrel restart check (`:78-80`); context menu installed on `web-contents-created` (`:89-95`).
4. `app.on('ready')` (`:98`) registers every IPC family — electron, main, path, gRPC, cookies, git service, LLM config, WebSocket, Socket.IO, cURL, MCP, secret storage, electron storage — then `registerAllIpcHandlers([new SyncIpcHandler()])` (`:99-113`). Spellchecker dictionary download is disabled with a bogus URL (`:121-124`); web permissions are default-deny through `isPermissionAllowed` (`:127-129`).
5. Database/services/runtime initialization: `initDatabase(mainDatabase)` → `initServices(servicesNodeImpl)` → `initRuntime(nodeRuntime)` → `_createModelInstances()` (`:144-148`), then proxy watcher, version backup, Sentry analytics watch, git credential migration, git folder-slug backfill (ordered before any window exists to avoid races, `:150-161`), `_launchApp()` (`:163`), updater init, `responses/` directory (`:168`).
6. Default protocol `insomnia`/`insomniadev` registration, dev variant re-registering the Electron binary with argv (`:175-219`); `window-all-closed` quits except macOS (`:221`); macOS `activate` recreates a window; quit closes the inspector.
7. `_launchApp` (`:259`) handles argv paths (folder → `insomnia://app/open-folder?path=...`, `:244-256`), a single-instance lock (skipped when `PLAYWRIGHT_TEST` so E2E can run instances in parallel), `second-instance`, macOS `open-url`/`open-file`, and renderer `openDeepLink`. It also deletes the `Origin` header from every outgoing session request (`:334-335`).
8. `_createModelInstances` (`:348`) creates stats, settings, scratchpad project and scratchpad workspace idempotently; `_trackStats` (`:405`) emits `appStarted` analytics and schedules an "updated" toast 5 s after launch.

Windows (`packages/insomnia/src/main/window-utils.ts`): a `Map` keyed `Insomnia | HiddenBrowserWindow` (`:39`), `createWindow` at `:204` with `webPreferences` from `MAIN_WINDOW_SECURITY` (nodeIntegration false, contextIsolation true, `:242-250`), navigation guard opening external http(s) in the OS browser (`:269-285`), `setWindowOpenHandler` deny (`:284`), dev URL from `.vite-port` with fallback 3334, production URL `https://insomnia-app.local` (`:288-304`), a full application menu (`:320-793`), zoom persistence (`:827-901`), and `createWindowsAndReturnMain` which creates the hidden window and defers plugin-window creation to `did-finish-load` so Playwright's `firstWindow()` returns the app (`:904-914`). The hidden window is created at `:97` and stopped at `:200`; it deliberately runs with Node integration (documented in `packages/insomnia/src/main/window-security.ts:10-11`).

### Preload — `packages/insomnia/src/entry.preload.ts`

Builds bridge objects (`main`, `dialog`, `app`, `shell`, `clipboard`, `webUtils`, `path`, `database`, `env`, plus `plugins`, `cookies`, `grpc`, `secretStorage`, `electronStorage`, `git`, `llm`, `webSocket`, `socketIO`, `curl`, `mcp`) and wraps errors with `invokeWithNormalizedError` (`packages/insomnia/src/main/ipc/invoke.ts:20-26`). Under context isolation it calls `contextBridge.exposeInMainWorld` for each (`:512-537`); otherwise it assigns to `window` (`:538-549`). Because a Proxy cannot cross contextBridge, services are exposed as a flat `_dataServicesInvoke` (`:520-527`) and the renderer rebuilds the Proxy (`packages/insomnia/src/entry.client.tsx:49-56`, `packages/insomnia/src/ui/services-proxy.ts:6-20`).

IPC contract is typed by channel unions: `HandleChannels` (`packages/insomnia/src/main/ipc/electron.ts:23-213`), `MainOnChannels` (`:218-270`), `RendererOnChannels` (`:272-304`), `OnceChannels` (`:310`), helpers `ipcMainHandle`/`ipcMainOn`/`ipcMainOnce` (`:214,306,311`). `main/ipc/main.ts` alone registers 56 invoke handlers and exposes the `RendererToMainBridgeAPI` surface (`:224-391`, `registerMainHandlers` at `:392`). Domain-scoped handlers live in dedicated modules (cookies, gRPC, curl, ws, socket.io, MCP, secret storage, electron storage) and are registered from the ready handler. A generic multiplexer `main.invoke` dispatches to `BaseIpcHandler` subclasses (`packages/insomnia/src/main/ipc/base-ipc-handler.ts:26-34`).

Window kinds:

- **Main renderer** — `entry.client.tsx`: Sentry, templating DB auth token (`:42`), renderer database (`:45`), services Proxy (`:50-56`), runtime (`:57`), localStorage→session migration (`:62`), session/vault injection from env vars, SQLite-free `applyColorScheme` (`:154`), Konnect project migration, initial entry via `history.replaceState` (`:173-190`), then `hydrateRoot(<HydratedRouter/>)` (`:191`). Root route provides the shell, error boundary (`packages/insomnia/src/root.tsx:121`), app hooks (`:812`) and the app-wide modals (`packages/insomnia/src/ui/modals.tsx:79`).
- **Server entry** — `entry.server.tsx` exists (SSR `renderToPipeableStream`) but the router is configured SPA (`react-router.config.ts:5`).
- **Hidden script window** — `entry.hidden-window.ts` initializes services from the preload and runs scripts on port messages with a default 5000 ms timeout (`:31-33`); its HTML enables `unsafe-eval` CSP (`packages/insomnia/src/hidden-window.html`). Its preload exposes `bridge`, a proxied `Promise`, and `_dataServices` (`packages/insomnia/src/entry.hidden-window-preload.ts:64-72`).
- **Plugin window** — `entry.plugin-window.ts` handles `plugins.invoke` messages with `invokePluginMethod`, initializes a database proxy that tunnels every call over `database.invoke` and deletes `_dataServices` after init; its preload provides `window.app` (sync `getPath`), `showAlert/showWrapper/showPrompt`, `dialog` and `clipboard` shims (`packages/insomnia/src/entry.plugin-window-preload.ts:3-43`). Creation/teardown and bridge metrics are in `packages/insomnia/src/main/plugin-window.ts:177,249`.

### Renderer routing and UI

`flatRoutes` mounts 175 modules under `packages/insomnia/src/routes/`: 109 `organization.*` routes, 29 `git.*`, 11 `auth.*`, plus AI, cloud-credentials, onboarding, import, settings, trial, remote-files. The canonical hierarchy is `organization.$organizationId.project.$projectId.workspace.$workspaceId.{debug,spec,test,mock-server,mcp,insomnia-sync}`. Mutations are route `clientAction`s (e.g. send at `packages/insomnia/src/routes/organization.$organizationId.project.$projectId.workspace.$workspaceId.debug.request.$requestId.send.tsx:393`, implementation at `:116`; collection runner at `...debug.runner.tsx:946`) and reads are `clientLoader`s (workspace at `...workspace.$workspaceId.tsx:91`). The root registers global listeners for main-process pushes (preferences, undo/redo, reload-plugins, toasts, plugin dialogs, prompts) in `packages/insomnia/src/ui/renderer-listeners.ts:14-45`.

### CLI — `packages/insomnia-inso`

`bin/inso` → `dist/index.js` → `go()` (`packages/insomnia-inso/src/cli.ts:265`). Commander root declares version, description, and global options `-w/--workingDir`, `--verbose`, `--ci`, `--config`, `--printOptions` (`:325-348`); options merge from `.insorc` (cosmiconfig, `tryToReadInsoConfigFile`, `:76-105`) then command flags (`:288-312`). Commands: `run test` (`:354`), `run collection` (`:494`), `lint spec` (`:896`), `export spec` (`:962`), `script <name>` (`:1008`), hidden `generate-docs` (`:1041`). Data is loaded by trying an Insomnia export file, then a Git repo, then a NeDB app-data directory (`packages/insomnia-inso/src/db/index.ts:16-65`; adapters at `db/adapters/insomnia-adapter.ts:137`, `git-adapter.ts:73`, `ne-db-adapter.ts:43`). Tests run through an in-memory database plus the app's send-request callback (`cli.ts:451,791`) and `runTestsCli` (`:468`); collections use the same send callback with iteration data, delays and bail; reports are produced by `RunCollectionResultReport` (`src/commands/run-collection/result-report.ts:30`). Reporters: `dot,list,min,progress,spec,tap` (`src/reporter/index.ts:4`). Telemetry uses `insomnia-analytics` (`src/analytics.ts:15`).

## How Heric should mirror this

| Insomnia area | Heric target |
|---|---|
| `packages/insomnia-data/src/{models,services,database}` (contracts, 54 model modules, `AllTypes`/`BaseModel` zod schemas) | `crates/heric-model` (pure types/IDs/validation), trait surface re-exported by `crates/heric-core` |
| `packages/insomnia-data/node-src` (NeDB, init-model repairs, migrations) | `crates/heric-storage` (new native store + Insomnia NeDB reader/migrator) |
| `packages/insomnia/src/network/network.ts`, `src/main/network/*` (libcurl pipeline, timeline, cookies, proxy, certs) | `crates/heric-http`; orchestration in `crates/heric-core` |
| `packages/insomnia/src/common/render.ts`, `src/templating/*`, `src/common/templating/*`, `src/ui/worker/templating*` | `crates/heric-template` |
| `packages/insomnia/src/scripting/*`, `src/templating/sandbox/*`, `insomnia-scripting-environment/src/objects` | `crates/heric-script` |
| `packages/insomnia/src/main/{network/websocket,socket-io}.ts`, `main/ipc/grpc.ts`, `main/mcp/*` | `crates/heric-protocols` |
| Authentication modules and editors (`src/main/network/o-auth-1|o-auth-2`, `src/main/network/get-auth-header.ts`, `src/common/authentication` consumers) | `crates/heric-auth` |
| `src/main/importers/*`, `src/main/har.ts`, `src/common/import.ts`, `src/common/insomnia-v5.ts` | `crates/heric-import` |
| `generateCodeSnippet`/`getCodeSnippetTargets` IPC using `httpsnippet` (`src/main/ipc/main.ts:573,580`) | `crates/heric-codegen` |
| `insomnia-vcs` + `src/sync/git/*` + `src/main/git-service.ts` | `crates/heric-git` |
| Spectral lint, OpenAPI/Swagger import, design-mode spec (`src/main/lint-process.mjs`, `src/main/bundle-spectral-ruleset.ts`, importers `openapi-3`/`swagger-2`) | `crates/heric-openapi` |
| Main-process services, IPC registries, sync/backup/updater, runtime abstraction (`src/runtimes/*`) | `crates/heric-core` |
| `packages/insomnia/src/{ui,routes,root.tsx}` (React/Codemirror/monaco UI) | `apps/gui` (eframe/egui, binary `heric`) |
| `packages/insomnia-inso` | `apps/cli` (clap, binary `heric-cli`) |
| `insomnia-smoke-test`, `insomnia-test` | Heric's own E2E suite wired in `.github/workflows`, not a crate |
| `insomnia-api`, `src/konnect`, `src/main/cloud-sync`, `src/account`, `src/ui/account` (Kong cloud login, cloud sync, Konnect) | Out of scope per `PLAN.md`; keep interfaces as stubs |
| `insomnia-analytics` (Segment) | Out of scope; if needed, a no-op telemetry trait in `heric-core` |
| `insomnia-component-docs` (Docusaurus) | Covered by the mdBook sites (`docs/book/en|pt-BR|es`) |
| Windows `cpp/insomnia.cpp` secure wrapper | Packaging concern for `apps/gui` release builds (see ADR 0001) |

## Feature list

Priorities: P0 essential, P1 important, P2 optional. Paths are relative to `D:\Projects\Heric\Insomnia`.

### Process, storage and data model

- **Runtime-agnostic data contracts wired once** — `IDatabase` with buffering (`packages/insomnia-data/src/database/types.ts:48-95`), `initDatabase`/`database` Proxy (`packages/insomnia-data/src/database/index.ts:17,30`), `initServices`/`services` Proxy (`packages/insomnia-data/src/services/index.ts:10,18`). Edge case: business code must never import concrete implementations; renderer, main and inso each wire a different implementation. **P0**
- **NeDB persistence** — collections per model type, in-memory mode, change buffering, `db.changes` broadcast to all windows (`packages/insomnia-data/node-src/database/database-nedb.ts:44`; broadcast `packages/insomnia/src/main/database.main.ts:33`). Edge case: plugin window must not open a second NeDB handle, hence an IPC Proxy (`packages/insomnia/src/main/database.plugin-window.ts:4-21`). **P0**
- **Database IPC bridge** — every `IDatabase` method tunnelled over `database.invoke` (`packages/insomnia/src/ui/database.client.ts:11-90`; handler `packages/insomnia/src/main/database.main.ts:18`). Edge case: `init`, `onChange`, `flushChanges` are deliberately no-ops in the renderer. **P0**
- **Model registry and hierarchy** — `AllTypes` sown from zod (`packages/insomnia-data/src/models/base-types.ts:50,65`), `all()/getModel()/canSync()/duplicate` (`packages/insomnia-data/src/models/index.ts:5-70`), descendant map (`:80`). Edge case: models expose `rewriteReferences` for ID remapping on import/duplicate. **P0**
- **Core models** — Request (`.../models/request.ts:30-38` with auth schemas at `:45-80`), Workspace with `collection|design` scope and scratchpad ID (`workspace.ts:6-16,40`), Project/scratchpad (`project.ts:12,80`), Environment with KV pair types incl. `SECRET` (`environment.ts:5-14,24`), Response/timeline (`response.ts:6-14,52`), Organization/scratchpad and Konnect IDs (`organization.ts:3-11`). **P0**
- **Schema migrations** — Insomnia v5 YAML schema versioning and migration chain (`packages/insomnia/src/common/insomnia-schema-migrations/index.ts`, `v5.1.ts`, `schema-version.ts`); v5 import/export (`packages/insomnia/src/common/insomnia-v5.ts:990,1006`; `mcpUrlToInsomniaV5Yaml` at `:1015`). Edge case: imports of any older export must migrate to the current v5.1 schema first. **P0**
- **Scratchpad bootstrap** — settings/stats/scratchpad project and workspace created before any UI (`packages/insomnia/src/entry.main.ts:348-403`); tolerant of duplicate creation races. **P1**
- **Session and vault key storage** — session persisted, private key, vault salt/key (`packages/insomnia/src/common/account/session.ts:6-80`); encrypted environment values with AES-GCM and vault masking (`packages/insomnia/src/common/templating/mask-or-decrypt-vault-data.ts:15`; renderer key ops `packages/insomnia/src/ui/vault-key.client.ts:10-53`). Edge case: masked value is `••••••` and must round-trip without leaking; secrets are registered as confidential values. **P1** (cloud parts out of scope, local vault P1)
- **Secret storage** — `safeStorage`-encrypted strings with silent plaintext fallback when encryption is unavailable (`packages/insomnia/src/main/ipc/secret-storage.ts:14-72`, fallback at `:54-70`). Edge case: on decrypt failure the ciphertext is returned unchanged. **P1**
- **Local file read/write IPC** — `secureReadFile`, `insecureReadFile(WithEncoding)`, `writeFile`, `readDir`, `readOrCreateDataDir`, response-body export (`packages/insomnia/src/main/ipc/main.ts:259-283` interface; `packages/insomnia/src/main/secure-read-file.ts`, `main/safe-fs-write.ts`). Edge case: path traversal and arbitrary body-path writes are statically audited (`packages/insomnia/src/main/templating-worker-database-surface.ts:1-30`). **P0**

### Request execution

- **Send pipeline** — render → plugin transform → auth → curl → cookies → timeline → after-response script → response patch (`packages/insomnia/src/network/network.ts:274,743,804,875,981,1076`; route `sendActionImplementation` `...send.tsx:116`). Edge case: `clientAction` blocks navigation, so long sends are driven from fetchers; collection runner has its own action (`...runner.tsx:946`). **P0**
- **libcurl engine with cancellation** — request options, proxy PAC parsing, header parsing, body-path handling, HTTP version selection, cancel by ID (`packages/insomnia/src/main/network/libcurl-promise.ts:33-104,296,344,553-637`). Edge case: binary body streamed to a file; multipart converted before send. **P0**
- **Request timeline** — timeline file per response plus in-memory execution steps (`packages/insomnia/src/main/network/request-timing.ts:3-37`; IPC `timeline.getPath`/`appendToFile` in `main/ipc/main.ts`). **P1**
- **Inheritance and defaults** — auth, headers, user-agent suppression resolved up the request-group chain (`packages/insomnia/src/network/network.ts:52-114`), default header application (`packages/insomnia/src/network/apply-default-headers.ts`). **P0**
- **Authentication schemes** — Basic, Bearer, API key, Digest, NTLM, AWS, Hawk, ASAP, Netrc, OAuth 1.0, OAuth 2.0 with six grant types incl. MCP (`packages/insomnia/src/main/network/o-auth-2/get-token.ts:48-53`; editors in `packages/insomnia/src/ui/components/editors/auth/*.tsx`; header builder `main/network/get-auth-header.ts`). Edge case: OAuth2 tokens cached per request, PKCE S256/plain, refresh grant, MCP auth flow. **P0**
- **Cookie jar** — per-workspace jar, set-cookie parsing, cookie editing modal (`packages/insomnia/src/common/cookies.ts`, `network/set-cookie-util.ts`, `main/ipc/cookies.ts:66`, `ui/components/modals/cookies-modal.tsx`). **P1**
- **TLS/client certificates/proxy** — CA and client cert models and editors, proxy watcher and per-request proxy options (`packages/insomnia/src/main/proxy.ts:116`; routes `...cacert.*`, `...clientcert.*`). **P1**
- **Runner** — iterate requests/folders, environment overrides, iteration count/data, delay, bail, live progress, result history (`...debug.runner.tsx:152,946`; `ui/components/panes/runner-*`). Edge case: runner folders versus explicit request ordering. **P1**
- **Sensitive value redaction** — registry and collector mask secrets in logs/timelines/exports (`packages/insomnia/src/main/network/sensitive-values-registry.ts:15-30`, `common/network/sensitive-value-collector.ts:290`). **P1**
- **HAR export** — workspaces, requests, current request, rendered request; cookie conversion (`packages/insomnia/src/main/har.ts:32-341`). **P1**

### Protocols

- **GraphQL** — body editor with schema explorer, introspection, query history (`packages/insomnia/src/ui/components/editors/body/graph-ql-editor.tsx:232`, `ui/components/graph-ql-explorer/`); GraphQL-typed responses. **P1**
- **gRPC** — unary/client/server/bidi streaming, proto file management, server reflection, TLS/mTLS (`packages/insomnia/src/main/ipc/grpc.ts:57-131`; `ui/components/panes/grpc-*-pane.tsx`). **P1**
- **WebSocket** — persistent connection events, preview/response panes (`packages/insomnia/src/main/network/websocket.ts:680`; `ui/components/websockets/`). **P2**
- **Socket.IO** — open/emit/listen/off events (`packages/insomnia/src/main/network/socket-io.ts:625`; `ui/components/socket-io/`). **P2**
- **MCP client** — stdio and streamable HTTP transports, OAuth client provider, tools/prompts/resources, elicitation and sampling forms (`packages/insomnia/src/main/mcp/transport-stdio.ts:12`, `transport-streamable-http.ts:29`, `oauth-client-provider.ts:29`, `client-requests.ts:19-140`; UI `ui/components/mcp/`). **P2**
- **SSE / event streams** — `insomnia-event-source` privileged scheme (`packages/insomnia/src/main/api.protocol.ts:18-46`). **P2**

### Templating and scripting

- **Template rendering** — Liquid engine (Nunjucks-compatible syntax) with cached engines, tag detection short-circuit, comment stripping (`packages/insomnia/src/templating/index.ts:25-40`; `common/templating/liquid-engine.ts:21`). Edge case: no template syntax → text returned unchanged; `allowTags: false` rejects `{% %}` for sandboxed render. **P0**
- **Render context** — environment layering (base → sub-env → folder → request), ancestors, confidential-value collection, rendered request/context (`packages/insomnia/src/common/render.ts:129,301,355,600,837,1009`). **P0**
- **Local template tags** — `faker`, `base64`, `now`, `uuid`, `os`, `hash`, `file`, `jsonpath`, `cookie`, `prompt`, `response`, `request` (`packages/insomnia/src/common/templating/local-template-tags.ts:18,35,95,150,170,206,269,290,331,373,477,811`). Edge case: `{% response %}` can send a dependent request; `{% prompt %}` blocks on UI. **P0**
- **Plugin template tags and reload** — tags fetched from main; persistent worker caches them and needs a `reload` message (`packages/insomnia/src/ui/worker/templating.worker.ts:19-31`); renderer handler forwards a per-message DB auth token because the worker has no `window.main` (`packages/insomnia/src/ui/worker/templating-handler.ts:9-35`). **P1**
- **Script execution window** — pre-request/after-response scripts run in a hidden window with 5 s default timeout, busy-state tracking, restart on crash (`packages/insomnia/src/entry.hidden-window.ts:19-66`; `main/window-utils.ts:97-198`). **P0**
- **JS sandbox and security policy** — QuickJS engine, require interceptor, blocked/masked globals, `insomnia.*` objects, vendored allowlisted libs (`packages/insomnia/src/scripting/quickjs-script-engine.ts`, `script-security-policy.ts:1-30`, `templating/sandbox/surface-profiles.ts:5,33`, `host-bridge.ts:101`; vendored list `packages/insomnia/scripts/sandbox-vendored-libs-list.ts`). Edge case: per-surface ceilings (`floor ∪ (declared ∩ ceiling)`) so a plugin cannot self-escalate; bundle plugins bypass profiles. **P1** (P0 if scripts are in M5 scope; treat as P1 per plan)
- **Unit testing feature** — generate Mocha tests from suites, fake `insomnia` global, `pm.test`-style assertions, test result persistence (`packages/insomnia-testing/src/generate/generate.ts:15`, `run/run.ts:143,151`, `run/insomnia.ts:12`; UI routes `...workspace.$workspaceId.test.*`). **P1**
- **Plugin system** — discovery, themes, request/response hooks, actions, sandbox mode, bundling of first-party plugins (`packages/insomnia/src/plugins/index.ts:52-585`, `common/plugins/permissions.ts:62`, `common/plugins/sandbox-mode.ts:63`; bridge `ui/plugins/renderer-bridge.ts`). **P2**

### Import, export, design and codegen

- **Importers** — Insomnia v1–v4, Postman collection and environment, HAR, cURL, Swagger 2, OpenAPI 3, WSDL, Postman data-dump archives; tried sequentially (`packages/insomnia/src/main/importers/importers/index.ts:14-27`; `convert.ts:21`; archive extraction `main/ipc/extract-postman-data-dump.ts`). **P1**
- **Import UX pipeline** — source types file/URI/curl/clipboard/MCP, resource scanning, conflict resolution, import into project/workspace (`packages/insomnia/src/common/import.ts:39,191,335,455,590`; routes `import.scan.tsx`, `import.resources.tsx`; `ui/components/modals/import-modal/`). **P1**
- **OpenAPI lint** — Spectral with custom rulesets, SSRF guard on `$ref` resolution, compiled-ruleset cache, utility process (`packages/insomnia/src/main/lint-process.mjs:1-40`, `main/bundle-spectral-ruleset.ts`, `main/spectral-ruleset-cache.ts`; duplicated guard in `packages/insomnia-inso/src/commands/lint-specification.ts:19-30`). **P1**
- **Spec export** — export with `--skipAnnotations` stripping `x-kong-` keys (`packages/insomnia-inso/src/commands/export-specification.ts:20-40`; route `...workspace.$workspaceId.spec.update.tsx`). **P1**
- **Design mode** — spec viewer/editor, generate request collection from spec (`...workspace.$workspaceId.spec.tsx:7`, `...spec.generate-request-collection.tsx`). **P2**
- **Mock server** — mock routes and servers, AI-generated routes through a utility process, route CRUD (`...workspace.$workspaceId.mock-server.*`; `packages/insomnia/src/main/mock-generation-process.mjs:1-60`). **P2**
- **Code snippet generation** — httpsnippet targets and per-client snippets over IPC (`packages/insomnia/src/main/ipc/main.ts:573,580`; modal `ui/components/modals/generate-code-modal.tsx`). **P2**
- **cURL paste import** — global keyboard paste handling (`packages/insomnia/src/ui/hooks/use-global-curl-paste-import.ts`; modal `paste-curl-modal.tsx`). **P1**

### Git and sync

- **Git sync service** — clone/init, branch, checkout, fetch, pull/push, commit(s), stage/unstage, partial staging, discard, diff, log, merge/reset, relocate, migrations, credential validation, repository tree, file issues (`packages/insomnia/src/main/git-service.ts:800,1352,1445,1923,4231`; routes `git.*.tsx`, 29 files; `sync/git/git-vcs.ts`, `project-routable-fs-client.ts:31`). Edge case: managed folder slugs are backfilled once before any window exists (`main/git-service.ts:3968`). **P1**
- **Git providers** — GitHub/GitLab OAuth sign-in, repository and email listing (`routes/git-provider.*`, `git-credentials.*`; `main/git/*`). **P2**
- **VCS snapshot store** — staging, branches, snapshots, deltas, merge conflicts with resolution sources, compression hook, filesystem and in-memory drivers (`packages/insomnia-vcs/src/vcs.ts:96`, `store/current-store.ts:7`, `store/index.ts:15`, `store/drivers/file-system-driver.ts:13`, `types.ts:3-99`). **P2** (cloud sync out of scope; the store is still the model for local history)
- **Cloud sync** — backend project init, pull, sync IPC handler, conflict prompts (`packages/insomnia/src/main/cloud-sync/initialization.ts:11,32`, `vcs.ts:45-151`, `sync-ipc-handler.ts:23`). Edge case: pending merge conflicts are resolved through a renderer round-trip. **Out of scope**

### UI shell

- **App shell and modals** — root layout, error boundary, modal registry and `showModal`, toasts, command palette (`packages/insomnia/src/root.tsx:121,812`; `ui/components/modals/index.ts:1-40`; `ui/components/command-palette.tsx:46`; `ui/hooks/use-command-search.ts:22`). **P0**
- **Sidebar and navigation** — project/workspace/request tree, drag and drop, filtering, pinning, context menus, sync bar (`packages/insomnia/src/ui/components/sidebar/project-navigation-sidebar/project-navigation-sidebar.tsx:1280`, `use-sidebar-drag-and-drop.tsx`, `sync-bar.tsx:10`). Edge case: delete fallback must select a neighbouring tab (`ui/components/tabs/request-delete-fallback.ts`). **P0**
- **Request pane** — method/URL bar, tabs for params/headers/body/auth/scripts/docs, body editors (raw, form, url-encoded, file, GraphQL) (`ui/components/panes/request-pane.tsx:44`, `ui/components/editors/body/body-editor.tsx:47`, `raw-editor.tsx:15`, `url-encoded-editor.tsx:17`, `file-editor.tsx:11`). **P0**
- **Response pane** — status/time/size, tabs for body/headers/cookies/timeline/preview/tests, viewers for JSON/XML/HTML/CSV/PDF/image/multipart/error (`ui/components/panes/response-pane.tsx:43`; `ui/components/viewers/response-viewer.tsx:72`, `response-timeline-viewer.tsx:11`, `response-web-view.tsx:8`). **P0**
- **Code editors** — CodeMirror 5 based editors for all text surfaces, undo/redo routed through the renderer to reconcile with the native menu (`ui/components/editors/*`; listeners `ui/renderer-listeners.ts:14-15`). **P0**
- **Environment UI** — environment picker, key-value editor with secret type, global environment selection (`ui/components/environment-picker.tsx:58`, `ui/components/editors/environment-key-value-editor/`, routes `...environment.*`). **P0**
- **Settings** — tabs `data, keyboard, themes, plugins, general, proxy, credentials, ai, scripting` (`ui/components/modals/settings-modal.tsx:34-44`; panels in `ui/components/settings/`). **P0**
- **Themes** — 18 built-in themes, plugin themes, light/dark/auto color scheme application (`packages/insomnia/src/plugins/themes/*`, `ui/plugins/misc.ts:218-260`). **P1**
- **Keyboard shortcuts and menus** — native menu with zoom levels/resize presets/sidebar toggle/devtools, global shortcut hook, hotkey help (`main/window-utils.ts:320-793`; `ui/hooks/use-global-keyboard-shortcuts.ts:10`). **P1**
- **Onboarding and scratchpad tutorial** — first-run wizard, onboarding version key, tutorial pane (`routes/onboarding.$.tsx`, `ui/components/panes/scratchpad-tutorial-pane.tsx`, `common/constants.ts` onboarding keys). **P2**
- **Git folders as documents / deep links** — `insomnia://app/open-folder?path=`, macOS folder document type, second-instance forwarding (`main/entry.main.ts:244-256,299-330`; `electron-builder.config.js:55-66`). **P2**

### Platform and operations

- **Auto-update** — electron-updater with channel URL, status events, disabled on Linux and Windows portable (`packages/insomnia/src/main/updates.ts:44,45,98`; `common/constants.ts` `updatesSupported`). **P2**
- **Backup before upgrade** — version-aware backup and restore (`packages/insomnia/src/main/backup.ts:13,39,63`). **P2**
- **Logging** — main and renderer log files, log directory, Sentry initialization (`packages/insomnia/src/main/log.ts:10,51`; `main/sentry.ts`; `ui/sentry.ts`). **P1**
- **Analytics** — Segment events from main and renderer, organization scoping, page views (`packages/insomnia/src/main/analytics.ts:22-102`, `ui/analytics.ts:3-51`, `packages/insomnia-analytics/src/analytics.ts:44`). **Out of scope (telemetry)**
- **Security hardening** — main window `nodeIntegration:false`/`contextIsolation:true` pinned by tests (`main/window-security.ts:1-19`), default-deny permission policy (`main/permission-policy.ts:1-8`), trusted-origin navigation guard (`main/trusted-origin.ts`, `main/window-utils.ts:269-285`), plugin name validation and tarball host allowlist (`main/create-plugin.ts:9-40`, `main/install-plugin.ts:24-26`), Origin header stripping (`entry.main.ts:334-335`). **P1**
- **Windows secure wrapper** — DLL-hijack mitigation built from `packages/insomnia/src/cpp/insomnia.cpp` (`cpp/README.md`). **P2**

### CLI

- **Run collection** — workspace by ID/name, request/folder selection, env/globals overrides, iteration data (JSON/CSV), delay, bail, proxy/TLS options, JSON output report with redaction requiring `--acceptRisk` (`packages/insomnia-inso/src/cli.ts:494-890`; risk gate `:578`; report `commands/run-collection/result-report.ts:30`). Edge case: globals from a Git project must be passed by file path. **P1**
- **Run tests** — suite/spec selection, env, name pattern, reporter, bail, keep file, timeout, cert/proxy options (`cli.ts:354-492`). **P1**
- **Lint spec** — Spectral with custom ruleset file, SSRF guard (`cli.ts:896-957`; `commands/lint-specification.ts:1-30`). **P1**
- **Export spec** — to stdout or file, `--skipAnnotations` (`cli.ts:962-1005`). **P1**
- **Scripts from `.insorc`** — named tasks that must start with `inso`, pass-through args parsed with shellwords (`cli.ts:1008-1039`, guard at `:1024`). **P2**
- **Config discovery** — cosmiconfig `.insorc` with options and scripts (`cli.ts:76-105`). **P2**
- **Binary packaging** — `pkg` single binary plus verification script (`package.json:43-44`; `src/scripts/verify-pkg.js`). **P2**

### Testing infrastructure

- **Playwright smoke suite** — three projects (Smoke/Critical/Migration) against dev, build or packaged app; echo and Vite web servers; `.vite-port` reuse (`packages/insomnia-smoke-test/playwright.config.ts:47-60`; launcher `playwright/launch.ts:36`; fixtures and per-test data path `playwright/paths.ts`). Edge case: Windows teardown kills the whole process tree (`playwright/test.ts:10-36`). **P1**
- **Second E2E harness** — page objects, flows, coverage merging, enforced single worker (`packages/insomnia-test/playwright.config.ts:38-49`; fixtures `misc/fixtures.ts:81`). **P2**
- **Mock servers for tests** — REST, GraphQL, gRPC (+reflection), WebSocket, Socket.IO, OAuth/OIDC, basic auth, mTLS, Git HTTP backend, cloud APIs (`packages/insomnia-smoke-test/server/*.ts`; mirrored in `packages/insomnia-test/misc/server/`). **P1**
- **Schema freshness gates** — generated JSON schema and autocomplete snippets checked in CI (`(.github/workflows/test.yml:58-70)`; generator `packages/insomnia/scripts/generate-schema.ts`). **P2**
