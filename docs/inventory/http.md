# HTTP pipeline

Scope: the plain HTTP request path of Insomnia — renderer send action, `sendCurlAndWriteTimeline`,
`curlRequest`/`createConfiguredCurlInstance` (node-libcurl), the streaming/event-source variant
(`curl.open`), auth header injection, cookie jar, client/CA certificates, proxy, redirects,
timeouts, response and timeline persistence. Paths are relative to `D:\Projects\Heric\Insomnia`.
`packages/insomnia/src/main/network/libcurl-promise.ts` = **LP**, `packages/insomnia/src/network/network.ts` = **NET**,
`packages/insomnia/src/main/network/curl.ts` = **CURL**.

## Flow of one request

1. UI route action `sendActionImplementation` (`packages/insomnia/src/routes/organization.$organizationId.project.$projectId.workspace.$workspaceId.debug.request.$requestId.send.tsx:116`):
   `window.main.startExecution({ requestId })` (:139), `fetchRequestData(requestId)` (:140, impl **NET**:164) loads request, environment (+ global/base variants), cookie jar (`services.cookieJar.getOrCreateForParentId`, **NET**:199), settings, client certificates (**NET**:218), CA cert (**NET**:219), `responseId` + `timelinePath` (**NET**:221-222). A `SensitiveValueCollector` is created from `hideSecretValuesInPreviewAndConsole` (:142) and pre-filled from every confidential KV pair across the five environment layers (`collectConfidentialRawValues`, :148; `packages/insomnia/src/common/render.ts:333`). All later timeline writes go through `withRedaction(runtime, collector)` (:163), which re-parses each NDJSON line and redacts only string leaves (`packages/insomnia/src/common/network/sensitive-value-collector.ts:63`, `redactTimelineLine` note at :88-97 — redacting raw JSON would corrupt `timestamp`).
2. Execution step "Executing pre-request script" (:175) → `tryToExecutePreRequestScript` (**NET**:274). Errors (`'error' in mutatedContext`) and `execution.skipRequest` produce a `Response` with `statusMessage: 'Error' | 'Cancelled'` and no network call (:187-222).
3. Step "Rendering request" (:232) → `tryToInterpolateRequest` (**NET**:824) → `tryToTransformRequestWithPlugins` (:244) which first applies plugin-supplied `DEFAULT_HEADERS` (`packages/insomnia/src/network/apply-default-headers.ts:6`), then request hooks (`packages/insomnia/src/runtimes/network/network-adapter.renderer.ts:55-69`). `parseGraphQLReqeustBody(renderedRequest)` rewrites GraphQL bodies (:249).
4. Step "Sending request" (:250) → `sendCurlAndWriteTimeline(renderedRequest, clientCertificates, caCert, settings, timelinePath, responseId, redactingRuntime)` (:251, impl **NET**:875).
   - `transformUrl` (**NET**:888, impl :1031) appends query params incl. `apikey`-in-query (`getAuthQueryParams` :1016), runs `smartEncodeUrl` unless `settingEncodeUrl` false, and extracts a Unix socket from `http(s)://unix:/<socket>:/<path>` into `{ finalUrl, socketPath }` (:1053-1055).
   - Timeline seeds: "Preparing request to <finalUrl>", "Current time is <ISO>", "Enable/Disable automatic URL encoding" (**NET**:895-903); "Disable cookie sending due to user setting" if `!settingSendCookies` (:905).
   - `getRuntime().network.getAuthHeader(renderedRequest, finalUrl)` (:907) — in the renderer this is IPC `getAuthHeader` (`packages/insomnia/src/main/ipc/main.ts:486`), so OAuth flows run in main. Returned timeline entries are appended (:908-910).
   - `requestOptions` (:911-920): `certificates = filterClientCertificates(clientCertificates, renderedRequest.url, 'https:')` (:917), `caCertficatePath = caCert?.disabled === false ? caCert.path : null` (:918).
   - `getRuntime().network.executeCurlRequest(requestOptions)` (:922): renderer = `cancellableCurlRequest` (`packages/insomnia/src/network/cancellation.renderer.ts:38`) → IPC `curlRequest` (`main.ts:662`); node/inso = direct `curlRequest` (**LP**:104).
