# Scripting

Inventory of the Insomnia scripting subsystem: pre-request and after-response scripts, the
sandbox/security model, the `insomnia.*` object model, Postman compatibility (`pm.*`), module
imports, and the legacy Mocha unit-test framework. All paths relative to
`D:\Projects\Heric\Insomnia`.

Priorities: **P0** = required for any usable script support; **P1** = needed for parity with the
common Insomnia feature set; **P2** = rarely used / legacy / proof-of-concept.

---

## Execution model and sandbox

### Entry points and call graph

- Pre-request: `tryToExecutePreRequestScript` (`packages/insomnia/src/network/network.ts:274-397`).
  Folder-level scripts and the request script are joined, each wrapped in
  `const fn${i} = async () => { ... }; await fn${i}();` (`network.ts:296-305`), empty scripts are
  skipped, and results are tagged `category: 'pre-request'`. **P0**
- After-response: `tryToExecuteAfterResponseScript` (`network.ts:743-802`); runs with
  `eventName: 'test'`, passes `responseCookies: resp.cookies`, produces
  `category: 'after-response'` results (`network.ts:783,798`). **P0**
- Shared executor: `tryToExecuteScript` (`network.ts:489-704`). Builds `requestLocation` from
  ancestor folders, decrypts vault values only when enabled (`network.ts:524-534`), calls the
  runtime `runScript` (`network.ts:537`), records `dataPropertyOrder` changes
  (`network.ts:594-638`), registers confidential values (`network.ts:640-661`), appends console
  output to the timeline (`network.ts:663-665`), and converts thrown errors into a timeline entry
  plus `{ error }` (`network.ts:692-703`). **P0**
- Mutation persistence: `savePatchesMadeByScript` (`network.ts:403-487`) writes back cookie jar
  (source `'manual'`), environment, base environment, globals, base globals, and parent-folder
  environments. **P0**
- Manual/debug execution route: `packages/insomnia/src/routes/debug.request.$requestId.send.tsx`
  (pre-request error at 186-201, `skipRequest` → Cancelled at 203-221, after-response at 294-303,
  merged test results at 324-357, `nextRequestIdOrName` propagation). **P1**

### Three execution backends

1. **Hidden BrowserWindow (default).** Renderer calls
   `window.main.hiddenBrowserWindow.runScript` via the network runtime. Window options:
   `{ contextIsolation: false, nodeIntegration: true, preload: entry.hidden-window-preload.min.js }`
   (`packages/insomnia/src/main/window-utils.ts:134-148`). Renderer-side timeout races the run
   (`packages/insomnia/src/network/concurrency.renderer.ts:56-80`); one script at a time
   (fastq concurrency 1). **P0**
2. **Node main-process executor.** `packages/insomnia/src/script-executor.ts`, routed from
   `packages/insomnia/src/runtimes/network/network-adapter.node.ts:81-84`. Injects
   `insomnia, require, console, eval, _, setTimeout, setImmediate(undefined),
   queueMicrotask(undefined), process(undefined)` (`script-executor.ts:39-54`), appends
   `console` output to the timeline file (`script-executor.ts:73`). Known defect: the globals
   mapping uses the environment's `id`/`name` instead of the globals object
   (`script-executor.ts:99-106`). **P2** for Heric (app-internal legacy path).
3. **QuickJS-WASM Web Worker (experimental, disabled).** Kill switch
   `QUICKJS_SANDBOX_ENABLED = false` (`packages/insomnia/src/scripting/run-script-quickjs.ts:18`);
   also gated by `settings.useQuickJsScriptSandbox` (default `false`). **P2**

### Timeouts (three different defaults)

- Renderer race: `context.settings.timeout || 30_000`, error text
  `Executing script timeout: ${v}` (`packages/insomnia/src/network/concurrency.renderer.ts:59-61`).
- Hidden window race: `data.context.timeout || 5000`, error text
  `Timeout: Running script took too long` (`packages/insomnia/src/entry.hidden-window.ts:39-43`).
- QuickJS: default `30_000` ms (`packages/insomnia/src/scripting/quickjs-script-engine.ts:49`);
  deadline interrupt (`:68`), 32 MB memory limit (`:69`), 256 KB stack (`:76`).
- Settings default `timeout: 30_000` (`packages/insomnia-data/src/models/settings.ts:78`).

### Sandbox construction (hidden-window path)

`runScript` (`packages/insomnia/src/scripting/run-script.ts`):

- Policy selection: sandbox active unless `settings.scriptSandboxEnabled === false`
  (`run-script.ts:22`).
- `prepareSandbox` (`packages/insomnia/src/scripting/sandbox.ts:295-348`): parses and statically
  checks the script, deep-freezes settings (`:324`), throws `SECURITY_POLICY_VIOLATION`
  (`:319`), filters rules removed via `disabledSecurityRules` / `disabledBlockedProperties` /
  `disabledBlockedRoots` (`:326-330`), and bypasses everything when sandboxing is off
  (`:331-336`). **P0**
- Script is executed through `AsyncFunction(...params, body)` in an IIFE with
  `'use strict'` by default (`run-script.ts:42-54,69`), plus `const $ = insomnia;`
  (`run-script.ts:49`) and bridge calls `__bridgeReset__`, `__waitForAllTestsDone__`,
  `__bridgeStop__`, `__bridgeSettle__`. Params: `insomnia, console, _, setTimeout,
  __waitForAllTestsDone__, __bridgeReset__, __bridgeStop__, __bridgeSettle__, ...maskNames`
  (`run-script.ts:31-41`). **P0**
- Return-value guard: the script must return the `InsomniaObject`
  (`run-script.ts:83-85`). Results are merged through `mergeRequests`, `mergeSettings`,
  `mergeClientCertificates`, `mergeCookieJar` (`run-script.ts:87-94`). **P0**
- `setTimeout` is wrapped to register a tracked promise (`run-script.ts:139-150`). **P1**

