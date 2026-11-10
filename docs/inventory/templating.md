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
| `now` | `Timestamp Format` (`iso-8601`/`millis`/`unix`/`custom`), `Custom Format Template` (hidden unless `custom`) | Aliases: `millis\|ms`, `unix\|seconds\|s`, `iso-8601`, `custom`; anything else throws `Invalid date type "<x>"`. `custom` uses date-fns `format` (import at `:1`), placeholder `MMMM Do YYYY, h:mm:ss a`. Always returns a string. | `:94-146` (aliases `:125-143`) |
| `uuid` | `Version` (`v4`/`v1`) | Always `crypto.randomUUID()`. **`v1` is accepted but still yields a v4 UUID**; the argument is ignored and this is pinned by a test. | `:148-166` |
| `os` | `Function` (`arch`/`cpus`/`freemem`/`hostname`/`platform`/`release`/`userInfo`), `JSONPath Filter` (hidden unless `userInfo`/`cpus`) | Calls `context.util.nodeOS()[fn]`; non-string values are `JSON.stringify`-ed. The JSONPath argument is UI-only and is **not** applied in `run`. | `:168-203` |
| `hash` | `Algorithm` (`md5`/`sha1`/`sha256`/`sha512`), `Digest Encoding` (`hex`/`base64`), `Input` | Non-string input throws `Cannot hash value of type "<t>"`; invalid encoding throws `Invalid encoding <e>. Choices are hex, base64`. `md5` logs a broken-crypto warning and delegates to `context.util.encode(value, encoding)`; `sha*` use `crypto.subtle.digest`, with unrecognized algorithms falling back to SHA-256. | `:205-266` |
| `file` | `Choose File` (`type: 'file'`) | Returns `context.util.readFile(path)`; missing path -> `No file selected`; there is **no direct fs fallback**. Main path calls `window.main.secureReadFile` (`templating/liquid-extension.ts:62`); worker path calls the `readFile` IPC handler (`main/templating-worker-database.ts:655`). Allowlist = `os.tmpdir()`, userData `responses/` and `version-control/`, plus user-configured folders; NeDB `insomnia.*.db` files are refused even inside allowed roots, including hard-link aliases (`main/secure-read-file.ts:36-60`). | `:268-286` |
| `jsonpath` | `JSON string`, `JSONPath Filter` (**`encoding: 'base64'`**) | Plain `JSON.parse` (large integers lose precision here, unlike the response tag); invalid JSON -> `Invalid JSON: <msg>`; bad query -> `Invalid JSONPath query: <f>`; no match -> `JSONPath query returned no results: <f>`; returns only the **first** result. | `:288-328` (`:299` encoding) |
| `cookie` | `Cookie Url`, `Cookie Name` | Missing `requestId`/`workspaceId` meta returns `null`. With a URL: `cookieJar.getCookiesForUrl(workspaceId, url)`; without: the whole jar. A miss throws `No cookie with name "<n>" found in cookie jar for url "<u>"` followed by the available names. Returns `cookie.value`. | `:330-369` |
| `prompt` | `Title` (required, validated), `Label`, `Default Value`, `Storage Key`, `Mask Text` (bool), `Default to Last Value` (bool, default true) | Empty title throws `Title attribute is required for prompt tag`. Storage key defaults to `${requestId}.${title}`. An explicit key with a cached value returns immediately. Any non-`send` purpose never prompts: returns the cached value if non-null, else `defaultValue \|\| ''`. On `send` it calls `context.app.prompt(title, { label, defaultValue, inputType: maskText ? 'password' : 'text' })`; a `null` (cancel) does **not** overwrite the cache. `disablePreview` when Mask Text is true (`:376`); a `Clear` action calls `context.store.clear()` (`:413-421`). | `:371-474` (`:428-472`) |
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
8. Body decoding (`:664-666`, `:681-692`, `:705-716`): charset taken from the response `contentType`'s `charset=` parameter, default `utf8`; a decode failure is logged and falls back to a raw UTF-8 conversion instead of throwing.
9. `url` (`:667-680`): if the stored URL contains the confidential mask, throws `The response URL was redacted because it contained confidential values. Turn off "Hide Confidential values in Preview/Console" in Preferences to chain it.` This is deliberate: response documents are redacted before persistence, so chaining would silently send `••••••`.
10. `raw` (`:681-692`): returns the decoded body; a string-typed body buffer (the read-failure signal) throws that string as a `TypeError`.
11. `header` (`:693-703`): case-insensitive match; no headers -> `No headers available`; miss -> lists all header names.
12. `body` with a filter starting with `$` -> JSONPath (`:718-752`): parsed with **JSONBig** (`JSONBigStringParser`) so integers beyond IEEE-754 keep every digit; invalid JSON -> `Invalid JSON: <msg>`; invalid query -> `Invalid JSONPath query: <f>`; no results -> `Returned no results: <f>`; a single result returns the string itself or `JSON.stringify` of a non-string; **multiple results return a JSON array string**.
13. `body` with any other filter -> XPath (`:753-803`), via `@xmldom/xmldom` `DOMParser` plus `xpath.select`. Scalar function results (`count()`, `sum()`, `boolean()`, `string()`) become their string form. Node results are filtered to attribute/element/text nodes; the returned `inner` is `nodeValue` for attributes, the child-node string for elements, the trimmed text for text nodes. Zero nodes -> `Returned no results: <f>`; more than one -> `Returned more than one result: <f>`; any other failure is rethrown as `Invalid XPath query: <f>` because the outer catch swallows the specific error. The unreachable tail throws `Oops`.
14. The XPath path needs the jsdom test environment because it compares against the global `Node.*` constants (`common/templating/__tests__/local-template-tags.test.ts:1-4`).