5. Main process `curlRequest` (**LP**:104): reads `INSOMNIA_DATA_PATH`/userData → `responses/<uuid>.response` (:107-111); rejects `file://` (:125); reads CA PEM via `insecureReadFile` (:126); `createConfiguredCurlInstance` (:128) → method selection, body/multipart, headers, `WRITEFUNCTION`, `DEBUGFUNCTION`, `CurlFeature.Raw`; `curl.perform()` (:278).
6. Completion: `end` → patch `{ bytesContent, bytesRead: SIZE_DOWNLOAD, elapsedTime: TOTAL_TIME*1000, url: EFFECTIVE_URL }` (:234-240); `_parseHeaders(rawHeaders)` (:559) yields one `HeaderResult` per redirect hop; buffer body flushed (`waitForStreamToFinish`, :583); `resolve({ patch, debugTimeline, headerResults, responseBodyPath })` (:243). Error path (:250-276): `CURLE_BAD_CONTENT_ENCODING` retries the whole request with `noDecompress: true` (:257-261, recursion), `CURLE_ABORTED_BY_CALLBACK` → `statusMessage 'Abort'`, error "Request aborted" (:265-268).
7. Back in **NET**: `'error' in output` early return with `elapsedTime: 0` (:924-940 — comment says this branch is hit during plugin calls); `debugTimeline` merged (:943); `extractCookies` (:945, adapter `runtimes/network/network-adapter.node.ts:57`, renderer :26) only when `settingStoreCookies`; rejected cookies and "Saved N cookies" appended (:951-956); `services.cookieJar.update` (:955); `lastRedirect = headerResults.at(-1)` (:958); timeline flushed via runtime (:960-962); returns `{ _id: responseId, parentId, timelinePath, bodyPath, contentType, headers, httpVersion, statusCode, statusMessage, cookies, timeline, ...patch }` (:964-978).
8. `responseTransform` (**NET**:981) adds `environmentId`, `globalEnvironmentId`, cookie settings, runs plugin response hooks (`network-adapter.renderer.ts:71-86`); on hook failure returns an error patch (:1005-1013).
9. Step "Executing after-response script" (:293) → `tryToExecuteAfterResponseScript` (**NET**:743), which persists mutated env/cookie jar (`savePatchesMadeByScript` **NET**:402) merging response cookies (:434).
10. Persistence: `redactResponsePatch(...)` (:348-357) → `services.response.create(...)` or save-to-file path (`writeToDownloadPath` :82, used at :370/:380); `requestMeta.activeResponseId` updated (:361-362); runner timing = sum of execution-step durations (:333-336); analytics `requestExecuted` (:393+).
11. Inso/collection runner uses the same `sendCurlAndWriteTimeline` from `packages/insomnia/src/network/send-request.node.ts:95`; plugins (`packages/insomnia/src/plugins/context/network.ts:49`) and GraphQL schema introspection (`packages/insomnia/src/ui/components/editors/body/graph-ql-editor.tsx:163`) and mock "new-mock-send" (`packages/insomnia/src/routes/...new-mock-send.tsx:61`) reuse it. `network.sendRequestWithoutSideEffects` bypasses it and calls `curlRequest` directly (`packages/insomnia/src/main/templating-worker-database.ts:817-843`; `plugins/context/network.ts:67-100`), forcing `authentication: {type:'none'}`, `certificates: []`, `caCertficatePath: null`, and `proxyEnabled=false` unless `proxyScope === 'all'` (templating-worker-database.ts:824-827, plugins/context/network.ts:69-73) — those are privilege boundaries against script/plugin-steered file reads and credential injection.
12. Streaming/event-stream variant: IPC `curl.open` (**CURL**:137, registered :498) creates a separate connection keyed by rendered request id; duplicates are rejected (:143-148). It reuses `createConfiguredCurlInstance` (:199), disables request-level cookies (`cookies: []`, :200), sets `CUSTOMREQUEST` (:206) and `POSTFIELDS` (:208) unconditionally, enables `CurlFeature.StreamResponse` (:211), attaches `DEBUGFUNCTION` (:247) and a `'stream'` handler (:275) that writes a `Response` immediately with `elapsedTime: performance.now() - start` (:176, :297), streams NDJSON events into `bodyPath` (:330-341), stores Set-Cookie via `addSetCookiesToToughCookieJar` (:313-324), and closes on remote end (:343-357). `curl.event.findMany` (:462) reverses events and content-sniffs with `isCurlEvent` (:450-458) so a plain JSON body is never mistaken for an event log (tests: `main/network/curl.test.ts:97-115`).
13. Cancellation: renderer `cancelRequestById` (`cancellation.renderer.ts:5`) → IPC `cancelCurlRequest` (`main.ts:666`) → `cancelCurlRequestHandlers[requestId]` closes the curl handle and the multipart read FD (**LP**:103, :187-192, cleanup :242/:253). Streaming connections are closed via `curl.close`/`curl.closeAll` and on `window-all-closed` (**CURL**:509).

Edge cases
- Timeline writes are append-only; if any stage throws before the first flush, the persisted timeline can be shorter than the in-memory one — a response document may refer to a partial timeline (**NET**:924-940, :960-962). P1.
- Each `sendCurlAndWriteTimeline` call allocates a fresh `responseId`/timeline path via `fetchRequestData`, but GraphQL introspection and plugin sends persist responses too — dedupe/history pruning is `maxHistoryResponses` (default 20, `packages/insomnia-data/src/models/settings.ts:62`). P1.
- Basic-auth derived header is registered for redaction twice: at render and after `getAuthHeader` in the streaming path (**CURL**:184-194). Missing either leaks base64 credentials to the timeline. P0.
- Streaming path registers multi-line auth timeline via `writeTimelineEntry` (:195) only after `sensitiveValues.reset` (:158); values are per-request and cleared in `deleteRequestMaps` (:152, :411-413).
- `curlRequest` retry on `CURLE_BAD_CONTENT_ENCODING` re-sends the request, duplicating side effects (POST twice). P1 (see error path above).

## libcurl options and what they control

All options are set on a fresh `new Curl()` in `createConfiguredCurlInstance` unless marked CURL/streaming.

