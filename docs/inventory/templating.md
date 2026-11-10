# Templating

Source of truth: `D:/Projects/Heric/Insomnia/packages/insomnia/src/` (read-only). Paths below are written relative to that root unless prefixed. Insomnia is the original app; nothing here is copied from Heric.

The feature is still *named* Nunjucks everywhere (types `NunjucksParsedTag` / `NunjucksParsedTagArg`, CodeMirror mode `nunjucks`, `extractNunjucksTagFromCoords`), but the engine is **LiquidJS** (`liquidjs ^10.27.0`, `packages/insomnia/package.json:133`). Nunjucks survives only to the extent the compatibility suite pins it. A rewrite must implement Liquid semantics, not Nunjucks semantics.

## Engine and sandbox

### Two render paths

| Path | Entry point | Engine builder | Tag factory |
|---|---|---|---|
| Main/Node (send, inso, vitest) | `templating/index.ts:25` `render(text, config)` | `common/templating/liquid-engine.ts:21` `buildLiquidEngine` | `templating/liquid-extension.ts:23` `createLiquidTag` — calls `ext.run` in-process |
| Renderer Web Worker | `ui/worker/templating-handler.ts:35` `renderInWorker` -> `ui/templating/worker.ts:23` `render` | same `buildLiquidEngine` | `common/templating/liquid-extension-worker.ts:65` `createLiquidTagWorker` — every call routed to main over IPC |

Selection is by runtime adapter, not a flag:
- `runtimes/templating/templating-adapter.node.ts:3-11` dynamic `import('../../templating/index')`.
- `runtimes/templating/templating-adapter.renderer.ts:3-6` dynamic `import('../../ui/worker/templating-handler')`.
- `common/render.ts:647` and `:658` call `getRuntime().templating.renderTemplate(...)`, which is why `render()` in `common/render.ts` works in both processes.
- `ui/templating/renderer-safe.ts:15` is a **stub** that returns the input unchanged (renderer linting only); `:10` `reload()` forwards to the worker, `:23` `getTagDefinitions()` merges local tags with renderer-bridge plugin tags. Never use it for real rendering.

### Engine configuration and hardening

`common/templating/liquid-engine.ts:21-68`:

- Delimiters fixed to `{{ }}` / `{% %}` (`:29-32`). `{# #}` comments are stripped *before* parsing by `stripLiquidComments` (`:11`, regex `/\{#[\s\S]*?#\}/g`) — call sites `templating/index.ts:52`, `ui/templating/worker.ts` render path.
- `strictVariables` defaults to **true** (`:26`), overridden per call: `strictVariables: !ignoreUndefinedEnvVariable` (`templating/index.ts:119`, `ui/templating/worker.ts:138`). Undefined variable -> `UndefinedVariableError` -> `RenderError.reason === 'undefined'` (`common/templating/render-error.ts:28-55`).
- `strictFilters: true` (`:34`) — nonexistent filter throws.
- `jsTruthy: true` (`:35`) — `''`, `0`, `[]` are falsy, matching Nunjucks; pinned by tests.
- `ownPropertyOnly: true` (`:36`) — blocks prototype traversal (`constructor`, `__proto__`, `toString`, `hasOwnProperty`).
- `dynamicPartials: false` (`:37`) — interpolated include paths disabled.
- `renderLimit: 10_000` (`:40`) and `memoryLimit: 10_000_000` (`:41`) — hard stop on runaway loops/allocation.
- `include`, `render`, `layout` are re-registered to a `BlockedFileTag` that always throws `"{% include %}, {% render %}, and {% layout %} are disabled. Use the File template tag to read files."` (`:44-55`). All file access must go through the `file` tag -> `window.main.secureReadFile`.
- `debug` filter registered as identity (`:58`) purely for Nunjucks back-compat.
- Tag registration loop (`:63-67`) stores metadata in a `Map`; that map feeds `getTagDefinitions()`.

### Tag execution routing (where a tag's `run()` actually happens)