### Static AST sandbox

`checkSandboxViolations` (`packages/insomnia/src/scripting/sandbox.ts:74-254`) uses acorn +
acorn-walk. **P0**

- Source is parsed as `module` then re-parsed/treated as script; unparseable code is silently
  allowed (`sandbox.ts:88-92`) — static checks are best-effort, runtime masks are the real barrier.
- Alias tracking (`sandbox.ts:114-147`) catches `const p = process`.
- Member expressions, `this`, blocked roots, blocked properties, `Symbol.species`
  (`sandbox.ts:150-188`).
- Destructuring patterns from blocked objects (`sandbox.ts:189-227`).
- `import` declarations and dynamic `import()` blocked (`sandbox.ts:229-241`).
- Calls to blocked roots blocked (`sandbox.ts:244-252`).

Rule tables:

- Blocked properties (18): `prototype`, `mainModule`, `constructor`, `__proto__`,
  `prepareStackTrace`, `captureStackTrace`, `getPrototypeOf`, `setPrototypeOf`, `getFunction`,
  `getThis`, `__defineGetter__`, `__defineSetter__`, `__lookupGetter__`, `__lookupSetter__`,
  `defineProperty`, `defineProperties`, `getOwnPropertyDescriptor`, `getOwnPropertyDescriptors`
  (`packages/insomnia/src/scripting/script-security-rules.ts:14-89`). **P0**
- Blocked roots (12): `this`, `globalThis`, `global`, `window`, `self`, `frames`, `process`,
  `module`, `exports`, `Buffer`, `constructor`, `arguments`
  (`script-security-rules.ts:91-150`). **P0**
- Mask rules (9, parameter set to `undefined`): `globalThis`, `global`, `Function`, `process`,
  `setImmediate`, `queueMicrotask`, `Proxy`, `Reflect`, `WebAssembly`
  (`script-security-rules.ts:152-216`). **P1**
- Runtime interceptors (`packages/insomnia/src/scripting/script-security-policy.ts:9-60`):
  `require` (allowlist), `window` (proxy exposing only `resetAsyncTasks`,
  `stopMonitorAsyncTasks`, `asyncTasksAllSettled`), `eval` (re-runs the AST check, then
  `(0, eval)(script)`). **P1**
- Always-on interceptors regardless of settings: `{require, window, eval}`
  (`packages/insomnia/src/scripting/sandbox.ts:40`). **P1**

Security settings and defaults (`packages/insomnia-data/src/models/settings.ts`):
`followRedirects: true` (`:49`), `timeout: 30_000` (`:78`), `enableVaultInScripts: false`
(`:86`), `scriptSandboxEnabled: true` (`:91`), `scriptStrictModeEnabled: true` (`:92`),
`useQuickJsScriptSandbox: false` (`:93`), `disabledSecurityRules: []` (`:95`),
`disabledBlockedProperties: []` (`:96`), `disabledBlockedRoots: []` (`:97`),
`enableLegacyUnitTests: false` (`:99`). UI: `packages/insomnia/src/ui/components/settings/scripting-settings.tsx:151-411`.

### Hidden-window error surface

`packages/insomnia/src/entry.hidden-window.ts:34-68`: timeouts, `SECURITY_POLICY_VIOLATION`
passthrough (`:49-53`), other errors prefixed `Error from Pre-request or after-response script:`
with stack appended (`:54-57`), reported to Sentry. **P1**

### Async task tracking

- `window.Promise` is replaced with `ProxiedPromise` in the hidden-window preload
  (`packages/insomnia/src/entry.hidden-window-preload.ts:66-70`); bridge exposes
  `requireInterceptor`, `onmessage`, `curlRequest`, `readCurlResponse`, `setBusy`, `appendFile`,
  `Promise: OriginalPromise`, and the three async-task operations
  (`entry.hidden-window-preload.ts:19-72`). **P1**
- `ProxiedPromise` (`packages/insomnia-scripting-environment/src/objects/async-objects.ts:11-67`):
  records instances plus `all`/`race`/`reject`/`resolve`; `allSettled` is not separately recorded;
  `any` and `withResolvers` return rejected promises with the strings
  `'super.any' not supported` / `'Promise.withResolvers' not supported` (`:34-36,64-66`).
- `asyncTasksAllSettled`, `stopMonitorAsyncTasks`, `resetAsyncTasks`
  (`async-objects.ts:70-84`). **P1**

### Request/response merging semantics (edge cases)

- `mergeRequests` forces `preRequestScript: ''` on the merged request
  (`packages/insomnia-scripting-environment/src/objects/request.ts:708-740`). **P0**
- `mergeSettings` only carries proxy settings (`request.ts:474-495`). **P1**
- `mergeClientCertificates` requires `pfx` XOR (`key` + `cert`), otherwise throws
  (`request.ts:497-578`). **P1**
- `mergeRequestBody` preserves mime types per body mode and only overwrites provided fields
  (`request.ts:620-706`). **P1**
- `mergeCookieJar` (`packages/insomnia-scripting-environment/src/objects/cookies.ts:365-381`). **P1**
- `Execution.skipRequest` and `setNextRequest` are read back by the debug route and drive
  cancellation/flow control (`packages/insomnia-scripting-environment/src/objects/execution.ts:25-40`,
  route at `debug.request.$requestId.send.tsx:203-221`). **P1**

### QuickJS bridge (P2, disabled)

`packages/insomnia/src/scripting/quickjs-script-engine.ts`: BOOTSTRAP exposes only
`insomnia.environment.{get,set}`, `insomnia.variables.{get,set}`, a frozen `insomnia.request`,
`insomnia.sendRequest` (string URL or `{url, method, headers|header, body}`), a response with
`code/status/headers/body/responseTime/json()/text()`, and `$` alias (`:141-209`); `insomnia.test`
throws. Explicitly unsupported: `insomnia.test()/pm.test()`, `collectionVariables`, vault, cookies,
client certificates, request mutation (`:25-27`); text/plain bodies only (`:253-277`); dangerous
bridge keys rejected (`:233,389`); database bridge endpoint
`insomnia-templating-worker-database://network.sendrequestwithoutsideeffects` with
`x-insomnia-templating-auth` (`:239-240`).