Rendering implications: `prompt` can never prompt outside `send` (`:453-458`), so preview rendering must use stored/default values and never block on UI input. `context.app.prompt` is itself purpose-gated in the plugin bridge (`plugins/context/app.ts:55`).

### Tags coming from plugins

- `vault`, bundled `@kong/insomnia-plugin-external-vault` (`common/constants.ts:352`). It is an ordinary plugin tag plus the preview masking short-circuit (`shouldMaskExternalVaultTag`).
- Bundled `@kong/insomnia-plugin-ai`: discovered like any bundle plugin, with no special handling in the templating layer.
- User-installed plugin tags: arbitrary names, executed per the routing rules above.

## Custom filters

Only **one** filter is registered by Insomnia:

- `debug` — identity function, registered for Nunjucks back-compat at `common/templating/liquid-engine.ts:58`, pinned by `templating/__tests__/liquid-compat.test.ts:98-101`.

Everything else is a stock LiquidJS filter. The compatibility suite pins these specifically (`liquid-compat.test.ts:74-102`):

| Filter | Form | Pinned output | Cite |
|---|---|---|---|
| `upcase` | `{{ s \| upcase }}` | `hello` -> `HELLO` | `liquid-compat.test.ts:76-79` |
| `downcase` | `{{ s \| downcase }}` | `HELLO` -> `hello` | `:76-79` |
| `default` | `{{ x \| default: 'fallback' }}` | fallback when `x` undefined — **requires `ignoreUndefinedEnvVariable: true`**, because `strictVariables` would otherwise throw first | `:81-87` |
| `replace` | `{{ s \| replace: 'a', 'b' }}` | colon syntax, not parentheses; `abc` -> `bbc` | `:89-92` |
| `size` | `{{ s \| size }}` | returns the string `'5'` (not a number) | `:94-97` |

Chaining works (`{{ s | upcase | downcase }}` -> `hello`, `:240-243`). No Insomnia-specific filters implement hashing, base64, dates, or randomization: those are all template tags, not filters.

## Nunjucks syntax actually used by Insomnia

There is no separate Nunjucks parser. Everything goes through LiquidJS with two accommodations: `{# ... #}` comments are regex-stripped before parsing, and `_` is a second copy of the whole context.