1. **Built-in tags** (`common/templating/local-template-tags.ts`) — always in-process. In the worker, their `run()` executes inside the worker with `context.util` implemented as IPC calls (`common/templating/liquid-extension-worker.ts:129-184`).
2. **Bundle plugins** (shipped in the app) — `plugin.getBundlePluginTemplateTags` (`main/templating-worker-database.ts:891`) and `plugin.executeBundlePluginTag` (`:919`). Executed in the main process.
3. **User plugins** — `plugin.getUserPluginTemplateTags` (`main/templating-worker-database.ts:941`) and `plugin.executeUserPluginTag` (`:1038`). At `:1051` the handler consults `shouldSandboxPlugin(settings, plugin)`:
   - sandboxed -> `runPluginTagInSandbox(...)` (`:1056-1061`), with `resolveTemplateTagModules` / `resolveTemplateTagCapabilities` from `templating/sandbox/surface-profiles`.
   - elevated/unsandboxed -> `runPluginTag(targetTag.templateTag.run, body)` in-process (`:1068`).
   - unknown plugin/tag -> `Unsupported tag ${tagName} for plugin ${pluginName}` (`:1070`).
4. The worker reaches those handlers through a custom protocol, not Electron IPC: `fetchFromTemplateWorkerDatabase` (`common/templating/liquid-extension-worker.ts:49-63`) fetches `insomnia-templating-worker-database://<path>` with the bearer-ish header `x-insomnia-templating-auth`; token set by `setTemplatingDbAuthToken` (`:45`) from each render message (`ui/worker/templating.worker.ts:42`).

Execution-mode decision table: `common/plugins/sandbox-mode.ts:40-56` returns `'internal'` (bundle plugins), `'unsandboxed'` (sandbox setting off), `'elevated'` (plugin manifest escape hatch), or `'sandboxed'`. `isSandboxEnabled` at `:23`.

### QuickJS sandbox for user plugin tags