---

## insomnia.* API

`InsomniaObject` (`packages/insomnia-scripting-environment/src/objects/insomnia.ts:22-148`).
Every property below is present at runtime; the generated autocomplete catalog has 227 entries
(`packages/insomnia-scripting-environment/src/autocomplete-snippets.json`).

### Root object

| Method / property | Behavior | Source |
|---|---|---|
| `insomnia.environment` | `Environment` for the active environment (empty object if none) | `insomnia.ts:23` |
| `insomnia.collectionVariables` | Alias of `baseEnvironment` (same instance) | `insomnia.ts:24,72` |
| `insomnia.baseEnvironment` | `Environment` for the base environment | `insomnia.ts:25` |
| `insomnia.globals` / `insomnia.baseGlobals` | Global variable scopes (private fields, copied into `toObject`) | `insomnia.ts:41-42,131-132` |
| `insomnia.iterationData` | Iteration scope for the current run | `insomnia.ts:39` |
| `insomnia.variables` | Layered `Variables` accessor | `insomnia.ts:26` |
| `insomnia.request` | `Request` (method, url, headers, body, auth, proxy, certificate, pathParameters) | `insomnia.ts:27` |
| `insomnia.response` | `Response`, only in after-response scripts | `insomnia.ts:30` |
| `insomnia.cookies` | `CookieObject` backed by the cookie jar | `insomnia.ts:28` |
| `insomnia.info` | `RequestInfo` (`eventName`, `iteration`, `iterationCount`, `requestName`, `requestId`) | `insomnia.ts:29` |
| `insomnia.execution` | `Execution` (`location` with `.current`, `skipRequest`, `setNextRequest`) | `insomnia.ts:31` |
| `insomnia.vault` | `Vault`, present only when `enableVaultInScripts` | `insomnia.ts:32,180-183` |
| `insomnia.clientCertificates` | Array of `Certificate` | `insomnia.ts:34` |
| `insomnia.parentFolders` | `ParentFolders` of the request | `insomnia.ts:47` |
| `insomnia.settings` | Getter that always returns `undefined` | `insomnia.ts:125-127` |
| `insomnia.sendRequest(request, cb)` | Sends a request through the host; throws `sendRequest is not available in this execution context.` when no curl executor is injected | `insomnia.ts:106-111` |
| `insomnia.test(msg, fn)` | Registers an assertion block; proxy-wrapped handler with `.skip` | `insomnia.ts:89-103` |
| `insomnia.test.skip(msg, fn)` | Records a skipped test | `insomnia.ts:95-97`, `test.ts:54-61` |
| `insomnia.expect(value)` | chai `expect` | `insomnia.ts:121-123` |
| `insomnia.toObject()` | Serializes the whole context (envs, request, cookies, info, response, test results, execution, folders) | `insomnia.ts:129-147` |
| `insomnia.pushRequestTestResult(result)` | Private; used by `test()` internals | `insomnia.ts:117-119` |
| `$` | Alias for `insomnia`, declared in the script wrapper | `run-script.ts:49` |

Edge cases: `initInsomniaObject` logs `No environment is selected...` when no environment
(`insomnia.ts:168-172`), warns when a selected certificate does not match the request URL
(`insomnia.ts:210-239`), and applies `transformToSdkProxyOptions`, which logs
`Using proxy: ...` (`proxy-configs.ts:375`).

### Environment

`packages/insomnia-scripting-environment/src/objects/environments.ts:31-149`; `Vault` at `:389-432`.

| Method / property | Behavior | Source |
|---|---|---|
| `name` | Environment name | `environments.ts:58` |
| `has(name)` | Membership test | `environments.ts:68` |
| `get(name)` | Value or `undefined` | `environments.ts:78` |
| `set(name, value)` | Sets `boolean \| number \| string \| undefined \| null`; `NaN` logs a warning and is skipped | `environments.ts:90,309-311` |
| `unset(name)` | Deletes key | `environments.ts:103` |
| `clear()` | Removes all keys | `environments.ts:111` |
| `replaceIn(template)` | Liquid interpolation + `{{$faker}}`; throws `TypeError` for non-string/object/non-entry | `environments.ts:127`, `interpolator.ts:8-53` |
| `toObject()` | Plain object copy | `environments.ts:142` |
| `toJSON()` | Same as above | `environments.ts:146` |
| `Vault.get/set/toObject` | Proxy throws `Vault is disabled in script` when disabled; `unset`/`clear`/`set` always throw | `environments.ts:399-432` |

### Variables

`packages/insomnia-scripting-environment/src/objects/environments.ts:195-367`.

| Method / property | Behavior | Source |
|---|---|---|
| `variables.has(name)` | Checks all scopes | `environments.ts:250` |
| `variables.get(name)` | Precedence: local > folder > iterationData > environment > collection (base) > global > baseGlobal | `environments.ts:287-297` |
| `variables.set(name, value)` | Writes to local scope; `NaN` warns and skips | `environments.ts:309-311` |
| `variables.replaceIn(template)` | Resolves variables with the same precedence | `environments.ts:330` |
| `variables.toObject()` | Merged view of all scopes | `environments.ts:349` |
| `variables.localVarsToObject()` | Only the local scope | `environments.ts:364` |

Edge case: scalar variables are supported; `VariableList` also exposes `Variable` objects
(`variables.ts:31-105`) with `get()`, `set(value)`, `cast(value)`, `static types()` (throws),
and `VariableList.toObject` / `isVariableList` (`variables.ts:114-140`).