| Construct | Form | Where produced / required |
|---|---|---|
| Variable interpolation | `{{ name }}`, `{{ _.name }}`, `{{ _['my-var'] }}`, `{{ obj.prop }}`, `{{ arr[0] }}` | Both root and `_` forms work because the context is spread twice (`templating/index.ts:44`; worker equivalent in `ui/templating/worker.ts`). Brackets are required for dashes/spaces. |
| Tag invocation | `{% name 'arg', 'arg' %}` — comma separated, quotes conventional | `common/templating/utils.ts:71-90` `unTokenizeTag` generates exactly this. |
| Base64-wrapped argument | `{% jsonpath '...', 'b64::<base64>::46b' %}` | `utils.ts:115-144`; used where a filter contains `{}` or quotes. |
| Comment | `{# ... #}` | Stripped by `stripLiquidComments`; highlighted by the CodeMirror mode. |
| Literal block | `{% raw %} ... {% endraw %}` | Supported and tested (`liquid-compat.test.ts:116-135`); the CodeMirror mode tracks `inRaw`. |
| Importer-generated | `{% faker 'guid' %}` from Postman `{{$guid}}` | `main/importers/importers/postman.ts:62-66`. |
| External vault | `{% vault ... %}` with `externalVaultTag_<32 chars>` ids | `common/templating/utils.ts:170-180` (`externalVaultTagPrefix`, `vaultTagIdRegex`, `replaceVaultTagIdIfNeeded`, `containsExternalVaultTag`). |
| Response chaining detection | `responseTagRegex = /{% *response *.* %}/` | `utils.ts:167`. |

Liquid built-ins that work but are not part of Insomnia's own output: `if`/`elsif`/`else`, `unless`, `case`/`when`, `for` (with `limit:`, `offset:`, `reversed`, `else`, numeric ranges, `forloop.index/first/last`), `assign`, `capture`, `increment`, `decrement`, `echo`, the `{% liquid %}` block, and `tablerow`. All pinned in `liquid-compat.test.ts:322-510`.

**Nunjucks constructs that must NOT be implemented**

| Construct | Why | Test |
|---|---|---|
| `{% elif %}` | Liquid uses `elsif`; `elif` is a parse error | `liquid-compat.test.ts:162-170` |
| `{% set %}` / `{% endset %}` | Nunjucks-only; parse error | `:175-183` |
| `{{ x \| filter(arg) }}` | Liquid uses `filter: arg, arg`; parentheses are not filter syntax | `:185-190` |
| `{% include %}`, `{% render %}`, `{% layout %}` | Deliberately blocked; all throw `... disabled ...` | `:295-320` |

**Semantics that must be preserved exactly**

- JS truthiness: `''`, `0`, `[]` are falsy (`liquid-compat.test.ts:62-72`).
- No HTML escaping anywhere: script tags, SVG handlers and `&lt;` entities pass through verbatim (`:523-557`). This is intentional for raw request bodies; a rewrite must not add escaping.
- `assign` stores a literal: `{% assign v = "{{ secret }}" %}{{ v }}` outputs `{{ secret }}` (`:559-567`).
- `capture` renders once at capture time and is not re-rendered afterwards (`:569-575`).
- Own-property-only access: `constructor`, `__proto__`, `obj.constructor`, `obj.toString`, `obj.hasOwnProperty` all throw (`:253-277`, `:587-611`).
- DoS limits fire: `(1..11000000)` throws (`:613-617`); a 200-filter chain still resolves (`:625-629`).
- `allowTags: false` still allows `{{ }}` but rejects every `{% %}` (`:659-672`). Used so a sandboxed plugin's `util.render` cannot invoke a real tag's unsandboxed `run` (`templating/index.ts:23`, `main/templating-worker-database.ts:375`).
- Empty contexts and one-sided delimiters pass through unchanged (`:235-237`, `:46-48`).

Not present: Nunjucks macros/imports, template inheritance, `{{ x if y }}` inline conditionals, async filters, `call`/`caller`. None are registered or tested.

## Existing tests usable as golden tests