`templating/sandbox/plugin-tag-sandbox.ts`:
- `:54-55` `runTagInSandbox(opts)`; `timeoutMs = 10_000` by default, deliberately matching LiquidJS's `renderLimit`.
- `:58` a **fresh** QuickJS context per call, disposed after; `:73` `ctx.runtime.setInterruptHandler(() => Date.now() > deadline)` so a synchronous busy-loop still dies; `:75` `ctx.runtime.setMemoryLimit(32 * 1024 * 1024)`.
- `:81` `installHostBridge(ctx, discover ? rejectingBridge : bridge)` — in discovery mode every host call is rejected, so plugin module top-level code cannot reach the host.
- `:88-90` envelope/`__tagName` injected as globals *before* the bootstrap, which captures them into closure state and deletes the globals (plugin code can't rewrite `grantedModules`).
- Capability model documented in `templating/sandbox/PERMISSIONS.md`: default-deny `insomnia.permissions = { modules: [...], capabilities: [...] }`; baseline `path` and `crypto` always available; `events` and pinned `uuid`/`ajv` are grantable from `templating/sandbox/vendored/`; the sandbox's vendored version may lag but never lead the app's own dependency (CI guardrail).

### Tag argument model

Arguments are re-tokenized from raw source text on every render; no AST is reused.

- `common/templating/tokenize-args.ts:3` `tokenizeArgs(argsStr)`: character scanner. Single/double-quoted strings with backslash escape (`:18-29`, `:38-40`); booleans; numbers matching `^\d*\.?\d*$`; identifiers -> `variable`; anything else -> `expression`. A missing comma swallows the remainder into one argument (pinned by `common/templating/__tests__/utils.test.ts:232`, "handles no commas").
- `common/templating/resolve-arg.ts:5` `resolveArg(arg, ctx, liquid)`: `variable` / `expression` go through `liquid.evalValue`, everything else is used literally.
- `common/templating/utils.ts:132-144` `decodeEncoding`: any string matching `^b64::(.+)::46b$` is base64-decoded. `:115-130` `encodeEncoding` produces that wrapper. Used because JSONPath/XPath filters contain `{`, `}`, quotes that would otherwise break tag parsing — see `encoding: 'base64'` on the response filter arg (`common/templating/local-template-tags.ts:514`) and jsonpath filter (`:299`).
- Arg metadata type: `common/templating/types.ts:182-200` (`type: 'string' | 'number' | 'boolean' | 'variable' | 'expression' | 'enum' | 'file' | 'model'`, plus `encoding`, `defaultValue`, `forceVariable`, `hide(args)`, `validate`, `options`, `model`, `displayName` which may be a function of already-parsed args).
- Round-trip helpers for the tag editor: `tokenizeTag` (`utils.ts:57-68`), `unTokenizeTag` (`:71-90`), `getDefaultFill` (`:93-113`).

## Variable resolution and chaining

### Layer assembly (lowest to highest precedence)

`common/render.ts:129` `buildEnvironmentRenderLayers` pushes layers in this exact order:

| # | Layer | `source` | Cite |
|---|---|---|---|
| 1 | Global (project) environment, root | `global` | `:160-176` |
| 2 | Global environment, sub/child | `global` | `:178-192` |
| 3 | Collection environment, root/base | `collection` | `:194-212` |
| 4 | Collection environment, sub/active | `collection` | `:214-232` |
| 5 | Folder environments, ancestors in order | `folder` | `:234-263` |
| 6 | Iteration / uploaded runner data | `iteration` | `:265-275` |
| 7 | Transient script variables | `transient` | `:277-290` |

`buildRenderContext` (`common/render.ts:355-594`) merges each layer through `renderSubContext` (`:405-445`) in that order, so later layers win. Per-key source attribution is recorded in parallel (`keySource`, `keySourceMeta`, exposed through `getKeysContext` at `:401-404`) and drives the provenance chips in the UI.

Every environment object is ordered with `orderedJSON.order(..., dataPropertyOrder, JSON_ORDER_SEPARATOR)` (`:161-165` and each later layer), so JSON-editor key order is preserved rather than JS insertion order.

### Merge rules

- Skip dangerous keys `__proto__`, `constructor`, `prototype` (`:410-413`).
- Self-recursive variables: before overwriting a string, if the new value references its own key (regex `\{\{ ?KEY[ |][^}]*\}\}` at `:427`), it is rendered against the base context first. Documented example: base `base_url = google.com`, override `base_url = {{ base_url }}/foo` yields `google.com/foo`, where a plain `Object.assign` would keep the literal.
- Template-last key ordering: `_getOrderedEnvironmentKeys` (`:999-1007`) sorts any value containing `{{` or `{%` after plain values, so a single pass resolves `A: x`, `B: {{ A }}` regardless of key order.
- Confidentiality is tracked per key across layers, not per layer: `confidentialityByKey` (`:456-459`) is overwritten by each higher layer, and a layer with no `kvPairData` entry for that key (iteration, transient, raw JSON) sets it to undefined, i.e. public, deliberately clearing a lower layer flag. Masking happens once, at merge time, before any cross-variable substitution (`:465-483`), so a masked value cannot be re-materialised later.
- `getConfidentialityByKey` (`:104-115`) ignores disabled rows and SECRET rows (registered separately) and takes the last enabled non-SECRET row verdict. `maskConfidentialValue` (`:117-127`) recurses arrays/objects and replaces leaves with `models.environment.vaultEnvironmentMaskValue`.

### Three-pass render loop

After layers merge, keys are re-sorted and the *entire final context* is rendered up to 3 times (`common/render.ts:553-577`):

- `for (let i = 0; i < 3; i++)` over `_getOrderedEnvironmentKeys(finalRenderContext)` (`:558-559`).
- Each value renders with `render(value, finalRenderContext, null, 'keep', 'Environment')` (`:568`). `errorMode: 'keep'` means a broken template stays verbatim instead of aborting context construction.
- `skipNextTime[key]` (`:560-573`): when a render returns the input unchanged, that key is skipped on later passes because tag execution may be non-idempotent and expensive.
- Three passes cap chaining at three hops: `a -> b -> c` resolves, a longer chain does not fully resolve.

### Second pass for tag-valued variables

Independent of the loop, `common/render.ts:649-659` re-renders one string when the first pass output contains `{%` **and the original input contained no tag syntax** (`hasNunjucksCustomTagSymbols` at `:633`, check at `:656`). This is the `{{ foo }}` -> `{% uuid 'v4' %}` -> uuid case. Gating on the original input is the injection guard: a response body, file content or OAuth token that happens to contain `{%` is never re-interpreted against the live context.

### Render traversal of request fields

`common/render.ts:600-712` `render(obj, context, blacklistPathRegex, errorMode, name, ignoreUndefinedEnvVariable)`:

- Deep clone first (`:609`); caller data is never mutated.
- Left untouched: `Date`, `RegExp`, `Error`, `Boolean`, `Number`, `Null`, `Undefined` (`:618-630`).
- Strings with none of `{{` / `{%` / `{#` return unchanged (`:632-638`); `''` returns unchanged (`:640-642`).
- Arrays recurse with path `[i]` (`:670-673`).
- Objects with `disabled: true` are skipped entirely (`:674-680`).
- At the first level only, keys starting with `_` reuse the parent path (`:684-688`) — that is how the `{ _request, _cookieJar }` wrapper in `getRenderedRequestAndContext` (`:893-897`) behaves like top-level fields.
- `blacklistPathRegex` skips matching paths: `request.settingDisableRenderRequestBody ? /^body.*/ : null` (`:899`), gRPC `skipBody` (`:817`).

### Request-level resolution

`common/render.ts:837-...` `getRenderedRequestAndContext`:

- Description is blanked during the main render and re-rendered afterwards at lower priority (`:880-882`).
- Headers/authentication are inherited from request groups first (`:884-886`).
- Only `source === 'manual'` cookies are rendered; server-set cookies are excluded so a response cannot plant a template re-executed on a later request (`:887-891`).
- GraphQL bodies get `#}` -> `# }` before render (`:870-878`, references issue 895) — retained for stored bodies.
- gRPC: `getRenderedGrpcRequest` (`:805-822`) and `getRenderedGrpcRequestMessage` (`:824-835`).

### Context object, purposes, confidentiality

`getRenderContext` (`common/render.ts:715-803`) builds helpers only: `getMeta` (requestId, workspaceId), `getPurpose`, `getExtraInfo`, `getEnvironmentId`, `getGlobalEnvironmentId`, `getProjectId`, `getSettings` (`{ dataFolders, hideSecretValuesInPreviewAndConsole, forceReveal }`), and `getSensitiveValueCollector` only if one was supplied (`:770-787`).

Active-environment selection: `workspaceMeta.activeGlobalEnvironmentId` may point to a root (parent id starts with `wrk_`) or a sub environment; when sub, its parent becomes the root (`:740-756`).

`RenderPurpose = 'send' | 'general' | 'preview' | 'script' | 'no-render'` (`common/templating/types.ts:68`). One policy function, `getConfidentialValuePolicy` (`common/templating/confidential-value-policy.ts:23-42`):

| purpose | hideSecretValues | forceReveal | result |
|---|---|---|---|
| `send`, `script` | any | any | `reveal` |
| `preview` | `false` or any+forceReveal | `true` | `reveal` |
| `preview` | `undefined` or `true` | falsy | `mask` |
| `general`, `no-render`, `undefined` | any | any | `mask` (fail-closed) |

`shouldMaskExternalVaultTag` (`confidential-value-policy.ts:55-72`) short-circuits only plugin `@kong/insomnia-plugin-external-vault` tag `vault` during `preview` when the policy is `mask`; both tag factories check it before calling `run` (`templating/liquid-extension.ts:108-113`, `common/templating/liquid-extension-worker.ts:186-191`), so no provider fetch happens. `shouldShowRevealToggle` (`:86-92`) keeps the eye toggle visible once a mask was seen.

Vault decrypt/mask: `common/templating/mask-or-decrypt-vault-data.ts:15-63`, never mutating input. Decrypt path registers each plaintext string in the collector (`:39-44`); returns `{}` when the vault is enabled but no session key exists (`:45-48`); non-object passthrough (`:27-29`); mask path replaces every leaf with `vaultEnvironmentMaskValue` (`:55-61`). The sealed key is merged into a runtime `vault` namespace at `common/render.ts:537-550`; a non-object user variable colliding with that reserved name throws (`:535-541`).

Cutting across the worker boundary: `common/templating/render-context-serialization.ts:22-57` flattens helpers into `serializedFunctions` (`requestId`, `workspaceId`, `environmentId`, `extraInfo`, `globalEnvironmentId`, `keysContext`, `projectId`, `purpose`, `settings`, `renderId`) and rebuilds them on the far side; functions are not structured-cloneable.

For `send`, rendered **normal** confidential values are registered for timeline redaction (`common/render.ts:580-587`); external vault results register at the tag call site (`templating/liquid-extension.ts:117-120`; worker posts `{type:'sensitiveValue', id, value}` instead — `common/templating/liquid-extension-worker.ts:195-204`, received by `ui/worker/templating-handler.ts:51-55`).

### Engine caching and invalidation

- Main: `getLiquid` caches `liquidAll` + metadata only when `ignoreUndefinedEnvVariable` is falsy (`templating/index.ts:98-130`); `reload()` nulls both (`:73-76`). Because `strictVariables` is part of engine construction, loose-mode renders always build a fresh engine.
- Worker: same policy in `ui/templating/worker.ts:112-...`, cache cleared by `reload()` (`:71-74`). The worker's `reload` message additionally calls `plugin.reloadPlugins` and swaps the auth token, because the worker can otherwise never invalidate its baked-in plugin set (`ui/worker/templating.worker.ts:28-36`).
- Tag metadata surfaced to the editor: `getTagDefinitions()` filters `deprecated`, sorts by `priority`, and normalises `liveDisplayName` / `disablePreview` / `args` / `actions` (`templating/index.ts:81-96`; worker twin `ui/templating/worker.ts:79-94`; renderer twin `ui/templating/renderer-safe.ts:23-52`). Priorities default to registration index (`templating/index.ts:113-116`), user plugins spread after local tags so they can override a built-in by name (`:107-111`).
- UI caches render contexts per editor for 5 s (`ui/context/nunjucks/use-nunjucks.ts:82`); `tag-editor.tsx:93` and `variable-editor.tsx:38` always render previews with `purpose: 'preview'` plus `forceReveal` from the eye button.

## Custom tags

All twelve built-ins live in `common/templating/local-template-tags.ts` and are exported as `localTemplateTags: TemplateTag[]` (`:1018-1033`), each wrapped in a synthetic plugin `{ name: <tag name>, description: 'Built-in plugin', version: '0.0.0', directory: '', permissions: {} }`. Syntax is `{% name 'arg', 'arg' %}`; unquoted identifiers are evaluated as variables (see "Tag argument model"). Results are converted with `String(result)`, with `null`/`undefined` becoming `''` (`templating/liquid-extension.ts:115-121`).

| Tag | Arguments (in order) | Behavior | Source |
|---|---|---|---|
| `faker` | `Function` (enum of 117 names from `fakerFunctions`) | Returns `fakerFunctions[name]()`. Keys are the exported object in `packages/insomnia-data/common-src/faker-functions.ts:3` (`guid`, `timestamp`, `isoTimestamp`, `randomUUID`, `randomAlphaNumeric`, `randomBoolean`, `randomInt`, `randomColor`, `randomHexColor`, ...). An unknown name throws a `TypeError` from the missing function call; there is no validation. | `:17-31`; `packages/insomnia-data/common-src/faker-functions.ts:3` |
| `base64` | `Action` (`encode`/`decode`), `Kind` (`normal`/`url`/`hex`), `Value` | `normal` encode: UTF-8 -> binary string -> `btoa`. `hex` encode: hex pairs -> bytes -> `btoa`. `url` encode: normal then `+`->`-`, `/`->`_`, trailing `=` stripped. Decode reverses, including hex output as lowercase. Invalid action/kind fail an `invariant`. Odd-length hex input silently drops the trailing nibble (`/.{1,2}/g`). | `:34-92` |
| `now` | `Timestamp Format` (`iso-8601`/`millis`/`unix`/`custom`), `Custom Format Template` (hidden unless `custom`) | Aliases: `millis|ms`, `unix|seconds|s`, `iso-8601`, `custom`; anything else throws `Invalid date type "<x>"`. `custom` uses date-fns `format` (import at `:1`), placeholder `MMMM Do YYYY, h:mm:ss a`. Always returns a string. | `:94-146` (aliases `:125-143`) |
| `uuid` | `Version` (`v4`/`v1`) | Always `crypto.randomUUID()`. **`v1` is accepted but still yields a v4 UUID**; the argument is ignored and this is pinned by a test. | `:148-166` |
| `os` | `Function` (`arch`/`cpus`/`freemem`/`hostname`/`platform`/`release`/`userInfo`), `JSONPath Filter` (hidden unless `userInfo`/`cpus`) | Calls `context.util.nodeOS()[fn]`; non-string values are `JSON.stringify`-ed. The JSONPath argument is UI-only and is **not** applied in `run`. | `:168-203` |
| `hash` | `Algorithm` (`md5`/`sha1`/`sha256`/`sha512`), `Digest Encoding` (`hex`/`base64`), `Input` | Non-string input throws `Cannot hash value of type "<t>"`; invalid encoding throws `Invalid encoding <e>. Choices are hex, base64`. `md5` logs a broken-crypto warning and delegates to `context.util.encode(value, encoding)`; `sha*` use `crypto.subtle.digest`, with unrecognized algorithms falling back to SHA-256. | `:205-266` |
| `file` | `Choose File` (`type: 'file'`) | Returns `context.util.readFile(path)`; missing path -> `No file selected`; there is **no direct fs fallback**. Main path calls `window.main.secureReadFile` (`templating/liquid-extension.ts:62`); worker path calls the `readFile` IPC handler (`main/templating-worker-database.ts:655`). Allowlist = `os.tmpdir()`, userData `responses/` and `version-control/`, plus user-configured folders; NeDB `insomnia.*.db` files are refused even inside allowed roots, including hard-link aliases (`main/secure-read-file.ts:36-60`). | `:268-286` |
| `jsonpath` | `JSON string`, `JSONPath Filter` (**`encoding: 'base64'`**) | Plain `JSON.parse` (large integers lose precision here, unlike the response tag); invalid JSON -> `Invalid JSON: <msg>`; bad query -> `Invalid JSONPath query: <f>`; no match -> `JSONPath query returned no results: <f>`; returns only the **first** result. | `:288-328` (`:299` encoding) |
| `cookie` | `Cookie Url`, `Cookie Name` | Missing `requestId`/`workspaceId` meta returns `null`. With a URL: `cookieJar.getCookiesForUrl(workspaceId, url)`; without: the whole jar. A miss throws `No cookie with name "<n>" found in cookie jar for url "<u>"` followed by the available names. Returns `cookie.value`. | `:330-369` |
| `prompt` | `Title` (required, validated), `Label`, `Default Value`, `Storage Key`, `Mask Text` (bool), `Default to Last Value` (bool, default true) | Empty title throws `Title attribute is required for prompt tag`. Storage key defaults to `${requestId}.${title}`. An explicit key with a cached value returns immediately. Any non-`send` purpose never prompts: returns the cached value if non-null, else `defaultValue || ''`. On `send` it calls `context.app.prompt(title, { label, defaultValue, inputType: maskText ? 'password' : 'text' })`; a `null` (cancel) does **not** overwrite the cache. `disablePreview` when Mask Text is true (`:376`); a `Clear` action calls `context.store.clear()` (`:413-421`). | `:371-474` (`:428-472`) |
| `response` | `Attribute` (`body`/`raw`/`header`/`url`), `Request` (`type: 'model'`, model `Request`), `Filter` (base64; label becomes `Filter (JSONPath or XPath)` / `Header Name` / `Filter`), `Trigger Behavior` (`never` default / `no-history` / `when-expired` / `always`), `Max age (seconds)` (number, for `when-expired`) | See the dedicated section below. | `:477-807` (args `:482-568`, run `:570-806`) |
| `request` | `Attribute` (`name`/`folder`/`url`/`parameter`/`header`/`cookie`/`oauth2`/`oauth2-identity`/`oauth2-refresh`), `Name` (label varies; hidden for url/oauth2*/name/folder), `Parent Index` (number, only for `folder`) | Missing meta returns `null`. `url`: renders `request.url`, appends every rendered query parameter via `URL.searchParams`, returns a `URL`. `cookie`: jar lookup with the same error shape as the cookie tag. `parameter`/`header`: case-insensitive match against rendered names, returns the rendered value; no candidates -> `No query parameters available` / `No headers available`; no match -> lists choices. `oauth2*`: `oAuth2Token.getByRequestId`; without an access token -> `No OAuth 2.0 access tokens found for request`. `name`: raw `request.name`, not rendered. `folder`: `request.getAncestors()` excluding self, index `folderIndex \|\| 0`, out of range -> `Could not get folder by index <i>. Must be between 0-<n-1>`. Anything else -> `null`. | `:809-1015` (run `:896-1013`) |

### `{% response %}` in detail

`local-template-tags.ts:570-806`, in execution order:

1. Defaults: `filter = filter || ''`, `resendBehavior = (resendBehavior || 'never').toLowerCase()` (`:571-572`).
2. Attribute must be `body|header|raw|url`, else `Invalid response field <field>` (`:574-576`); no id -> `No request specified` (`:577-580`); unresolvable request -> `Could not find request <id>` (`:582-585`).
3. Resend decision (`:587-627`): `never` and `no-history` resend only when no stored response exists; `when-expired` resends when `ageSeconds > maxAgeSeconds`; `always` always; any other value sets `shouldResend = false`.
4. Cycle guard: if `context.context.getExtraInfo()?.requestChain` already contains the request id, resend is forced off (`:629-633`). Otherwise the id is pushed and `context.network.sendRequest(request, { requestChain, environmentId })` is called (`:635-642`).
5. **Resend executes only when `context.renderPurpose === 'send'`** (`:635`); preview/general reuse the latest stored response and fail if none exists.
6. Failures: no response -> `No responses for request`; `response.error` -> `Failed to send dependent request <e>`; falsy `statusCode` -> `No successful responses for request` (`:644-657`).
7. `body`/`header` require a filter -> `No <field> filter specified` (`:659-661`); the filter is trimmed (`:663`).