| Option | Value / condition | Where | Notes & edge cases |
|---|---|---|---|
| `URL` | `finalUrl` | **LP**:362 | Streaming passes `req.url` (:200). |
| `UNIX_SOCKET_PATH` | `socketPath` when present | **LP**:363 | Only from `http(s)://unix:/...` URL form (**NET**:1053). No platform guard; Windows depends on the libcurl build. P2 |
| `VERBOSE` | `true` | **LP**:368 | Required for `DEBUGFUNCTION` to fire. P0 |
| `NOPROGRESS` | `true` | **LP**:370 | Prevents progress output. |
| `ACCEPT_ENCODING` | `''` (all supported) or `null` when `noDecompress` | **LP**:372 | Sends no Accept-Encoding header by design — `parse-header-strings.ts:83-85` suppresses the default one, so decompression is applied to whatever the server returns. Retry-without-decompress on `CURLE_BAD_CONTENT_ENCODING` (:257). P1 |
| `CAINFO_BLOB` | workspace CA PEM | **LP**:375 | Read with `insecureReadFile` (:126). P0 |
| `SSL_OPTIONS` | `CurlSslOpt.NativeCa` | **LP**:378 | Always uses the OS trust store, in addition to `CAINFO_BLOB`. P0 |
| `SSLCERT`/`SSLCERTTYPE` | `cert` + `PEM`, or `pfx` + `P12` | **LP**:385-386, :393-394 | Paths must pass `isPathAllowed(…, settings.dataFolders)` (:382-403). If both cert and pfx exist, pfx wins (set later). P0 |
| `SSLKEY` | `key` path | **LP**:401 | Allowlist-checked. |
| `KEYPASSWD` | `passphrase` | **LP**:405 | Applied even when no cert/key set. P2 |
| `HTTP_VERSION` | from `preferredHttpVersion` | **LP**:412, map :637-661 | Values `V1_0,V1_1,V2PriorKnowledge,V2_0,v3,default` (`insomnia-data/common-src/settings.ts:75-84`). Unknown value logs "Unknown HTTP version" and sets nothing. P0 |
| `MAXREDIRS` | `maxRedirects` only if `> 0` | **LP**:415-419 | `0`/negative leaves libcurl default (unlimited); comment warns `-1` breaks some libcurl versions. Default 10 (settings.ts:63). P0 |
| `PROXYTYPE`+`PROXY` | system proxy (PAC parse) | **LP**:431-433, `parseResolvedProxy` :296-341 | Only first PAC entry; `DIRECT`→`PROXY ''`. |
| `PROXY` | manual `httpProxy`/`httpsProxy` by URL scheme, `''` on bypass | **LP**:438-443 | `setDefaultProtocol` adds a scheme; `shouldBypassProxyForHost` (:664-681) handles exact host and leading-dot suffix only. P0 |
| `PROXYAUTH` | `CurlAuth.Any` when proxy set | **LP**:445 | Credentials come only from proxy URL userinfo (no prompt). P1 |
| `NOPROXY` | raw `noProxy` | **LP**:447-450 | Passed so libcurl also handles IP/CIDR entries (comment). |
| `TIMEOUT_MS` | `settings.timeout` or `0` | **LP**:452-457 | Whole-transfer timeout; default 30 000 ms (settings.ts:78). No `CONNECTTIMEOUT` is set — a stalled connect is bounded only by the total. P0 |
| `SSL_VERIFYHOST`/`SSL_VERIFYPEER` | `0` when `validateSSL` false | **LP**:461-465 | OAuth token calls substitute `validateAuthSSL` (o-auth-2/get-token.ts:667). P0 |
| `FOLLOWLOCATION` | request `'global'|'on'|'off'` → settings default | **LP**:472-479 | `'global'` falls back to `settings.followRedirects` (default true). P0 |
| `PATH_AS_IS` | `true` when `!settingRebuildPath` | **LP**:481-484 | Prevents dot-segment normalization. P1 |
| `COOKIEFILE` | `''` when `settingSendCookies` | **LP**:486-488 | Enables the in-memory cookie engine. P0 |
| `COOKIE` | `name=value` per `req.cookies` | **LP**:490-492 | Renderer always emits `cookies: []` (`common/render.ts:957`), so in practice only the jar is used; any value here would be sent to every host. P1 |
| `COOKIELIST` | one Netscape-format line per jar cookie | **LP**:493-513 | `#HttpOnly_` prefix, `hostOnly ? FALSE : TRUE`, expiry seconds (`0` for session/invalid). P0 |
| `USERAGENT` | first `user-agent` header, else `insomnia/<version>`; `''` if `suppressUserAgent` | **LP**:516-523 | The same header may also still be in `HTTPHEADER`; libcurl lets the custom header list win. P1 |
| `HTTPAUTH`+`USERNAME`/`PASSWORD` | digest or NTLM when no Authorization header, not disabled | **LP**:525-534 | Handled by libcurl challenge/response, not by `getAuthHeader`. P0 |
| `NETRC` | `CurlNetrc.Required` | **LP**:535-537 | Fails if no netrc entry. P2 |
| `CUSTOMREQUEST` | method, except HEAD/POST | **LP**:140-148 | GET always goes through `CUSTOMREQUEST`; HEAD uses `NOBODY`, POST uses `POST=1`. P0 |
| `NOBODY` | `1` for HEAD | **LP**:141 | |
| `POST` | `1` for POST | **LP**:142-144 | |
| `INFILESIZE_LARGE`+`UPLOAD`+`READDATA` | body-file path | **LP**:162-171 | Multipart or file body; AWS `iam` + file body is rejected (:160-161); FD closed and multipart temp deleted on end/error (:170-171, `closeReadFunction` :542-551). P1 |
| `POSTFIELDS` | body text (or urlencoded rebuild) | **LP**:172-174, `parseRequestBody` :603-618 | Urlencoded bodies are rebuilt from `body.params` with `URLSearchParams`; any other body with a mimeType or POST/PUT/PATCH/QUERY uses `body.text`. GET/DELETE without mimeType send no body. P0 |
| `HTTPHEADER` | `parseHeaderStrings(...)` output | **LP**:181-182 | Full header assembly below. P0 |
| `WRITEFUNCTION` | appends to file stream, counts bytes | **LP**:196-200 | Body never buffered in memory; `bytesContent` vs `bytesRead` (`SIZE_DOWNLOAD`) can differ when compressed. P0 |
| `DEBUGFUNCTION` | timeline lines | **LP**:202-228 | Suppresses SSL data and "Added cookie" (dumps the whole jar), hides `DataOut` above `maxTimelineDataSizeKB`, summarizes `DataIn` as "Received N chunk". P0 |
| `CurlFeature.Raw` | enabled | **LP**:231 | Gives raw header bytes so duplicate `Set-Cookie` survives (`_parseHeaders` :559). P0 |
| `CurlFeature.StreamResponse` (streaming only) | enabled | **CURL**:211 | Switches to `'stream'` event. P1 |