### Request and related objects

`packages/insomnia-scripting-environment/src/objects/request.ts`.

| Method / property | Behavior | Source |
|---|---|---|
| `request.name` | Request name | `request.ts:283` |
| `request.url` | `Url` instance | `request.ts:284` |
| `request.method` | HTTP method string | `request.ts:285` |
| `request.headers` | `HeaderList<Header>` | `request.ts:286` |
| `request.body` | `RequestBody` or `undefined` | `request.ts:287` |
| `request.auth` | `RequestAuth` | `request.ts:288` |
| `request.proxy` | `ProxyConfig` or `undefined` | `request.ts:289` |
| `request.certificate` | `Certificate` or `undefined` | `request.ts:290` |
| `request.pathParameters` | Array of path params | `request.ts:291` |
| `request.addHeader(header)` | Accepts `Header` or `{key, value}`; throws otherwise | `request.ts:315-325` |
| `request.addQueryParams(params)` | Delegates to `url` | `request.ts:327-329` |
| `request.authorizeUsing(type, options?)` | Delegates to `auth.use` | `request.ts:331-334` |
| `request.clone()` | Deep clone via `toJSON` | `request.ts:336-338` |
| `request.forEachHeader(cb)` | Iterates headers | `request.ts:340-342` |
| `request.getHeaders(options?)` | `{ignoreCase, enabled, multiValue, sanitizeKeys}`; merges duplicate keys into arrays | `request.ts:344-374` |
| `request.removeHeader(header, options?)` | Key or `Header` | `request.ts:375-399` |
| `request.removeQueryParams(params)` | String, string[] or `QueryParam[]`; throws on other types | `request.ts:400-402` |
| `request.size()` | `{body, header, total}` byte sizes | `request.ts:404-447` |
| `request.update(options)` | Replaces all fields | `request.ts:448-460` |
| `request.upsertHeader(header)` | Removes every header with the same key, then appends | `request.ts:462-471` |
| `request.toJSON()` | Serializable form including headers, body, auth | `request.ts:408-423` |

`RequestBody` (`request.ts:137-226`): fields `mode` (`undefined | formdata | urlencoded | raw | file | graphql`),
`file`, `formdata`, `graphql`, `raw`, `urlencoded`, `options`; methods `isEmpty()` (`:162`),
`toString()` (`:185-212`), `update(opts)` (`:216`). `FormParam` (`request.ts:43-81`) is a `Property`
with `key`, `value`, `type`.

`Header` (`packages/insomnia-scripting-environment/src/objects/headers.ts`): `key`, `value`,
`update({key,value})` (`:193`), statics `create` (`:97`), `isHeader` (`:106`), `parse` (`:120`),
`parseSingle` (`:144`), `unparse` (`:168`), `unparseSingle` (`:181`). `HeaderList` adds
`contentSize()` (`:245`), `isHeaderList` (`:233`), and `toObject({excludeDisabled, multiValue})`.

`QueryParam` (`urls.ts:18-125`): `key`, `value`, `toRawString()` (`:105`), `update()` (`:109`),
statics `parse` (`:62`), `parseSingle` (`:67`), `unparse` (`:76`), `unparseSingle` (`:88`).

`Url` (`urls.ts:142-367`): getters `auth` (`:150`), `hash` (`:157`), `host` (string[], `:161`),
`path` (string[], `:164`), `port` (`:167`), `protocol` (`:170`), `query` (`:173`), `variables`
(`:176`); `urlObject` is the underlying `URL` (private field, exposed through the generated API
docs); methods `addQueryParams` (`:262`), `getHost` (`:280`), `getPath` (`:287`),
`getPathWithQuery` (`:294`), `getQueryString` (`:301`), `getRemote` (`:308`),
`removeQueryParams` (`:315`), `toString` (`:336`), `toStringWithoutQuery` (`:350`), `update`
(`:364`); statics `isUrl` (`:236`), `parse` (`:240`).

`UrlMatchPattern` (`urls.ts:375-598`): Chrome-style patterns with `<all_urls>` (`:404`) and `+`
protocol delimiter (`:406`); `getProtocols` (`:410`), `test` (`:426`), `testHost` (`:464`),
`testPath` (`:503`), `testPort` (`:547`), `testProtocol` (`:578`), `toString` (`:591`), `update`
(`:595`). `UrlMatchPatternList.test(url)` returns true when any pattern matches (`:612-614`).

`RequestAuth` (`auth.ts:259-351`): types `noauth, basic, bearer, jwt, digest, oauth1, oauth2,
hawk, awsv4, ntlm, apikey, edgegrid, asap, netrc` (`:7-37`); `isValidType` (`:296`), `clear(type)`
(`:300`), `parameters()` (`:306`), `toJSON()` (`:310`), `update()` (`:324`), `use(type, options)`
(`:336`). Edge case: the `use()` error message omits `netrc` (`:338-340`); `noauth`/`netrc`
serialize without options (`:317-319`).

### Response

`packages/insomnia-scripting-environment/src/objects/response.ts:54-357`.

| Method / property | Behavior | Source |
|---|---|---|
| `body` | Raw body string | `response.ts:55` |
| `code` | Status code | `response.ts:56` |
| `cookies` | `CookieList` | `response.ts:57` |
| `headers` | `HeaderList` | `response.ts:58` |
| `originalRequest` | `Request` that produced the response | `response.ts:59` |
| `responseTime` | Elapsed ms | `response.ts:60` |
| `status` | Reason phrase, falls back to `RESPONSE_CODE_REASONS[code]` then `''` | `response.ts:61,78` |
| `stream` | `Buffer \| ArrayBuffer \| undefined` (undefined for curl output) | `response.ts:62` |
| `contentInfo()` | Type/size info derived from body | `response.ts:111` |
| `dataURI()` | `data:` URI built from content info (base64; contains the typo `baseg4`) | `response.ts:164-172` |
| `json(reviver?)` | Parses body as JSON | `response.ts:174` |
| `jsonp(reviver?)` | Throws (not implemented) | `response.ts:183` |
| `reason()` | Reason phrase | `response.ts:187` |
| `size()` | Body byte size | `response.ts:191` |
| `text()` | Body as text | `response.ts:201` |
| `to` | chai assertion namespace: properties `withBody`, `error` (4xx/5xx), `ok` (200), `json`; methods `status(val)`, `header(name)`, `body(content)`, `jsonBody(val)`, `jsonSchema(schema, options?)` | `response.ts:208-357` |
| `toScriptResponse(...)` | Builds a `Response` from runtime data | `response.ts:359-401` |
| `readBodyFromPath(path)` | Reads a body file | `response.ts:403-419` |