All paths relative to `packages/insomnia/src`.

| File | Size / tests | What it pins |
|---|---|---|
| `templating/__tests__/liquid-compat.test.ts` | 672 lines, 99 tests, 26 describes | The complete syntax contract. Describes: `template tag arguments` (`:7`), `variable interpolation` (`:29`), `control flow` (`:51`), `filters` (`:74`), `comment stripping` (`:104`), `raw blocks` (`:116`), `error handling` (`:137`), `nunjucks breaking changes` (`:162`), `edge cases` (`:193`), `prototype chain isolation` (`:253`), `template injection isolation` (`:278`), `file-loading tags blocked` (`:295`), `unless tag` (`:322`), `case / when tag` (`:339`), `for tag — advanced` (`:368`), `echo tag` (`:438`), `liquid block tag` (`:450`), `increment and decrement tags` (`:468`), `capture and tablerow tags` (`:485`), `XSS: variable output passthrough` (`:523`), `XSS: filter chain passthrough` (`:543`), `assign and capture: no re-evaluation` (`:559`), `prototype pollution resistance` (`:587`), `DoS resistance` (`:613`), `unicode and special byte inputs` (`:633`), `allowTags: false` (`:659`). Highest-value fixture set: most tests are one-line input/expected pairs. |
| `common/templating/__tests__/local-template-tags.test.ts` | 551 lines, 53 tests | Tag behavior. Describes: `response tag` (`:56`; `url attribute` `:60`, `JSONPath body — large integer precision` `:77` incl. 19/20-digit ids and the `MAX_SAFE_INTEGER` boundary, `JSONPath body — floating point` `:117`, `XPath filters` `:149` incl. `count()`, `sum()`, `boolean()` true/false, `string()`, element/attribute/text nodes, zero match, multi match, invalid syntax), `base64 tag` (`:292`, incl. `aGVsbG8=`, `q8Ej`, `ICA-IA`, `ICA_IA`), `hash tag` (`:333`, known SHA-1/SHA-512 digests of `abc`), `now tag` (`:374`), `uuid tag` (`:405`, `v1` still v4), `jsonpath tag — standalone` (`:422`), `prompt tag` (`:444`, explicit-key caching, cancel does not overwrite the cache), `file tag: filesystem access isolation` (`:513`). The whole file is `@vitest-environment jsdom` (`:1-4`). |
| `common/templating/__tests__/utils.test.ts` | 408 lines, 24 tests | `forceBracketNotation`, `normalizeToDotAndBracketNotation`, `getKeys` flattening (functions ignored), `tokenizeTag` (complex tags, whitespace, string/number/boolean/expression, missing commas, default case), `unTokenizeTag` quoting, `encodeEncoding`/`decodeEncoding`, `replaceVaultTagIdIfNeeded` (multiple tags, no-op outside `{% %}`), `extractUndefinedVariableKey`. |
| `common/__tests__/render.test.ts` | 1612 lines, 61 tests | The environment/confidentiality contract. Blocks: `render()` (`:52`), `buildRenderContext()` (`:116`) with legacy no-render masking (`:117`), preview masking preserving JSON shape and derived variables (`:139`), last-enabled KV row plus higher-priority public layers clearing confidentiality (`:182`), higher-priority confidential overriding public (`:215`), reveal when the setting is off (`:240`), disabled rows ignored (`:263`), `script` reveals (`:286`), `forceReveal` bypass (`:309`), no mutation of vault data (`:332`) or env data (`:354`); `collectConfidentialRawValues()` (`:891`); `registerConfidentialValues()` (`:1103`, script-written confidential keys); `render()` object/error cases (`:1177`: simple and complex objects, bad template, `keep` mode, error path, private first node); `getRenderedGrpcRequestMessage()` (`:1301`); `getRenderedGrpcRequest()` (`:1329`); `getRenderedRequestAndContext()` (`:1418`: Basic Auth header registration, no scheme prepended to a masked URL); `getRenderContext()` (`:1490`: project-environment attribution). |
| `common/templating/confidential-value-policy.test.ts` | 103 lines, 11 tests | Purpose × hideSecretValues matrix via `it.each` (`:38-46`), `forceReveal` only for preview (`:47`), `shouldMaskExternalVaultTag` false for other plugins/tags/purposes (`:58-66`), true for preview+setting ON (`:73`), false for preview+setting OFF (`:79`), false under forceReveal (`:83`), `shouldShowRevealToggle` (`:91-99`). |
| `common/templating/__tests__/mask-or-decrypt-vault-data.test.ts` | 72 lines, 5 tests | Decrypt into a new object leaving ciphertext untouched (`:20`), return `{}` when the vault is enabled without a key (`:37`), copy when the vault is disabled (`:47`), mask into a new object (`:58`), non-object passthrough (`:68`). |
| `common/templating/__tests__/render-context-serialization.test.ts` | 77 lines, 4 tests | Functions stripped so the result is cloneable (`:22`), env data preserved and helpers flattened into `serializedFunctions` (`:30`), helpers rebuilt after a clone round-trip (`:50`), `forceReveal` survives the boundary (`:62`). |
| `ui/templating/__tests__/worker.test.ts` | 94 lines, 2 tests | `_.variable` argument resolves to its value (`:51`); a user plugin tag is registered and its execution routed to main (`:63`). |
| `ui/worker/__tests__/templating.worker.test.ts` | 174 lines, 2 tests | A user plugin missing when the engine was built appears after a `reload` (`:88`); the external vault result is posted back under the render id (`:162`). |
| `ui/worker/__tests__/templating-handler.test.ts` | 72 lines, 1 test | A `sensitiveValue` message is registered into the collector while the render still resolves, and `renderId` reaches `serializedFunctions` (`:42-72`). |
| `templating/sandbox/plugin-tag-sandbox.test.ts` | 913 lines, 68 tests | Sandbox parity and gating: base64 parity (`:104`), `os` async bridge round-trip (`:127`), `crypto` parity (`:156`), module registry gating (`:301`), manifest-declared grants (`:441`), stdlib globals (`:490`), `Buffer` parity (`:505`), `process` stub (`:530`), Web Crypto subset (`:559`), `URL` (`:588`), identity marker (`:620`), `URLSearchParams` (`:635`), raw bridge unreachable (`:672`), `createMapBridge` (`:695`), capability gating (`:705`), capability-aware context (`:806`), plus timeout (`:273`) and 32 MB memory-limit (`:286-294`) cases. |
| `konnect/__tests__/transform.test.ts` | 17.1 KB | `sanitizeRoute` strips `{{ }}`/`{% %}` from externally sourced route data repeatedly until stable (`konnect/transform.ts:12-20`) and nulls fully stripped arrays (`:23-29`); cases at `:15-90`. |
| `main/importers/importers/postman.test.ts` | 19.8 KB | Postman `{{$tag}}` -> `{% faker 'tag' %}` mapping (implementation `main/importers/importers/postman.ts:62-66`). |
| `common/__tests__/har.test.ts` | 20.1 KB | Purpose handling at export boundaries: `purpose: 'preview'` masks confidential env values with the setting ON (`:447-477`); explicit export passes no purpose and masks regardless of the setting (`:478-510`); a masked host and Basic Auth header stay masked (`:526`). |
| `ui/utils/render-realtime-connect.test.ts` | 244 lines | Realtime connect relies on `tryToInterpolateRequestAndCollectSensitiveValues` and returns `sensitiveValues` for redaction (`ui/utils/render-realtime-connect.ts:15-34`). |