Header assembly (`packages/insomnia/src/network/parse-header-strings.ts`):
- `Expect` and `Transfer-Encoding` are suppressed whenever a body/file is present (:37-43). Without this, curl adds `Expect: 100-continue` and hangs on some servers. P0.
- Auth header is appended only if no user `Authorization` header, auth not disabled, and type is not `iam`/`digest`/`ntlm` (:50-54). IAM is signed in-process with `aws4` (:55-66, `_getAwsAuthHeaders` :118-149) and drops `content-type` from the signature headers (:147-149).
- Multipart forces `Content-Type: multipart/form-data; boundary=X-INSOMNIA-BOUNDARY` (:68-76; boundary constant `network/multipart-constants.ts:1`). Edge: the boundary is fixed, so a part value containing it corrupts the body. P2.
- Defaults injected: `Accept: */*` (:78-80); `Accept-Encoding` suppressed (:83-85); `content-type` suppressed (:88-90) so curl does not add one. Empty values become `Name;` and the sentinel `__Di$aB13d__` becomes `Name:` (curl syntax for "do not send"), :92-100. P0.
- Header merging/overrides happen earlier: `getOrInheritHeaders` (**NET**:77-111) concatenates duplicates with `, ` except `SINGLE_VALUE_HEADERS` (`common/common-headers.ts:8-19`: proxy-authorization, content-length, content-type, content-encoding, content-location, connection, host, upgrade, range, trailer), preserves last-seen casing, sorts by first appearance. P0.

## Timeline phases

Two independent concepts share the word "timeline".

A. Persisted response timeline (`ResponseTimelineEntry { name, value, timestamp }`, NDJSON in `<responseId>.timeline`; renderer prefixes `HeaderIn '< '`, `DataIn '| '`, `SslDataIn '<< '`, `HeaderOut '> '`, `DataOut '| '`, `SslDataOut '>> '`, `Text '* '` — `packages/insomnia/src/ui/components/viewers/response-timeline-viewer.tsx:13-23`). Ordered phases:
1. Client-side seeds: preparing / current time / URL-encoding state (**NET**:895-903); cookie-send note (:905).
2. Auth timeline, `[oauth2] …` prefixed (`o-auth-2/get-token.ts:161-165`), including a copy of the token-request's own timeline between "--- token request/response details ---" markers (:379-386) and refresh details (:484-491); errors return the same timeline (:91-94 in `get-auth-header.ts`).
3. Configuration debug entries from `createConfiguredCurlInstance`: HTTP version (:408-410), "Adding SSL PEM/P12/KEY certificate" (:387, :395, :402), timeout (:457), "Enable/Disable SSL validation" (:467-471), "Using system proxy: …" (:433) or "Using proxy: …" (:444), "Enable cookie sending with jar of N cookie(s)" (:493-497).
4. libcurl debug entries: `HeaderOut`, `HeaderIn`, `DataOut` (text, hidden above `maxTimelineDataSizeKB`, default 10 KB — settings.ts:64; code :214-219), `DataIn` summarized per chunk (:220-224), `Text`. SSL payloads and per-domain cookie additions are dropped (:203-208). P0 for the Console tab.
5. Cookie outcome: "Rejected cookie: <err>" (**NET**:951-953) and "Saved N cookies" (:955-956).
6. On the streaming path, timeline entries come from the same debug callback (**CURL**:247-271) plus a "Preparing request to <url>" seed (:128) and the teardown message written by `deleteRequestMaps` (:401-413).
- Redaction happens only in `runtime.appendTimeline` (**NET**:961) via `withRedaction`; the streaming path redacts at write time (`writeTimelineEntry` **CURL**:80-84). When the hide setting is off, the collector is `null` and nothing is redacted (`sensitive-value-collector.ts:290`).

B. Execution steps (in-memory, per `requestId`): `TimingStep { stepName, startedAt, duration }` (`packages/insomnia/src/main/network/request-timing.ts:3-8`), mutated by `addExecutionStep` (:14-25), `completeExecutionStep` (:27-35), `updateLatestStepName` (:37-48), and broadcast to every window on the dynamic channel `syncTimers.<requestId>` (:23, :33, :46). Consumers: `packages/insomnia/src/ui/hooks/use-execution-state.ts:30` and `ui/components/response-timer.tsx:37`. Steps emitted by the send action: "Executing pre-request script" (:175), "Rendering request" (:232), "Sending request" (:250), "Executing after-response script" (:293). "Is executing" = last step has no `duration` (`use-execution-state.ts:39-46`). Runner duration is the sum of step durations (`send.tsx:333-336`). P1.

C. Timings reported per response: `elapsedTime` = `CURLINFO_TOTAL_TIME` × 1000 (**LP**:238) for HTTP; `performance.now() - start` for streaming (**CURL**:176, :297); hard-coded `0` on the plugin/runtime error branch (**NET**:931) and on script errors. `bytesRead` = `SIZE_DOWNLOAD`, `bytesContent` = bytes written (**LP**:236-237).

Edge cases
- `maxTimelineDataSizeKB` uses `|| 1`, so a configured `0` silently means 1 KB (**LP**:217, **CURL**:262). P2.
- Timeline order is wall-clock write order across processes; auth entries are generated before curl config entries. P2.
- `elapsedTime: 0` on error makes the UI show 0 ms instead of the real time-to-failure (see above). P1.
- Streaming timeline file is keyed by requestId while its response body lives in the same file family as normal bodies; `findMany` must content-sniff (**CURL**:450-458, tests `main/network/curl.test.ts:97-115`). P1.

## Auth integration points

Dispatch: `getAuthHeader` (`packages/insomnia/src/main/network/get-auth-header.ts:28`), called from **NET**:907 (HTTP), **CURL**:184 (streaming), and HAR export (`main/har.ts:310`, masked variant). It returns `{ header?, timeline? }` and never throws for OAuth2 (errors are swallowed into the timeline, :91-94).