Edge cases: `body` is `''` when absent; `stream` may be `undefined` for curl-sourced responses
(`send-request.ts:202-288`); chai plugins are registered lazily inside the `to` getter on every
access (idempotent but side-effectful).

### Cookies

`packages/insomnia-scripting-environment/src/objects/cookies.ts`.

| Method / property | Behavior | Source |
|---|---|---|
| `cookies.all()/count()/toObject()` | Inherited `CookieList` behavior | `cookies.ts:180-207` |
| `cookies.jar()` | Returns the `CookieJar` | `cookies.ts:248-250` |
| `cookie.key`, `cookie.value`, `cookie.domain`, `cookie.path`, `cookie.expires`, `cookie.secure`, `cookie.httpOnly`, `cookie.hostOnly`, `cookie.extensions` | Cookie fields (extensions unsupported, `maxAge`/`session` always `undefined`) | `cookies.ts:30-178,220-238` |
| `Cookie.parse/stringify/unparse/unparseSingle/isCookie` | Static converters | `cookies.ts:68-138` |
| `jar.set(url, key, value, cb)` | Callback API; warns when the cookie has no domain | `cookies.ts:277-307` |
| `jar.get(url, name, cb)` | Callback API | `cookies.ts:309` |
| `jar.getAll(url, cb)` | Callback API | `cookies.ts:314` |
| `jar.unset(url, name, cb)` | Callback API | `cookies.ts:319` |
| `jar.clear(url, cb)` | Callback API | `cookies.ts:329` |
| `jar.toInsomniaCookieJar()` | Persistable jar | `cookies.ts:334` |

Edge case: the script cookie jar is domain-keyed (`Map<domain, Map<key, Cookie>>`) and is
rebuilt from the Insomnia jar on every script run (`cookies.ts:212-246,256-259`).

### Proxy and certificates

- `ProxyConfig` (`proxy-configs.ts:45-257`): `getProtocols()` (`:177`), `getProxyUrl()` (`:193`),
  `test(url?)` (`:210`), `update(options)` (`:237`), `updateProtocols()` always throws (`:253`).
  `ProxyConfigList.resolve(url?)` (`:299`), `toObject` (`:270-333`);
  `transformToSdkProxyOptions` (`:336-382`) logs the sanitized proxy URL at `:375`.
- `Certificate` (`certificates.ts:18-60`): `name`, `matches` (`UrlMatchPatternList`), `key`,
  `cert`, `passphrase`, `pfx`, `disabled`; `canApplyTo(url)` (`:47`), `update(options)` (`:51`),
  `isCertificate` (`:43`).

### Folders, info, execution

- `Folder` (`folders.ts:9-67`): `id`, `name`, `environment`, `toObject()`. `ParentFolders`
  (`folders.ts:69-147`): `get(idOrName)` (`:84`), `getById` (`:99`), `getByName` (`:114`),
  `findValue(key)` returns the nearest ancestor value (`:129`), `toObject()` (`:139`),
  `getEnvironments()`.
- `RequestInfo` (`request-info.ts:26-100`): `eventName` (`'prerequest' | 'test'`, `:31`),
  `iteration` (`:37`), `iterationCount` (`:43`), `requestName` (`:49`), `requestId` (`:53`),
  `toObject()` (`:83`).
- `Execution` (`execution.ts:7-40`): `location` is a Proxy whose `.current` returns the last path
  segment (`:16-23`), `skipRequest()` (`:25`), `setNextRequest(idOrName)` (`:29`), `toObject()`.
  Throws `TypeError('Location input must be array of string')` otherwise (`:27`).

### Generic property model

`packages/insomnia-scripting-environment/src/objects/properties.ts`.

- `PropertyBase`: `meta()` (`:36`), `parent()` (`:40`), `forEachParent()` (`:44`),
  `findInParents()` (`:76`), `toJSON()` (`:117`), `toObject()` (`:125`), `toString()` (`:129`),
  statics `propertyIsMeta`/`propertyUnprefixMeta` (`:21,27`).
- `Property`: `id`, `name`, `disabled`, `description`, `toJSON`, `toObject`, `describe`
  (`:134-191`), statics `replaceSubstitutions` (`:150`), `replaceSubstitutionsIn` (`:164`).
- `PropertyList` (`:193-428`): `add`, `all`, `append`, `assimilate(source, prune)`, `clear`,
  `count`, `each`, `filter`, `find`, `get(key)`, `has`, `idx`, `indexOf`, `insert`,
  `insertAfter`, `map`, `one(id)`, `populate`, `prepend`, `reduce`, `remove`, `repopulate`,
  `toObject`, `toString`, `upsert`, `isPropertyList`. `eachParent` throws
  `unsupportedError('eachParent')` (`:256-258`).

### Console

`packages/insomnia-scripting-environment/src/objects/console.ts`: `log` (`:69`), `warn` (`:78`),
`debug` (`:87`), `info` (`:96`), `error` (`:105`), `clear` (throws, `:113-115`), `dumpLogs`
(`:118`), `dumpLogsAsArray` (`:123`); `getExistingConsole`/`getNewConsole` (`:131-137`).
`dumpLogs` output is appended to the timeline by both executors (`script-executor.ts:73`,
`network.ts:663-665`).