## Feature list

Priority reflects what a Heric user notices first and what blocks otherwise-correct request sending. P0 = required for any usable implementation; P1 = parity in common workflows; P2 = platform/editor/import extras that can ship later.

### P0 — core rendering

| # | Feature | Cite | Edge cases / notes |
|---|---|---|---|
| 1 | LiquidJS engine with `strictVariables` (default on), `strictFilters`, `jsTruthy`, `ownPropertyOnly`, `dynamicPartials: false`, `renderLimit: 10_000`, `memoryLimit: 10_000_000` | `common/templating/liquid-engine.ts:21-42` | `strictVariables` derives from `ignoreUndefinedEnvVariable` per call (`templating/index.ts:119`), and the engine is cached only in strict mode (`:124-127`). Loose mode is what makes the `default` filter usable. |
| 2 | Comment stripping before parse (`{# ... #}`, multiline) | `common/templating/liquid-engine.ts:11-13`; `templating/index.ts:52`; `ui/templating/worker.ts:49` | Non-greedy `[\s\S]*?`; a `#}` inside a quoted argument is not protected. |
| 3 | Delimiter short-circuit: input with no `{{`/`{%`/`{#` is returned untouched | `templating/index.ts:34-39`; `common/render.ts:632-638` | Top-level `render()` returns a bare string, not a Promise (`liquid-compat.test.ts:46-48`). |
| 4 | Dual context: every key at the root and again under `_` | `templating/index.ts:44`; `common/templating/constants.ts:1-2` | `{{ x }}` and `{{ _.x }}` both resolve (`liquid-compat.test.ts:31-44`); brackets required for dashes/spaces. |
| 5 | Structured render errors: message, path, line/column, `reason: 'undefined' \| 'error'`, `undefinedEnvironmentVariables` | `common/templating/render-error.ts:28-55`, `:58-74`; used at `templating/index.ts:58-63` | `extractUndefinedVariableKey` strips filters and maps `_.x` -> `x` (`render-error.ts:62-72`). The renderer rebuilds the error from the worker message (`ui/worker/templating-handler.ts:57-74`). |
| 6 | Argument tokenizer: quoted strings with escapes, booleans, numbers, variables, expressions | `common/templating/tokenize-args.ts:3-92`; `common/templating/resolve-arg.ts:5-10` | A missing comma merges the rest into one argument (`utils.test.ts:232`). `resolveArg` uses `liquid.evalValue`, so a bare identifier is a context lookup. |
| 7 | All 12 built-in tags | `common/templating/local-template-tags.ts:17-1016` | See the Custom tags table; error strings are part of the tested contract. |
| 8 | Environment layering: global root/sub -> collection root/sub -> folder ancestors -> iteration -> transient | `common/render.ts:129-290`, `:355-594` | Later layers win. Preserve JSON property order via `dataPropertyOrder` (`:157-165`). Skip `__proto__`/`constructor`/`prototype` (`:410-413`). |
| 9 | Template-last key ordering plus a 3-pass render of the merged context | `common/render.ts:999-1007`, `:553-577` | Values containing `{{`/`{%` sort last. Unchanged results are skipped in later passes. Chains longer than 3 hops do not fully resolve. `errorMode: 'keep'` leaves broken templates verbatim. |
| 10 | Recursive object/array render with `disabled` skip, first-level `_`-key path reuse, blacklist regex, immutable input | `common/render.ts:600-712` | Date/RegExp/Error/Boolean/Number/Null/Undefined untouched; clone before mutation; `blacklistPathRegex` skips request bodies. |
| 11 | Second render pass when the first pass output gained `{%`, gated on the original input having no tags | `common/render.ts:649-659` | Injection guard: response bodies, file contents and OAuth tokens are never re-evaluated. |
| 12 | Request-level render: header/auth inheritance, description rendered separately at lower priority, only manual cookies rendered, gRPC variants | `common/render.ts:837-935`, `:805-835` | Server-set cookies must never be re-rendered (`:887-891`). |
| 13 | `{% response %}` incl. resend policies, cycle guard, send-only resend, JSONBig JSONPath, XPath, masked-URL refusal | `common/templating/local-template-tags.ts:570-806` | See the dedicated section: >15-digit precision, multi-result arrays, XPath single-result rule. |
| 14 | Tag execution contract: `run(context, ...args)` with `app/store/network/util`, `renderPurpose`, `meta`; result stringified, null/undefined -> `''` | `templating/liquid-extension.ts:36-126`; `common/templating/liquid-extension-worker.ts:78-212` | Main builds `util` from services plus `window.main` (`liquid-extension.ts:52-105`); the worker builds it from IPC (`liquid-extension-worker.ts:129-184`). |
| 15 | Confidential-value policy and masking (send/script reveal; preview conditional; undefined fail-closed mask) | `common/templating/confidential-value-policy.ts:23-42`; `common/render.ts:456-483` | Mask at merge time, before substitution. Mask value `••••••` (`:13`). A higher layer with no `kvPairData` entry clears confidentiality. |
| 16 | Vault decrypt/mask without mutating persisted data | `common/templating/mask-or-decrypt-vault-data.ts:15-63` | Decrypt only when the policy is `reveal`; `{}` when the vault is enabled without a key; non-object passthrough. |
| 17 | `{% prompt %}` never blocks outside `send` | `common/templating/local-template-tags.ts:428-472` | Other purposes return the cache or the default; cancel never overwrites the cache. |