| `authentication.type` | Mechanism | Location | Edge cases / priority |
|---|---|---|---|
| any, `disabled: true` | nothing | :34-36 | Renderer also blanks the object (`common/render.ts` filtering at :912-918 of that function region). P0 |
| `apikey` + `addTo: header` | header `{key, value}` | :38-51 | Missing key or value → no header. P0 |
| `apikey` + `addTo: cookie` | `Cookie: key=value` | :53-64 | Replaces the whole Cookie header; jar cookies are still injected by curl's engine, so both can be sent. P1 |
| `apikey` + `addTo: query` | query param | **NET**:1016-1028, joined in `transformUrl` (:1031) | Applied before curl sees the URL, so signatures/certs use the final URL. P1 |
| `basic` | `getBasicAuthHeader(username, password, useISO88591 ? 'latin1' : 'utf8')` | :66-70; `network/basic-auth/get-header.ts:5-15` | UTF-8 by default; ISO-8859-1 opt-in. Derived base64 is registered for redaction (**CURL**:184-194). P0 |
| `bearer` | `getBearerAuthHeader(token, prefix)`; `NO_PREFIX` sends the raw token | :72-75; `network/bearer-auth/get-header.ts:3-11`; prefix handling :17-26 | Trims token and prefix. P0 |
| `oauth2` | `getOAuth2Token(tokenId, auth)`; `.graphql` suffix stripped so schema fetches share tokens | :77-95 (stripping :83) | Token request itself uses `sendCurlAndWriteTimeline` (o-auth-2/get-token.ts:663) with a synthetic request (`_id = id + '.other'`, POST, urlencoded, `authentication: none`, :619-637) and client certs; `validateSSL` overridden by `validateAuthSSL` (:667). P0 |
| `oauth1` | `oauth-1.0a` header | :97-110; `o-auth-1/get-token.ts:43-116` | HMAC-SHA1/SHA256, RSA-SHA1 (signing key = privateKey, :104-113), PLAINTEXT (:15-41). Body hash only for `x-www-form-urlencoded` (:82-88). P1 |
| `hawk` | `Hawk.client.header` | :112-140 | Optional payload validation includes body text + content type (:122-139). P2 |
| `asap` | lazy `httplease-asap` generator | :142-170 | `additionalClaims` must parse to an object or it throws (:143-152); token expiry 10 min / max age 9 min (:161-162). P2 |
| `digest`, `ntlm` | libcurl `HTTPAUTH` + `USERNAME`/`PASSWORD` | **LP**:525-534 | Only when no user Authorization header; challenge-based. P0 |
| `netrc` | `CURLOPT_NETRC = Required` | **LP**:535-537 | Fails when no matching netrc entry. P2 |
| `iam` (AWS) | `aws4.sign` in `parseHeaderStrings` | `parse-header-strings.ts:49-66`, :118-149 | Signing host prefers a user `Host` header (:137-139). File-body upload unsupported (**LP**:159-161). P1 |
| `singleToken` | HTTP send path: no handler; only Socket.IO applies it | `common/constants.ts:262`; `main/network/socket-io.ts:345` | A `singleToken` HTTP request sends no auth. P1 |
| `none` | nothing | :172-173 | |

OAuth2 flow details (`packages/insomnia/src/main/network/o-auth-2/get-token.ts`)
- Token cache per auth id with folder inheritance and request override (`getExistingAccessTokenAndRefreshIfExpired`, declared ~:536; usage :180-187); expired/forced refresh via `refresh_token` (:470-543), `401` clears the access token (:495-500), `invalid_grant` also clears (:506-514).
- Grants: `implicit` (:200-261, token read from URL hash, :248-259), `authorization_code` (:263-350) with PKCE S256/plain (`usePkce`, verifier 32 random bytes, :266-273), `password` (:351-358), `client_credentials` (:359-370), `refresh_token` (above), `mcp_auth_flow` returns no token and is handled by the MCP layer (:174-176, constant :53).
- Client credentials go in the body (`credentialsInBody`) or as Basic (`getBasicAuthHeader`, :368-377 and :475-483).
- Interactive authorization uses a persistent partitioned Electron session (`initNewOAuthSession` :101-106, `getOAuthSession` :108-112); default-browser mode relays the callback encrypted with RSA-OAEP + AES-256-GCM (:114-154).
- Responses map to `OAuth2Token` with `expiresAt = now + expires_in*1000` (:580-595); error/token bodies parsed as JSON then urlencoded (:578-592 region, `tryToParse` ~:603-612).

Edge cases
- The auth header is computed from the **rendered** request, and `parseHeaderStrings` refuses to add it when the user already set `Authorization` — so a stale manual header silently wins. P0.
- Bearer with an empty `prefix` string yields `"Bearer <token>"`; `NO_PREFIX` is the only way to omit it (`get-auth-header.ts:17-26`). P2.
- OAuth2 token exchange can itself trigger a proxy/cert path; it uses the same settings object, so `proxyEnabled` applies to token endpoints too. P1.
- Module-level token cache is in-memory only; a token is not shared across processes (`getOAuth2Token` is main-only). P2.

## Cookies, certificates, proxy, redirects, timeouts

Cookies (send)
- Only when `settingSendCookies` (default true, `insomnia-data/src/models/request.ts:363`): cookie engine enabled and jar cookies injected as `COOKIELIST` lines (**LP**:486-513). Manual cookies are rendered with template syntax, server-set cookies are explicitly not re-rendered (`common/render.ts:887-908`). P0.
- Redirect hops all receive cookies because libcurl's engine owns matching; the response jar update covers all hops since `headerResults` holds every header block and `getSetCookiesFromResponseHeaders` flat-maps them (**NET**:945-950, `_parseHeaders` **LP**:559-580). P0.