### Interpolation exposed to scripts

`Environment.replaceIn` / `Variables.replaceIn` use LiquidJS with `{{ }}` and `{% %}`,
`strictVariables`, `jsTruthy`, `ownPropertyOnly: false` (`interpolator.ts:8-53`). `{{$name}}`
tags resolve against faker functions (`insomnia-data/common-src/faker-functions.ts:3-80+`, ~100
entries: `guid`, `timestamp`, `randomUUID`, `randomAlphaNumeric`, `randomBoolean`, `randomInt`,
`randomColor`, `randomIP`, `randomUserAgent`, `randomFirstName`, `randomCity`,
`randomCompanyName`, `randomCurrencyCode`, `randomDateFuture`, ...). Unknown function throws
`replaceIn: no faker function is found: ${funcName}` (`interpolator.ts:44-46`). **P1**

### Collection namespace (constructors available to scripts)

`packages/insomnia-scripting-environment/src/objects/collection.ts:25-35` exports
`RequestAuth`, `Certificate`, `Cookie`, `CookieList`, `Header`, `HeaderList`, `Property`,
`PropertyBase`, `PropertyList`, `ProxyConfig`, `ProxyConfigList`, `FormParam`, `Request`,
`RequestBody`, `Response`, `QueryParam`, `Url`, `UrlMatchPattern`, `UrlMatchPatternList`,
`Variable`, `VariableList`, `Folder`. Reachable as
`require('insomnia-collection')` or `require('postman-collection')`
(`packages/insomnia/src/scripting/require-interceptor.ts:89-96`). **P1**

---

## pm.* API

**There is no runtime `pm` object.** `pm` exists in exactly two places:

1. Import-time / paste-time translation: `translateHandlersInScript`
   (`packages/insomnia/src/main/importers/importers/translate-postman-script.ts:190-208`) applies
   `DEFAULT_PREPROCESS_RULES` (`:18-188`) and finally rewrites `pm.` → `insomnia.` with
   `translated.replace(/(?<![\.\$\-"'])\bpm\./g, 'insomnia.')` (`:205`). Applied on Postman import
   (`packages/insomnia/src/main/importers/importers/postman.ts:197,218`) and on paste in the
   script editor (`packages/insomnia/src/ui/components/editors/request-script-editor.tsx:505`).
2. Editor lint globals only: `pm: true` in the jshint options
   (`request-script-editor.tsx:120-134`), so pasted scripts do not show `pm is not defined`.

Consequences / edge cases:

- `pm.environment`, `pm.variables`, `pm.globals`, `pm.collectionVariables`, `pm.request`,
  `pm.response`, `pm.cookies`, `pm.vault`, `pm.iterationData`, `pm.info`, `pm.execution`,
  `pm.sendRequest`, `pm.test`, `pm.expect` all only work because they were textually rewritten to
  `insomnia.*`.
- `pm.response.to.have.status(...)` is rewritten to `insomnia.response.to...`; the assertion
  surface is the chai namespace in `response.ts:208-357`, **not** Postman's `pm.response.to.have`.
- Rules deliberately rewrite `pm.response.json()` etc.; comments in the rule table document
  unsupported constructs (`tests.desc =`, `Object.keys(globals)`, `'env' in globals`, `request`,
  `data`) (`translate-postman-script.ts:18-188`).
- Prefix guards mean `µpm.`, `$pm.`, `-pm.`, `"pm.`, `'pm.` are not rewritten (`:205`).
- Translated `pm.test` maps to `insomnia.test`; QuickJS refuses both (`quickjs-script-engine.ts:25-27`).
- No `pm.*` snippets exist in the autocomplete catalog (`autocomplete-snippets.json`).

**Priority P1** for the translator, **P0** for documenting that `pm` is a translation artifact.

---

## Module imports allowed

`require` is intercepted (`packages/insomnia/src/scripting/require-interceptor.ts:39-99`).

Allowlisted external modules (`require-interceptor.ts:14-25`):

| Specifier | Implementation |
|---|---|
| `ajv` | ajv |
| `chai` | chai |
| `cheerio` | cheerio |
| `crypto-js` | crypto-js |
| `csv-parse/lib/sync` | csv-parse sync parser |
| `lodash` | `es-toolkit/compat` |
| `moment` | moment |
| `tv4` | tv4 |
| `uuid` | uuid |
| `xml2js` | xml2js |
| `insomnia-collection` | `Collection` namespace |
| `postman-collection` | `Collection` namespace |

Node built-ins passed through (`require-interceptor.ts:60-85`): `path`, `assert`, `url`,
`punycode`, `querystring`, `string_decoder`, `stream`, `events`; plus `atob` and `btoa`.
Wrapped/restricted: `timers` (blocks `setImmediate`), `buffer` (blocks `Buffer.allocUnsafe` and
`Buffer.allocUnsafeSlow`), `util` (blocks `inherits`, `debuglog`).

Fallback: `no module is found for "${name}"`. Variants handled:
`node:path`-style prefixes are normalized; the `blockMethods` proxy labels blocked APIs.

Edge cases: `lodash` is emulated by `es-toolkit/compat`, so lodash edge APIs may differ.
`require` is always intercepted even when `scriptSandboxEnabled` is false
(`sandbox.ts:40`). The legacy unit-test runner has a **different** allowlist (`path`, `assert`,
`buffer`, `util`, `url`, `punycode`, `querystring`, `string_decoder`, `stream`, `timers`, `events`,
`atob`, `btoa`, `chai`, `chai-json-schema`) — `packages/insomnia-testing/src/run/run.ts:14-51`.

**Priority P0** for the allowlist, **P1** for per-module method blocking.

---

## Test framework and assertions