### P1 — parity in common workflows

| # | Feature | Cite | Edge cases / notes |
|---|---|---|---|
| 18 | Base64-wrapped tag arguments (`b64::...::46b`), encoded on save and decoded on run | `common/templating/utils.ts:115-144`; decode at both tag factories (`templating/liquid-extension.ts:43`, `common/templating/liquid-extension-worker.ts:85`) | No whitespace tolerance inside the wrapper; non-strings pass through. |
| 19 | Self-recursive variable support (`base_url: '{{ base_url }}/foo'`) | `common/render.ts:415-445` | The regex only matches `{{ key` followed by a space or a pipe; other spacing will not trigger the pre-render. |
| 20 | `{% file %}` through a host allowlist, never direct `fs` | `common/templating/local-template-tags.ts:278-284`; `main/secure-read-file.ts:32-60`; handler `main/templating-worker-database.ts:655` | Roots: tmpdir, userData `responses/` and `version-control/`, user folders. Reserved `insomnia.*.db` files refused, including hard-link aliases. |
| 21 | Sensitive-value collection and redaction for `send` | `common/templating/types.ts:70-76`; `common/render.ts:580-587`; `mask-or-decrypt-vault-data.ts:39-44`; `templating/liquid-extension.ts:117-120` | The collector cannot cross structured clone, so worker values return as `sensitiveValue` messages keyed by `renderId` (`common/templating/render-context-serialization.ts:19`, `:52-56`; `ui/worker/templating-handler.ts:45-55`). |
| 22 | `allowTags: false` mode (interpolation only) | `templating/index.ts:51`, `:135-146`; test `liquid-compat.test.ts:659-672` | Separate cached tagless engine; any `{%` is a parse error. |
| 23 | `getTagDefinitions()` metadata for editors and plugins | `templating/index.ts:81-96`; worker `ui/templating/worker.ts:79-94`; renderer `ui/templating/renderer-safe.ts:23-52` | Filters `deprecated`, sorts by `priority`, defaults `liveDisplayName` and `disablePreview`. Carries `args`, `actions`, `hide`, `validate`, `displayName(args)`. |
| 24 | Plugin tag registration and priority (user plugins can override a built-in by name) | `templating/index.ts:105-122`; worker `ui/templating/worker.ts:118-145` | Priorities default to registration index; plugins spread after local tags. |
| 25 | Purpose-gated masking of the external vault tag before any provider fetch | `common/templating/confidential-value-policy.ts:55-72`; `templating/liquid-extension.ts:108-113`; worker `:186-191` | Only `@kong/insomnia-plugin-external-vault` + tag `vault` + `preview` + mask policy. |
| 26 | Cookie-jar lookup honoring URL scoping (`getCookiesForUrl` vs whole jar) | `common/templating/local-template-tags.ts:345-367`, `:926-940` | Both tags return `null` without request/workspace meta; name match is exact and case-sensitive (`cookie.key === name`). |
| 27 | CodeMirror mode `nunjucks` overlaying the base mode and tracking raw blocks | `ui/components/.client/codemirror/modes/nunjucks.ts:1-83` | The mode name is intentionally kept as `nunjucks` (`:1-2`). |
| 28 | Tag editor preview with `purpose: 'preview'` and a reveal toggle | `ui/components/templating/tag-editor.tsx:93-96`, `:254-300`; `variable-editor.tsx:38`, `:59` | `forceReveal` affects preview only; `shouldShowRevealToggle` keeps the toggle once a mask appears. Preview context cached 5 s (`ui/context/nunjucks/use-nunjucks.ts:82`). |
| 29 | Renderer-safe stubs so linting/codegen never load Node code | `ui/templating/renderer-safe.ts:10-52` | `render()` returns its input unchanged; do not use it to produce output. |
| 30 | Error-path fidelity in rendered requests (`_request.headers[0].value` style) | `common/render.ts:613-712`; test `common/__tests__/render.test.ts:1266-1296` | `errorMode: 'throw'` surfaces the failing path; first-level `_` keys collapse a path segment. |