Cookies (store)
- Only when `settingStoreCookies` (default true): `extractCookies` → `addSetCookiesToToughCookieJar` (`network/set-cookie-util.ts:8`). Jar built with loose mode and `rejectPublicSuffixes = false` (`common/cookies.ts:43-44`). Cookie identity is `domain\0path\0key`; cookies touched by this response are stamped `source: 'response'`, others keep their identity/source (:34-42). Errors (invalid values/domains) become "Rejected cookie" timeline rows (**NET**:951-953). P0.
- `currentUrl` for parsing is the last redirect's `Location` resolved against `finalUrl` (**NET**:1060-1074); without one it is `finalUrl`. P1.
- Persistence writes the whole jar (`services.cookieJar.update`, **NET**:955), so concurrent requests in the runner can clobber each other (last writer wins). P1.
- The streaming path stores cookies independently (**CURL**:308-325) but never marks redaction-sensitive values; Set-Cookie values are not rendered so that is acceptable. P2.
- Cookie jar IPC surface: `cookies.fromJSON|parse|toString|getCookiesForUrl|addSetCookies|getResponseCookiesFromHeaders` (`packages/insomnia/src/main/ipc/cookies.ts:66-84`); `getCookiesForUrl` filters by URL for HAR export (`main/har.ts:337`) and the Cookie Manager UI — the HTTP sender does **not** URL-filter before handing cookies to curl. P1.
- Cookie/manual edits flow back into the request render through `savePatchesMadeByScript` (**NET**:402-437), which merges response cookies and tags script-created cookies as `manual` (:420-424).

Certificates
- Client certs are workspace-scoped, filtered per request by host/port with wildcard support: first pass requires port match, second pass ignores port (`insomnia-data/src/models/client-certificate.ts:44-56`; matching `insomnia-data/common-src/url-matches-cert-host.ts:6-58`, default port 443, `*` → `.*`). Disabled certs excluded. P0.
- Applied as `SSLCERT` (+`PEM`/`P12`), `SSLKEY`, `KEYPASSWD` (**LP**:380-406) after `isPathAllowed` against `settings.dataFolders` (:382-403) — scripts cannot point certs at arbitrary files. P0.
- The filter uses `renderedRequest.url` (**NET**:917) while curl is given `finalUrl`; these differ only by appended query params, so host/port matching is unaffected. P2.
- CA cert: mapped only when `caCert?.disabled === false` (**NET**:918) and read in main via `insecureReadFile` (**LP**:126); streaming reads it before its `try` block, which is why a missing file can escape cleanup (regression covered by `main/network/curl.test.ts:23-41`). P0.
- `validateSSL=false` disables both hostname and peer verification (**LP**:461-465). Per-request override does not exist; only `validateAuthSSL` for OAuth token calls. P0.
- mTLS also applies to OAuth token requests and GraphQL introspection because they reuse `sendCurlAndWriteTimeline`. P1.

Proxy
- `proxyEnabled=false` (default, settings.ts:72): per-request system-proxy resolution via Electron `session.defaultSession.resolveProxy(finalUrl)` → PAC string parsed by `parseResolvedProxy` (**LP**:296-341, call :421-435). Maps `PROXY|HTTP→Http`, `HTTPS→Https`, `SOCKS|SOCKS4→Socks4`, `SOCKS5→Socks5`, `DIRECT` or unknown → `null`; only the first entry is honored. On `null`/error the code sets `PROXY ''`, deliberately overriding libcurl's own environment-variable proxy lookup (:428-436). P0.
- `proxyEnabled=true`: scheme-specific `httpsProxy`/`httpProxy`, `setDefaultProtocol` applied, JS-side bypass via `shouldBypassProxyForHost` (**LP**:664-681 — exact host or leading-dot suffix only), raw `noProxy` also handed to libcurl for IP/CIDR (:447-450), `PROXYAUTH Any` (:445). Empty proxy means direct. P0.
- App-wide proxy (Chromium/net.fetch/PAC) is configured separately by `watchProxySettings` (`packages/insomnia/src/main/proxy.ts:116`, invoked at startup `entry.main.ts:150`): `fixed_servers` rules for http/https (:84-102), bypass list = `noProxy` plus Insomnia's own hosts unless `proxyScope === 'all'` (:92-95, host list :23-69); on failure falls back to `mode: 'system'` (:104-113). Because libcurl's "system proxy" branch reads `defaultSession.resolveProxy`, this config indirectly drives curl when `proxyEnabled=false`. P0/P1.
- `network.sendRequestWithoutSideEffects` forces `proxyEnabled=false` unless `proxyScope === 'all'` (`templating-worker-database.ts:824-827`; `plugins/context/network.ts:69-73`) so plugins/scripts cannot exfiltrate through the user's proxy by default. P0.
- No proxy authentication UI/prompt; only URL userinfo. P1.
- Test coverage: `main/network/libcurl-promise.test.ts:41-63` (protocol-matched manual proxy, noProxy bypass).

Redirects
- `FOLLOWLOCATION` from request setting with global fallback (**LP**:472-479); `MAXREDIRS` only if positive (:415-419); `PATH_AS_IS` when rebuild-path is off (:481-484). P0.
- No `UNRESTRICTED_AUTH` is set, so libcurl's default of not forwarding credentials to a different host applies. P1.
- Header parsing returns one block per hop; the response uses the **last** block for status/headers/HTTP version (**NET**:958, :969-975). Intermediate 1xx or proxy-CONNECT blocks are not filtered — a proxy CONNECT block can shift what "last" means. P1.
- Cookie storage URL follows the last `Location` (see above). P1.
- The UI only shows the final response body (curl discards intermediate bodies). P2.