### Runtime script assertions (pre-request / after-response)

- `insomnia.test(msg, fn)` runs `fn`, measures `performance.now()`, and pushes a
  `RequestTestResult` with `status: 'passed' | 'failed'`, `executionTime`, and
  `category: 'unknown'` (`packages/insomnia-scripting-environment/src/objects/test.ts:6-35`).
- Failure message format: `error: ${e} | ACTUAL: ${e.actual} | EXPECTED: ${e.expected}`
  (`test.ts:26`).
- `insomnia.test.skip` emits `status: 'skipped'`, `executionTime: 0` (`test.ts:54-61`).
- Pending tests are tracked and drained before the script returns
  (`test.ts:40-51`, `run-script.ts:50-52`).
- Result shape: `RequestTestResult` with `testCase`, `status`, `executionTime`, `errorMessage?`,
  `category` (`packages/insomnia-data/src/models/runner-test-result.ts:13-22`).
- Categories `'unknown' | 'pre-request' | 'after-response'` are assigned by the callers
  (`network.ts:378-380,798`), not by `test()`.
- Tests are **not awaited inline** by the user; the bridge drains them, so an unawaited failing
  assertion still yields a failed result.
- Assertions use chai (`insomnia.expect`) plus the `insomnia.response.to` extension
  (`response.ts:208-357`). `require('chai')` is allowed for `chai.assert`-style tests.

### Legacy Mocha unit-test framework (`packages/insomnia-testing/`)

- Code generation: `generate` (`packages/insomnia-testing/src/generate/generate.ts:15-91`) emits
  a preamble `const { expect } = chai;` and
  `beforeEach(() => insomnia.clearActiveRequest());`, nested `describe` blocks, and
  `insomnia.setActiveRequestId('...')` per test. Helpers `escapeJsStr`/`indent` in
  `generate/util.ts`.
- Runner: `packages/insomnia-testing/src/run/run.ts:14-161`. Injects a `require` interceptor
  (`:14-51`), a global `insomnia` object (`:53-79`), runs Mocha with a **60 s** timeout
  (`:81`), and returns `{ stats, tests, pending, failures, passes }` (`:151-161`);
  `runTestsCli` returns a boolean. Uses temp files and a JSON reporter that does not write to
  stdout (`javascript-reporter.ts`).
- `insomnia` object for unit tests (`packages/insomnia-testing/src/run/insomnia.ts`):
  `activeRequestId`, `setActiveRequestId`, `clearActiveRequest`, and
  `async send(reqId = null)` which throws `No selected request` when none is set.
- Host wiring: `getSendRequestCallback` (`packages/insomnia/src/network/unit-test-feature.ts:13-50`)
  returns `{ status, statusMessage, data, headers, responseTime }`; IPC `run-tests`
  (`packages/insomnia/src/main/ipc/main.ts:967-970`) calls `runTests(src, { sendRequest })`.
- UI routes: `...test.test-suite.$testSuiteId.run-all-tests.tsx`, `...$testId.run.tsx`;
  gated by `enableLegacyUnitTests` (default `false`).
- Tests are stored as generated JS source in the test-suite model, not as structured steps.

**Priority P2** for the legacy Mocha framework; **P0** for `insomnia.test` + chai assertions.

---

## Existing tests usable as golden tests

| Suite | Cases | Path |
|---|---|---|
| Static sandbox / AST rules | 73 | `packages/insomnia/src/scripting/__tests__/sandbox.test.ts` |
| Security policy + interceptors | 25 | `packages/insomnia/src/scripting/__tests__/script-security-policy.test.ts` |
| `require` allowlist and blocked APIs | 49 | `packages/insomnia/src/scripting/__tests__/require-interceptor.test.ts` |
| QuickJS engine behavior | 30 | `packages/insomnia/src/scripting/quickjs-script-engine.test.ts` |
| QuickJS worker client | 7 | `packages/insomnia/src/scripting/run-script-quickjs.test.ts` |
| Postman script translation | 3 top-level (`it` counts); many `test.each` rows in `translate-postman-script.test.ts` | `packages/insomnia/src/main/importers/importers/translate-postman-script.test.ts` |
| Vault persistence after scripts | 3 | `packages/insomnia/src/network/__tests__/script-vault-persistence.test.ts` |
| Unit-test generation | 1 (fixture-driven) + 5 (util) | `packages/insomnia-testing/src/generate/generate.test.ts`, `.../util.test.ts` |
| Unit-test runner | 8 | `packages/insomnia-testing/src/run/run.test.ts` |
| Unit-test end-to-end | 3 | `packages/insomnia-testing/src/integration/integration.test.ts` |
| Auth object | 2 | `packages/insomnia-scripting-environment/src/objects/__tests__/auth.test.ts` |
| Certificates | 1 | `.../certificates.test.ts` |
| Console | 7 | `.../console.test.ts` |
| Cookies | 9 | `.../cookies.test.ts` |
| Environments / Variables | 10 | `.../environments.test.ts` |
| Execution | 3 | `.../execution.test.ts` |
| Headers | 6 | `.../headers.test.ts` |
| Properties | 7 | `.../properties.test.ts` |
| Proxy configs | 8 | `.../proxy-configs.test.ts` |
| Request info | 2 | `.../request-info.test.ts` |
| Request | 8 | `.../request.test.ts` |
| Response | 1 | `.../response.test.ts` |
| `test()` | 6 | `.../test.test.ts` |
| URLs / match patterns | 13 | `.../urls.test.ts` |
| Variables | 6 | `.../variables.test.ts` |

Fixture pairs for code generation: `packages/insomnia-testing/src/generate/fixtures/01…05`
(input/output JS, including nested `describe` cases).

Gaps: `run-script.ts` itself has no direct unit test (covered indirectly via
`quickjs`/engine tests and vault-persistence); there is no renderer-level test for the
hidden-window bridge; no test for `savePatchesMadeByScript` beyond vault persistence; no test for
`script-executor.ts` (the known globals-mapping defect is untested).