### P2 — platform, editor and import extras

| # | Feature | Cite | Edge cases / notes |
|---|---|---|---|
| 31 | Persistent Web Worker render path with reload protocol and auth-token forwarding | `ui/worker/templating-handler.ts:7-41`; `ui/worker/templating.worker.ts:19-52` | `reload` may be the first message and carries its own token; it also calls `plugin.reloadPlugins`. Without it, a plugin missing at first engine build stays locked out for the session. |
| 32 | Render-context serialization across the worker boundary | `common/templating/render-context-serialization.ts:22-57` | Functions stripped; `purpose`, `settings`, `renderId` travel in `serializedFunctions`. |
| 33 | QuickJS sandbox for user plugin tags (10 s timeout, 32 MB heap, default-deny modules/capabilities, elevated escape hatch) | `templating/sandbox/plugin-tag-sandbox.ts:54-90`; `common/plugins/sandbox-mode.ts:23-56`; `templating/sandbox/PERMISSIONS.md` | Fresh context per call; interrupt handler kills sync loops; discovery mode installs a rejecting bridge. Bundled plugins are `internal` and never sandboxed. |
| 34 | IPC routing for tag helpers (`readFile`, `nodeOS`, `decode`, `encode`, `network.sendRequest`, cookie jar, response body, settings, plugin data) | `main/templating-worker-database.ts:654` (`pluginToMainAPI`), `:675`, `:687-690`, `:698`, `:735-750`, `:768`, `:771`, `:778`; worker side `common/templating/liquid-extension-worker.ts:90-184` | Custom protocol `insomnia-templating-worker-database://` with `x-insomnia-templating-auth`; every handler is inventoried in `main/templating-worker-database-inventory.ts`. |
| 35 | Platform mapping for `app.getInfo()` in the worker (`MacIntel` -> darwin, `Win32` -> win32, else linux) | `common/templating/liquid-extension-worker.ts:87-88` | Coarse; a Heric renderer can read the real OS. |
| 36 | Postman dynamic-variable import (`{{$guid}}` -> `{% faker 'guid' %}`) | `main/importers/importers/postman.ts:62-66` | One regex per faker key; only names present in `fakerFunctions` are mapped. |
| 37 | Konnect route sanitization (strip `{{ }}`/`{% %}` from external control-plane data until stable) | `konnect/transform.ts:12-29`, `:37-53` | Arrays that become empty are nulled so fallbacks apply. |
| 38 | GraphQL body `#}` -> `# }` rewrite before render | `common/render.ts:870-878` | Legacy workaround for stored bodies; only relevant if GraphQL bodies are supported. |
| 39 | Renderer `getTagDefinitions()` via the plugin renderer bridge | `ui/templating/renderer-safe.ts:23-52` | Assumes `~/ui/plugins/renderer-bridge` is available in the renderer. |
| 40 | Hung-render breadcrumb (log after 5 s, no abort) | `templating/index.ts:49` | Logging only; real limits come from `renderLimit`/`memoryLimit`. |