Timeouts
- Single `TIMEOUT_MS` for the whole transfer (**LP**:452-457), default 30 s; `timeout <= 0` disables it. There is no connect timeout, no low-speed abort, and no separate TLS-handshake timeout. P0/P1.
- Cancel maps to `CURLE_ABORTED_BY_CALLBACK` → `statusMessage 'Abort'` with error text "Request aborted" (**LP**:265-268); the renderer returns `{ statusMessage: 'Cancelled', error: 'Request was cancelled' }` when the promise is aborted before main responds (`cancellation.renderer.ts:49-53`). P0.
- The same `settings.timeout` is handed to the script sandbox (`tryToExecuteScript` context `timeout`, **NET** around :900 region of `tryToExecuteScript`), so a slow script and a slow request share one number. P1.

## Cross-platform notes

- libcurl is `@getinsomnia/node-libcurl` 3.3.0 (`packages/insomnia/package.json:63`), a fork. It is not context-aware and must not be imported by the renderer (comment **LP**:1-2); every send therefore crosses IPC (`curlRequest`, `main.ts:662`), and the renderer path wraps it in `cancellableCurlRequest` (`cancellation.renderer.ts:38`). Any Heric port must keep the native module in a separate process. P0.
- Packaging: `npmRebuild: false` and `asarUnpack` for `node_modules/@getinsomnia/node-libcurl` is declared only inside the `mac` block (`packages/insomnia/electron-builder.config.js:10`, :73-74); `win` (:116) and `linux` (:158) blocks do not repeat it, so their native binaries rely on default handling. Mac builds are universal and notarized (:46-55, :72), with `mergeASARs: false` (:56). P1.
- Trust store: `CurlSslOpt.NativeCa` (**LP**:378) delegates to the OS store (macOS keychain, Windows cert store, Linux OpenSSL paths); the workspace CA is an *addition* via `CAINFO_BLOB` (:375), never a replacement. P0.
- File access policy is cross-platform but path-sensitive: allowlist roots are `os.tmpdir()`, `<userData>/responses`, `<userData>/version-control`, plus user `dataFolders` (`packages/insomnia/src/main/secure-read-file.ts:36-46`); checks resolve symlinks and defend against hardlink aliases of NeDB files (:53-95). Multipart temp bodies live in `os.tmpdir()` (`main/network/multipart.ts:18`) and are deleted after send (**LP**:542-551) — on Windows the async `unlink` may fail while curl still holds the handle, and the error is ignored (:550). P1.
- Unix sockets: `UNIX_SOCKET_PATH` is set whenever the URL is written as `http(s)://unix:/<socket>:/<path>` (**NET**:1053-1055, **LP**:363) with no platform guard; support on Windows depends on the bundled libcurl build. P2.
- `INSOMNIA_DATA_PATH` overrides userData for response/timeline storage in all processes (**LP**:107, **CURL**:151; node timeline adapter `runtimes/network/network-adapter.node.ts:36-43`, which also guards path traversal on `responseId`). P1.
- Inso/CLI uses the node runtime directly (`network-adapter.node.ts:55`) with no Electron; `getTimelinePath` there calls `require('electron')` only for the userData path (:34-35), so CLI runs must set `INSOMNIA_DATA_PATH`. P1.
- OAuth interactive flows depend on Electron `BrowserWindow`/`shell.openExternal` (`main/network/o-auth-2/get-token.ts:86-96`, `main/authorize-user-in-window.ts:19`, `main/authorize-user-in-default-browser.ts:13`); there is no CLI equivalent in the HTTP path. P1.

## Feature list

P0 = required for a faithful HTTP client; P1 = expected behavior/edge handling; P2 = niche.