---

## Feature list

### P0 — core execution and object model

1. Pre-request and after-response script execution with per-request timeout (default 30 s) and
   sequential execution. (`network.ts:274-397,743-802`, `concurrency.renderer.ts:56-80`)
2. Isolated script context with injected `insomnia`, `console`, `_` (lodash-compatible),
   `setTimeout`, `$` alias, strict mode default, and returned-context merging.
   (`run-script.ts:31-94`)
3. `insomnia.environment`, `baseEnvironment`, `variables` (7-scope precedence), `globals`,
   `baseGlobals`, `iterationData`, `collectionVariables` alias. (`environments.ts:195-367`)
4. `insomnia.request` full mutation API (URL, query, headers, body, auth) and
   `insomnia.request.size()`. (`request.ts:282-471`)
5. `insomnia.response` read API (`body`, `code`, `headers`, `cookies`, `json()`, `text()`,
   `responseTime`, `originalRequest`). (`response.ts:54-207`)
6. `insomnia.test` / `.skip` / `insomnia.expect` with chai, and `response.to.*` assertions.
   (`test.ts:6-61`, `response.ts:208-357`)
7. Error surfacing: `SECURITY_POLICY_VIOLATION` passthrough, prefixed runtime errors, timeout
   messages, timeline logging. (`entry.hidden-window.ts:34-68`, `network.ts:692-703`)
8. Persistence of script mutations (env, base env, globals, cookies, request) after execution.
   (`network.ts:403-487`)
9. `console.*` capture into the request timeline. (`console.ts:69-127`, `network.ts:663-665`)
10. `insomnia.sendRequest` (callback style) executing a sub-request without side effects.
    (`insomnia.ts:106-111`, `send-request.ts:20-45`)
11. `insomnia.info` (`eventName`, `iteration`, `iterationCount`, `requestName`, `requestId`) and
    `insomnia.execution.location.current`. (`request-info.ts:26-100`, `execution.ts:7-40`)

### P1 — parity features

12. `require` allowlist with 12 modules + 8 Node built-ins, `insomnia-collection` /
    `postman-collection` namespaces. (`require-interceptor.ts:14-99`)
13. Static AST sandbox (18 blocked properties, 12 blocked roots) with alias/destructuring
    tracking and import blocking. (`sandbox.ts:74-254`, `script-security-rules.ts:14-150`)
14. Runtime interceptors for `require`, `window`, `eval`, plus 9 mask rules.
    (`script-security-policy.ts:9-60`, `script-security-rules.ts:152-216`)
15. Security settings UI with per-rule disabling. (`scripting-settings.tsx:151-411`)
16. Cookie API: `insomnia.cookies` list object plus `jar()` callback API; cookie jar persisted
    back with source `'manual'`. (`cookies.ts:180-381`)
17. Vault access gated by `enableVaultInScripts`, with decrypt-on-use. (`insomnia.ts:180-183`,
    `network.ts:524-534`, `environments.ts:389-432`)
18. Client certificate selection per request and certificate/request merge rules.
    (`insomnia.ts:210-239`, `request.ts:497-578`)
19. Proxy resolution from settings and `transformToSdkProxyOptions`. (`proxy-configs.ts:299-382`)
20. `insomnia.parentFolders` and folder-level environment resolution.
    (`folders.ts:69-147`, `network.ts:296-305`)
21. `insomnia.execution.skipRequest()` and `setNextRequest()`. (`execution.ts:25-40`)
22. Postman script translation on import and paste (`pm.` → `insomnia.` plus ~40 preprocess
    rules). (`translate-postman-script.ts:18-208`)
23. Async task tracking via `ProxiedPromise` and the four bridge operations.
    (`async-objects.ts:11-84`, `run-script.ts:43-54`)
24. Interpolation inside scripts: LiquidJS `{{ }}` / `{% %}` and `{{$faker}}`.
    (`interpolator.ts:8-53`)
25. Autocomplete catalog of 227 snippets and editor snippet menus.
    (`autocomplete-snippets.json`, `request-script-editor.tsx:157-505`)

### P2 — legacy, optional, or experimental

26. QuickJS-WASM sandbox (disabled by kill switch; reduced API surface, text bodies only).
    (`run-script-quickjs.ts:18`, `quickjs-script-engine.ts:20-277`)
27. Node main-process script executor (`script-executor.ts`) with the globals-mapping defect.
28. Legacy Mocha unit-test framework: code generation, temp-file execution, `insomnia.send()`,
    60 s timeout, `enableLegacyUnitTests` flag. (`packages/insomnia-testing/`)
29. `ProxyConfig.updateProtocols()` (throws), `PropertyList.eachParent()` (throws),
    `Response.jsonp()` (throws), `Promise.any` / `Promise.withResolvers` (rejected stubs).
30. jQuery-style `$` alias (only aliases `insomnia`, not jQuery).

### Known defects / decisions to replicate or fix

- `response.dataURI()` builds a base64 URI but the encoding label is misspelled `baseg4`
  (`response.ts:164-172`).
- `insomnia.settings` always returns `undefined` (`insomnia.ts:125-127`).
- `script-executor.ts:99-106` maps globals using the environment `id`/`name`.
- `Vault.set` is documented as a setter but always throws when disabled; `unset`/`clear` always
  throw (`environments.ts:399-432`).
- Unparseable scripts bypass static checks entirely (`sandbox.ts:88-92`).
- Rules disabled via settings are removed from the policy, but `require`/`window`/`eval`
  interceptors remain installed (`sandbox.ts:40,326-330`).
- Postman `pm.response.to.have.*` is translated to the chai namespace, so `have`-style chains do
  not exist; the translator's regex leaves some constructs untranslated by design
  (`translate-postman-script.ts:18-188`).