| # | Feature | Evidence | Pri |
|---|---|---|---|
| 1 | Send request via node-libcurl from the main process, body streamed to disk | **LP**:104-278, :196-200 | P0 |
| 2 | Method handling: HEAD→NOBODY, POST→POST, else CUSTOMREQUEST | **LP**:140-148 | P0 |
| 3 | Body types: raw text, urlencoded rebuild, file upload, multipart temp file | **LP**:150-174, :603-632 | P0 |
| 4 | Multipart builder with fixed boundary and mime lookup | `main/network/multipart.ts:16-118`, `network/multipart-constants.ts:1` | P0 |
| 5 | Multipart part paths checked against `dataFolders` before reading | **LP**:620-632, tests `libcurl-promise.test.ts:89-113` | P0 |
| 6 | Header assembly with overrides, defaults, suppression sentinels | `network/parse-header-strings.ts:33-101` | P0 |
| 7 | Folder/request header inheritance and single-value header rules | **NET**:77-111, `common/common-headers.ts:8-19` | P0 |
| 8 | User-Agent default `insomnia/<version>` and suppression | **LP**:516-523; `shouldSuppressUserAgent` **NET**:114-128 | P1 |
| 9 | `Accept: */*` default; no implicit Accept-Encoding/content-type | `parse-header-strings.ts:78-90` | P1 |
| 10 | HTTP version selection incl. HTTP/3 and prior-knowledge H2 | **LP**:408-413, :637-661 | P1 |
| 11 | Redirect following with per-request override + max-redirect cap | **LP**:415-419, :472-479 | P0 |
| 12 | Optional path-dot-segment preservation | **LP**:481-484 | P2 |
| 13 | Whole-request timeout; disable with 0 | **LP**:452-457 | P0 |
| 14 | Cancellation mid-flight with Abort status | **LP**:187-192, :265-268; `cancellation.renderer.ts:38-58` | P0 |
| 15 | Response headers per redirect hop, last hop promoted | **LP**:559-580; **NET**:958 | P0 |
| 16 | Raw header parsing preserving duplicate `Set-Cookie` | **LP**:231, :559 | P0 |
| 17 | Response body to file + byte counters + total time | **LP**:196-200, :234-240 | P0 |
| 18 | Decompression with single retry without Accept-Encoding | **LP**:372, :250-276 | P1 |
| 19 | Cookie sending from jar (Netscape format) and manual cookie list | **LP**:486-513 | P0 |
| 20 | Cookie storing via tough-cookie, origin tagging, rejection reporting | `network/set-cookie-util.ts:8-44`; **NET**:945-956 | P0 |
| 21 | Cookie jar persistence per workspace | **NET**:955; `main/ipc/cookies.ts:66-84` | P0 |
| 22 | Server-set cookies never re-rendered as templates | `common/render.ts:887-908` | P1 |
| 23 | Client certificates by host/port with wildcards, disabled flag | `insomnia-data/src/models/client-certificate.ts:44-56` | P0 |
| 24 | PEM, P12, key, passphrase, allowlist enforcement | **LP**:380-406 | P0 |
| 25 | Workspace CA certificate (PEM blob) + native CA store | **LP**:375-378 | P0 |
| 26 | Disable TLS verification (host + peer) | **LP**:461-465 | P0 |
| 27 | Separate `validateAuthSSL` for OAuth token requests | `o-auth-2/get-token.ts:667` | P1 |
| 28 | Manual proxy per scheme with noProxy (host + CIDR) | **LP**:438-450, :664-681 | P0 |
| 29 | System proxy via Electron PAC resolution when proxy disabled | **LP**:421-436, :296-341 | P0 |
| 30 | Proxy credentials from URL userinfo, any auth scheme | **LP**:445 | P1 |
| 31 | App-level Chromium proxy config with scope and bypass list | `main/proxy.ts:72-137`; `entry.main.ts:150` | P1 |
| 32 | Scripts/plugins cannot use proxy or inject credentials/certs | `templating-worker-database.ts:817-843`; `plugins/context/network.ts:67-73` | P0 |
| 33 | Basic auth (UTF-8 / ISO-8859-1) | `get-auth-header.ts:66-70`; `network/basic-auth/get-header.ts:5-15` | P0 |
| 34 | Bearer auth with custom prefix / NO_PREFIX | `get-auth-header.ts:17-26, 72-75` | P0 |
| 35 | API key in header, cookie, or query | `get-auth-header.ts:38-64`; **NET**:1016-1028 | P0 |
| 36 | Digest / NTLM via libcurl | **LP**:525-534 | P0 |
| 37 | netrc | **LP**:535-537 | P2 |
| 38 | AWS IAM signing (aws4), rejects file bodies | `parse-header-strings.ts:49-66, 118-149`; **LP**:159-161 | P1 |
| 39 | OAuth 2 grants: implicit, auth code (+PKCE S256/plain), password, client credentials, refresh | `o-auth-2/get-token.ts:200-543` | P0 |
| 40 | OAuth 2 token cache with folder inheritance, expiry, 401/invalid_grant invalidation | `o-auth-2/get-token.ts:~536-543`, :495-514 | P0 |
| 41 | OAuth 2 token-request timeline surfaced in main timeline | `o-auth-2/get-token.ts:379-386, 484-491` | P1 |
| 42 | OAuth 1.0a (HMAC-SHA1/256, RSA-SHA1, PLAINTEXT, body hash) | `o-auth-1/get-token.ts:15-116` | P1 |
| 43 | Hawk auth with optional payload validation | `get-auth-header.ts:112-140` | P2 |
| 44 | ASAP (JWT) auth | `get-auth-header.ts:142-170` | P2 |
| 45 | Interactive OAuth: embedded window or default browser with encrypted relay | `o-auth-2/get-token.ts:101-154, 290-330` | P1 |
| 46 | Pre-request rendering, plugin hooks, plugin default headers | `send.tsx:232-249`; `network-adapter.renderer.ts:55-69`; `apply-default-headers.ts:6` | P0 |
| 47 | Pre/after-response scripts with skipRequest and error responses | `send.tsx:175-222, 293-322`; **NET**:274, :743 | P0 |
| 48 | Confidential-value redaction of timeline, response URL, test results | `sensitive-value-collector.ts:63-112`; **NET**:907-910; `curl.ts:80-93` | P0 |
| 49 | Response history with `maxHistoryResponses` pruning | **LP**:244-245 in streaming; `send.tsx:361`; settings.ts:62 | P1 |
| 50 | Save response body to a chosen file / download path | `send.tsx:82-109, 366-390` | P1 |
| 51 | Postman-style Console timeline with header/data prefixes and size caps | `response-timeline-viewer.tsx:13-36`; **LP**:202-228 | P0 |
| 52 | Step timer (pre-request, render, send, after-response) | `request-timing.ts`; `send.tsx:175/232/250/293`; `use-execution-state.ts:5-49` | P1 |
| 53 | Event-stream/streaming HTTP connection with NDJSON event log | **CURL**:137-377, :450-484 | P1 |
| 54 | Only one streaming connection per request id | **CURL**:143-148 | P1 |
| 55 | Close all streaming connections on window close | **CURL**:509 | P2 |
| 56 | Unix domain socket requests via `http(s)://unix:/...` | **NET**:1053-1055; **LP**:363 | P2 |
| 57 | GraphQL introspection reuses the send pipeline (shared OAuth token) | `graph-ql-editor.tsx:163`; `get-auth-header.ts:83` | P1 |
| 58 | HAR export reuses auth header generation (masked) and URL-filtered cookies | `main/har.ts:310, 337` | P1 |
| 59 | Inso/collection-runner send path with the same pipeline | `network/send-request.node.ts:95`; `network-adapter.node.ts:55` | P0 |
| 60 | Test-only `X-Mockbin-Test` header injection | **LP**:176-179 | P2 |
