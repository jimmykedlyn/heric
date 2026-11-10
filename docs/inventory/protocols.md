# Protocols

Non-HTTP protocol support in the Insomnia (pre-rewrite) codebase. All paths are relative to the repository root; `insomnia` = `packages/insomnia/src`, `data` = `packages/insomnia-data/src`.

Architecture in one paragraph: every non-HTTP protocol is an Electron **main-process engine** (`main/network/*` for WebSocket/Socket.IO/curl-streaming, `main/ipc/grpc.ts` for gRPC) that owns a `Map` of live connections keyed by request id, writes responses as `*.response` + NDJSON `*.timeline` / event-log files under the response directory, and pushes state to the renderer over `webContents.send` channels. The renderer subscribes via hooks (`ui/hooks/use-ready-state.ts`, `use-realtime-connection-events.ts`, `use-close-connection.ts`) and holds per-request protocol state in `routes/...debug.tsx`. The preload bridge exposes `window.main.{grpc,webSocket,socketIO,curl}` (`entry.preload.ts:44-79,127-136`), and each channel must be allowlisted in `main/ipc/electron.ts` (invoke: `:39-41,103-106,179-189`; one-way: `:225-234,254-257`).

## GraphQL

### How it works

GraphQL is **not a transport** in this codebase: it is an HTTP `Request` whose body is GraphQL text and whose `mimeType` is `application/graphql` (`data/common-src/constants.ts:2`). Sending a GraphQL request goes through the ordinary HTTP/curl pipeline after one post-render fixup, `parseGraphQLReqeustBody(renderedRequest)`, which JSON-encodes the `{ query, variables, operationName }` envelope (`routes/organization.$organizationId.project.$projectId.workspace.$workspaceId.debug.request.$requestId.send.tsx:247-248`). Serialization lives in `getGraphQLContent` (`ui/components/editors/body/graph-ql-editor.tsx:71`): empty query and empty variables produce an empty body string.

**Schema introspection** is performed by *sending a second HTTP request*: `fetchGraphQLSchemaForRequest` (`graph-ql-editor.tsx:110`) builds `{ query: getIntrospectionQuery({ inputValueDeprecation }), operationName: 'IntrospectionQuery' }` (`:131-134`), constructs a synthetic private `Request` with `_id: request._id + '.graphql'`, `parentId: request._id`, `isPrivate: true` ("So it doesn't get synced or exported"), JSON mime type, and `settingMaxTimelineDataSize: 5000` (`:136-147`), then runs `fetchRequestData` → interpolation → plugins → `sendCurlAndWriteTimeline` (`:163`) → `responseTransform` (`:171`). The response body is `JSON.parse`d; GraphQL `errors[0]` becomes `schemaFetchError`, otherwise `buildClientSchema(data)` produces the schema (`:194`).

Because the synthetic id ends in `.graphql`, OAuth2 token lookup strips the suffix so schema fetches share the original request's token (`main/network/get-auth-header.ts:78-95`, GitHub issue #835 comment in code). Auto-fetch runs in an effect keyed on `[automaticFetch, environmentId, includeInputValueDeprecation, request._id, request.url, workspaceId]` (`graph-ql-editor.tsx:282-304`) and the toggle defaults to true via `localStorage['graphql.automaticFetch']` (`:268`). A local schema can instead be loaded from a JSON introspection file (`loadAndSetLocalSchema`, `:409`, requires `content.data` and `insecureReadFile`).

**Subscriptions** are WebSocket frames, not HTTP. The connect action rewrites the URL (`http`→`ws`, `https`→`wss`), injects `sec-websocket-protocol: graphql-transport-ws`, marks `isGraphqlSubscriptionRequest: true`, and sends `{ type: ConnectionInit }` as initial payload (`routes/organization.$organizationId...connect.tsx:62-89`). On `connection_ack` the main process parses the stored GraphQL body and sends `{ id: uuidV4(), type: Subscribe, payload }`; incoming `error`/`complete` closes the socket (`main/network/websocket.ts:515-539`). Operation type is detected by parsing the body and running `getOperationAST` (`data/models/request.ts:402-422`); subscriptions are routed to the realtime pane (`routes/...debug.tsx:425-430`).

### Wire format

- HTTP: `POST` with `Content-Type: application/graphql`; body is either raw GraphQL text or the JSON `{query, variables, operationName}` envelope produced by `getGraphQLContent`.
- Introspection response: standard `{ data: { __schema } }` consumed by `buildClientSchema`.
- Subscriptions: `graphql-transport-ws` JSON messages over WebSocket (`connection_init`, `connection_ack`, `subscribe`, `next`, `error`, `complete`).

### UI affordances

- Request creation preset (`routes/...request.new.tsx:57-72`), `GQL` badge on the sidebar row (`ui/components/tags/method-tag.tsx:38`).
- `GraphQLEditor` replaces the body editor: an Operations dropdown (multi-operation documents, selection preserved by index across edits, `changeQuery` `:343`; operation name `:319`), a Schema dropdown (Show Documentation, Remote: Refresh / Automatic Fetch / Include input value deprecation; Local: Load schema from JSON), a query CodeMirror with GraphQL hint/info/jump/lint when a schema exists (`:497-514`), a Variables editor with Nunjucks rendering and JSON prettify (`:328`), a "Schema fetched <TimeFromNow>" status (`renderSchemaFetchMessage` `:392`), and a clickable operation cursor/selection that dims other operations via a `TextMarker` (`highlightOperation` `:518-552`, `getCurrentOperation` `:554-591`).
- Documentation explorer is portaled into `#graphql-explorer-container` (`graph-ql-editor.tsx:462-473`; container defined in `ui/components/root.tsx:255`) and implemented in `ui/components/graph-ql-explorer/graph-ql-explorer.tsx:74` with navigation history and filtering.
- Subscriptions are sent from the URL bar, which shows **Connect** instead of **Send** for `isGraphqlSubscriptionRequest` (`ui/components/request-url-bar.tsx:125-126,198,301`), and render through the shared realtime pane.

### Error handling

Distinct states: parse errors in the query editor are reported and the AST/operations list is preserved without clobbering the body (`changeQuery` `:343`); variables syntax errors are tracked separately (`changeVariables` `:328`); schema fetch non-2xx or GraphQL errors surface as `schemaFetchError` with status and URL (`:194`); no url/request yields the "no request" schema message (`:110`); subscription server `error`/`complete` closes the socket and the close appears in the event log (`websocket.ts:529-538`).

### Source locations

`ui/components/editors/body/graph-ql-editor.tsx` (806 lines), `ui/components/graph-ql-explorer/graph-ql-explorer.tsx`, `ui/components/tags/method-tag.tsx:38`, `data/models/request.ts:399-422`, `main/network/get-auth-header.ts:78-95`, `main/network/websocket.ts:515-539`, connect/send routes cited above, smoke tests `packages/insomnia-smoke-test/tests/smoke/graphql.test.ts`.

## gRPC

### How it works

Engine: `main/ipc/grpc.ts` (599 lines) using `@grpc/grpc-js` (`makeGenericClientConstructor`) and `@grpc/proto-loader`.

**Proto sources.** (1) Local `.proto` files and directories imported through the Proto Files modal (`ui/components/modals/proto-files-modal.tsx`): each file is validated with `window.main.grpc.validateProtoFile` (`:50`), read with `insecureReadFile`, and persisted as `ProtoFile` (`:100-111`); directories are walked recursively creating `ProtoDirectory` + `ProtoFile` rows (`:114-141`). (2) Server reflection via the Kong fork `grpc-reflection-js`: `Client.listServices()`, then per service `fileContainingSymbol` → `lookupService` → method list with `mockRequestMethods` examples (`grpc.ts:233-285`). (3) Buf Schema Registry when `reflectionApi.enabled`: `@connectrpc/connect-node` `createConnectTransport({ baseUrl, httpVersion: '1.1' })` calling `FileDescriptorSetService`, with `Authorization: Bearer <apiKey>` and a `User-Agent: insomnia/<version>` header unless disabled; `ConnectError` codes are translated (`Code.Unauthenticated` → "Invalid reflection server api key"; `Code.NotFound` → module access message) (`:131-231`). Reflection falls back to Buf when enabled (`:243-245`).

**Proto materialization.** Files are written to `os.tmpdir()/insomnia-grpc/`: a directory tree at `<rootDirId>.<modified>/`, a single file at `<id>.<modified>.proto`; rewriting is skipped if the target exists; `sanitizeName` reduces names to a basename and replaces `.`/`..`; `assertWithinTempRoot` throws "Proto path escapes temporary directory" if a resolved path leaves the root (`main/network/grpc/write-proto-file.node.ts:18-117`).

**Method execution.** `getSelectedMethod` resolves the method from the proto file by id or from reflection (`grpc.ts:334`); `getMethodType` classifies request/response streaming into unary/client/server/bidi (`:321`). `start` validates the URL ("URL not specified"), finds the method ("The gRPC method X could not be found"), parses `unix:`/`grpc:`/`grpcs:` (`main/network/grpc/parse-grpc-url.ts`), builds credentials (`getChannelCredentials` `:380-414`: `grpcs:` → `createSsl` combinations of ca/cert/key with `rejectUnauthorized`; otherwise `createInsecure()`), and constructs the generic client (`:415-451`). Dispatch is `makeUnaryRequest` (`:453`), `makeClientStreamRequest` (`:464`), `makeServerStreamRequest` (`:474`), `makeBidiStreamRequest` (`:484`); unknown types error (`:493`). Client and bidi streams write JSON messages through `sendMessage` (`JSON.parse(body.text)`, flushed on `process.nextTick`, `:510-528`), `commit` calls `.end()` (`:530`), `cancel` calls `.cancel()` (`:531`).

**Responses** are pushed per call: `data` (stringified value), `status` (`StatusObject`), `error`, `end` (`onStreamingResponse` `:533-561`, `onUnaryResponse` `:563`). Metadata sent = enabled headers only (`filterDisabledOrInvalidMetaData` `:581`). `closeAll` cancels every live call and is also wired to `window-all-closed` (`:592-599`).

### Wire format

HTTP/2 gRPC frames with protobuf; JSON is the editor format and messages are `JSON.parse`d before writing. Requests carry metadata from headers; TLS on `grpcs:`. Streaming responses arrive as individual JSON messages (each rendered as its own `Response n` tab), terminal state is the gRPC status code.

### UI affordances

`ui/components/panes/grpc-request-pane.tsx` (543 lines): "gRPC" badge, one-line URL editor (placeholder `grpcb.in:9000`), method dropdown, "use request stubs" button (fills bodies from `automock` examples), reflection refresh, proto-file button, and a send button that reads **Send/Cancel** for unary and **Start** for streams (`ui/components/buttons/grpc-send-button.tsx`). Methods load on mount from `grpc.loadMethods(protoFileId)` (`:85-88`) or by reflection only when `reflectionApi.enabled` is false and a URL is set — the Buf API key is deliberately **not** sent on pane open (`:89-101`). Tabs: Messages (`Body`, `Stream n`), Method type, Headers (metadata key/value editor); client/bidi streams get **Stream +** (`:439`, sends rendered body via `grpc.sendMessage` `:424`, shows a redacted copy) and **Commit** (`:443-445`). `handleRequestSend` (`:147`) inherits metadata via `getOrInheritHeaders`, renderes with `skipBody: canClientStream`, finds client certificates by host and only passes cert/key/CA when the URL starts with `grpcs:` (`:181-199`). Response pane: per-message tabs plus `GrpcStatusTag` (`ui/components/panes/grpc-response-pane.tsx`, `ui/components/tags/grpc-status-tag.tsx` — code 0 "OK" green, else red with message). Live state (`running`, `requestMessages`, `responseMessages`, `status`, `error`, `methods`) lives per request in `routes/...debug.tsx:63` and is fed by `grpc.start/end/data/error/status` listeners (`:249-298`).

### Error handling

Status codes are first-class: any non-zero code renders red with `status.details`; CANCELLED errors are suppressed in the UI (`grpc.ts:533-561`); UNKNOWN/UNAVAILABLE also terminate. Connection-level messages are mapped to friendly text by `GRPC_CONNECTION_ERROR_STRINGS` / `isGrpcConnectionError` / `getGrpcConnectionErrorDetails` (`ui/utils/grpc.ts:1-20`: self-signed certificate, CERTIFICATE_REQUIRED, CANCELLED, WRONG_VERSION_NUMBER, local issuer, UNIMPLEMENTED, reflection failure) and open a connection-error modal (`routes/...debug.tsx:286`). Invalid proto files raise an "Invalid Proto File" modal (`grpc-request-pane.tsx:517-535`). Reflection failure without the Buf backend has a specific message (`grpc.ts:233-285`, test `main/ipc/__tests__/grpc-reflection-buf.test.ts`).

### Source locations

`main/ipc/grpc.ts`, `main/network/grpc/write-proto-file.node.ts`, `main/network/grpc/parse-grpc-url.ts`, `main/ipc/automock.ts`, `ui/components/panes/grpc-request-pane.tsx`, `ui/components/panes/grpc-response-pane.tsx`, `ui/components/tags/grpc-status-tag.tsx`, `ui/components/buttons/grpc-send-button.tsx`, `ui/utils/grpc.ts`, `ui/components/modals/proto-files-modal.tsx`, `data/models/grpc-request.ts:12-45`, `data/models/proto-file.ts`, `data/models/proto-directory.ts`, tests `main/ipc/__tests__/grpc.test.ts`, smoke tests `grpc.test.ts`, `grpc-reflection-api.test.ts`, `grpc-mtls.test.ts`.

## WebSocket

### How it works

Engine: `main/network/websocket.ts` (695 lines) on the `ws` library. `openWebSocketConnection` (`:174`) deduplicates per request id (`:178-183`), opens response and timeline write streams under `responses/` (`:193-197`), resets the per-request sensitive-value registry, applies auth (basic → Authorization; apikey → header, cookie, or query param; bearer → Authorization; only these three types are offered, `ui/components/websockets/websocket-request-pane.tsx:48`), sets a default User-Agent unless disabled, resolves client certificates (PEM/PFX, with timeline entries, `:285-320`), builds the cookie jar and appends the auth cookie (`:315-322`), maps `followRedirects` global/on/off (`:323`), takes subprotocols from the `sec-websocket-protocol` header (`:329`), and adds a proxy agent only for WebSocket requests when the proxy setting is on (`:330`). The socket is created with `{ headers, ca, cert, key, pfx, rejectUnauthorized, followRedirects, maxRedirects, agent? }` (`:332-346`).

Lifecycle events:
- `upgrade` (`:348`): creates a `WebSocketResponse` with status 101, captures handshake headers and `Set-Cookie` into the cookie jar (`:365-393`).
- `unexpected-response` (`:399`): dumps the HTTP body as a `DataOut` timeline entry, creates an error response, and removes the connection so the failure is inspectable.
- `open` (`:432`): writes an open event, timeline entries (Preparing request, time, HTTP 1.1, HeaderOut/HeaderIn — `parseResponseAndBuildTimeline` `:142`), broadcasts readyState true, and sends `initialPayload` if present (`:447`).
- `message` (`:452`): writes a message event; GraphQL subscription frames are diverted to `handleGraphQLWsMessage` (`:458`).
- `close` (`:468`): logs code/reason/wasClean, cleans up maps.
- `error` (`:484`): redacts, cleans up, and synthesizes an error response when `error.code` is present.
Unknown/exception path (`:495-507`) produces an error response too.

Outgoing payloads: `sendPayload` (`:584`) writes to the socket and logs an OUTGOING event; `sendWebSocketEvent` (`:618`) is the IPC entry; `closeWebSocketConnection` (`:634`), `closeAll` (`:642`). ReadyState is `ws.readyState === OPEN` (`:580`), broadcast on `${protocol}.${requestId}.readyState` (`:221`). History loading (`findMany` `:644`) reads responses in reverse order with `secureReadFile`.

### Wire format

RFC 6455 WebSocket. Handshake headers come from the request's headers; URL query parameters are appended by the UI (`ui/components/websockets/action-bar.tsx`, `generateConnectParams`). Payloads are strings stored on a separate `WebSocketPayload` entity (`data/models/websocket-payload.ts`: `{name, value, mode}`) and rendered with Nunjucks before send. Response artifacts: a `WebSocketResponse` row (`eventLogPath`, `timelinePath`, `statusCode`, `url`, `headers`) and NDJSON files (one JSON object per event: open/message/close/error/OUTGOING).

### UI affordances

`ui/components/websockets/websocket-request-pane.tsx`: form loads the `webSocketPayload` value (`:74`); `interpolateOpenAndSend` (`:88`) renders URL/headers/auth/cookie jar and, when disconnected, opens with `initialPayload`, otherwise calls `webSocket.event.send` (`:145`). Tabs: Params, Body (payload), Auth (only apikey/basic/bearer, `:478`), Headers, Docs; controls are disabled while the socket is open (`disabled = readyState`, `:196`). Connect/Disconnect live in the URL bar for realtime requests and in the action bar (`action-bar.tsx:202-208`), with Cmd/Ctrl+Enter to send. `disconnect-button.tsx` offers close-this / close-all per protocol. `event-log-view.tsx` renders a virtualized, filterable table with icons per event type and direction.

### Error handling

Handshake rejection is surfaced as an HTTP-like error response from `unexpected-response` (status/headers/body preserved in timeline and body), so 401/403 are visible rather than a bare socket error. Runtime errors become error responses with the redacted message. Close frames display code, reason, and `wasClean`. Sensitive values collected by the renderer (when the "redact sensitive data" setting is on) are scrubbed from timeline and event text before writing (`:90`, `:484`).

### Source locations

`main/network/websocket.ts`, `ui/components/websockets/websocket-request-pane.tsx`, `ui/components/websockets/action-bar.tsx`, `ui/components/websockets/disconnect-button.tsx`, `ui/components/websockets/event-log-view.tsx`, `ui/components/websockets/realtime-response-pane.tsx`, `data/models/websocket-request.ts:17-36`, `data/models/websocket-payload.ts`, `data/models/websocket-response.ts`, smoke test `websocket.test.ts` (connect, basic/bearer, 101 + Console timeline, close code 1005).

## Server-Sent Events

### How it works

SSE has **no request type**. An "Event Stream" is an ordinary HTTP `Request` whose `Accept` header value is exactly `text/event-stream`: `isEventStreamRequest` returns true when the first header named `Accept` has value `text/event-stream` (`data/models/request.ts:399-400`). The request preset is `GET` + that header (`routes/...request.new.tsx:76-83`); the constant is `CONTENT_TYPE_EVENT_STREAM` (`common/constants.ts:231`).

Sending uses the curl engine in streaming mode (`main/network/curl.ts`): `openCurlConnection` (`:137`) deduplicates, creates `<uuid>.response` and `<uuid>.timeline` files (`:151-155`), clears request-level cookies (`:200`), builds a configured curl instance with `CUSTOMREQUEST` and always-set `POSTFIELDS` (even empty, `:208`), enables `CurlFeature.StreamResponse` (`:211`), and attaches auth via the same `getAuthHeader` used for HTTP (Basic header registered as sensitive, OAuth2/bearer, `:184-195`). The DEBUGFUNCTION captures headers; on the first stream callback a `Response` row is created with status, headers and cookies (`:280-330`); each subsequent chunk is emitted as a `message` event and appended to the event log (`:331-341`); the terminal read produces a `close` event (`:344`). Errors redact, clean up and flip readyState (`:247`). `createErrorResponse` (`:379`), `deleteRequestMaps` (`:401`), `getCurlReadyState` (`:415`), `closeCurlConnection` (`:419`), `closeAll` (`:442`). `findMany` (`:462`) content-sniffs stored lines so a plain JSON HTTP body is not mistaken for an event log (`isCurlEvent` `:450`).

### Wire format

`text/event-stream` chunked HTTP. Each SSE `data:` block is normalized (CRLF→LF, blocks split on blank lines, multiple `data:` lines collected) and stored as an NDJSON event `{type: 'message', data, direction}` in the response's `bodyPath`. The timeline (`timelinePath`) records HTTP 1.1 request/response metadata.

### UI affordances

The URL bar shows **Connect** for event-stream requests and **Disconnect** while open; the button is cancellable while the ready state is live (`ui/components/request-url-bar.tsx:198,301-306,357`), and "Repeat on Interval" reconnects at an interval (`:430-450`). The sidebar row shows an `EventStreamSpinner` that becomes a success dot when the curl ready state is live (`ui/components/sidebar/.../request-node.tsx:96,192`). The shared realtime pane (`ui/components/websockets/realtime-response-pane.tsx`) gives Events, Headers, Cookies, Timeline tabs, an event-type filter/search/clear, and auto-selection of the latest event (`:353-394`). An extra **Summary** tab, shown only for curl (`:241`), extracts one JSONPath value from the stream (`useStreamSummary` `:242-249`), infers a default path for OpenAI/Anthropic/Gemini payloads (`common/stream-summary.ts:183-207`: `/v1/chat/completions` → `$.choices[0].delta.content`, `/v1/completions` → `$.choices[0].text`, `/v1/responses` → `$.delta`, `/v1/messages` → `$.delta.text`, Gemini `:streamgeneratecontent` → `$.candidates[0].content.parts[0].text`), and can render the result as Markdown. When an inferred path exists the Summary tab is selected by default (`:378`). The chosen path and Markdown preference are persisted on the request (`ui/hooks/use-stream-summary.ts`).

### Error handling

HTTP errors are normal responses created by the stream callback (status/headers visible); curl-level failures create an error response and set readyState false (`curl.ts:247,379`). Closing the tab or switching environment calls `curl.close` / `closeAll` (`ui/hooks/use-close-connection.ts`). A stored plain-HTTP JSON response is excluded from the Events view by the content sniff (`curl.ts:450-497`), covered by `packages/insomnia-smoke-test/tests/smoke/event-stream-response-pane.test.ts` (flipping Accept must not re-read the body as events).

### Source locations

`main/network/curl.ts`, `common/stream-summary.ts`, `ui/hooks/use-stream-summary.ts`, `ui/components/websockets/realtime-response-pane.tsx`, `ui/components/request-url-bar.tsx`, `data/models/request.ts:399-400`, `common/constants.ts:231`, tests `main/network/curl.test.ts`, `common/stream-summary.test.ts` (32 cases).

## Socket.IO

### How it works

Engine: `main/network/socket-io.ts` (646 lines) using `socket.io-client`. `openSocketIOConnection` (`:260`) deduplicates, opens response/timeline streams (`:277-281`), sets a default UA, builds the cookie jar, and passes options (`:329-354`): `extraHeaders`, `query`, `ca`, `passphrase`, `agent` (only when proxy is enabled), `pfx` or `cert`/`key`, `auth = { token }` for the only supported auth type `singleToken`, and `path` (the request's `settingPath`). Certificates come from the workspace CA plus client certificates, filtered to `wss:` URLs (`getCertificates` `:180`); timeline entry "Connecting to" with handshake path `/socket.io` (`buildTimeline` `:156`). On `connect` it logs an open event (plus an info event "Add event listeners to receive messages" when none exist) and creates a `SocketIOResponse` (`:362-397`); `engine.once('upgrade')` records the transport upgrade (`:399`); `disconnect` logs close with reason (`:407`); `connect_error` logs a close + error event and creates an error response (`:420`). Listeners are attached for open events (`:438`). ReadyState is `socket.connected` (`:475`).

**Sending**: `sendPayload` (`:479`) emits with rendered args; with `ack: true` it appends a callback that logs the server ack as an incoming event; the OUTGOING emit is logged. `sendWebSocketEvent` (`:508`), close/closeAll (`:532`, `:540`). **Named listeners**: `addSocketIOListener` (`:542`) logs an addEvent and registers `socket.on(name, …)` producing INCOMING message events; `removeSocketIOListener` (`:575`) logs removeEvent and `socket.off`s. Listener definitions persist on the request as `eventListeners: [{ id, eventName, desc, isOpen }]` (`data/models/socket-io-request.ts:17,32-43`). History (`findMany` `:594`) reads in reverse order.

### Wire format

Socket.IO protocol over WebSocket/polling (client-managed). Payload entity: `args: [{ id, value, mode }]`, optional `eventName` (default `'message'`), optional `ack` (`data/models/socket-io-payload.ts`). Each arg is rendered with Nunjucks and JSON-parsed when `mode` is JSON; ack replies are correlated by the client callback.

### UI affordances

`ui/components/socket-io/request-pane.tsx`: tabs Params (`:135`, query/path parameters), Body (`:146`), Events (`:155`), Auth (`:166`, `authTypes={['singleToken']}` `:309`), Headers (`:177`), Docs (`:188`); a read-only banner appears once connected. Body tab (`body-tab-pane.tsx`): add/remove args, per-arg JSON/plaintext mode, event name (default `message`), Ack toggle (`:183`); `handleSend` renders args, parses JSON-mode ones, then calls `socketIO.event.send({ requestId, eventName, ack, args, sensitiveValues })` (`:98-126`). Events tab (`event-tab-pane.tsx`): listener list with name/desc/open state; toggling on calls `socketIO.event.on` (`:130`), renaming while open forces an off (`:109-118`), removing an open listener calls `socketIO.event.off` (`:92`). `event-view.tsx` formats message args via JSON re-parse using the shared `SocketIOBodyContent`. Responses render in the realtime pane, which hides Headers/Cookies tabs for Socket.IO.

### Error handling

`connect_error` produces both a close event and an error event plus an error response with the message (`socket-io.ts:420-436`); a thrown setup error goes through the catch path (`:443-455`). Disconnect reasons are shown in the event log. Sensitive values are redacted from event logs and timelines via the same registry as WebSocket.

### Source locations

`main/network/socket-io.ts`, `ui/components/socket-io/request-pane.tsx`, `body-tab-pane.tsx`, `event-tab-pane.tsx`, `event-view.tsx`, `data/models/socket-io-request.ts`, `data/models/socket-io-payload.ts`, `data/models/socket-io-response.ts`, smoke test `socket-io.test.ts` (dev server on port 4020).

## Shared realtime infrastructure

- **Channels**: `REALTIME_EVENTS_CHANNELS = { readyState, newEventReceived, mcpNotification }` (`common/constants.ts:479-483`); engines emit `${protocol}.${requestId}.readyState` and `${protocol}.${responseId}.newEventReceived`.
- **Hooks**: `ui/hooks/use-ready-state.ts` (reads current state then subscribes), `use-realtime-connection-events.ts` (fetches `event.findMany` and refetches on newEventReceived), `use-close-connection.ts` (closes gRPC via cancel, WS/Socket.IO via close, curl for SSE, WS for GraphQL subscriptions, on tab close / env change / org change, plus closeAll variants).
- **Redaction**: `main/network/sensitive-values-registry.ts` — per-protocol registries with register/reset/redactText/redactEvent; when the setting is off the renderer sends no values and no redaction occurs.
- **Persistence**: realtime responses store `eventLogPath`/`timelinePath`; `findMany` reads in reverse chronological order. v5 export includes WebSocket, Socket.IO, gRPC, payloads, proto files and directories (`common/import.ts:168-186`; `common/insomnia-v5.ts:772-800,1180-1207`). Proto file **content** is not embedded in collection export nodes — only `protoFileId` — while proto entities are syncable types (`sync/git/ne-db-client.ts:209`).
- **Response routing**: `routes/...debug.tsx` mounts `GrpcRequestPane`/`WebSocketRequestPane`/`SocketIORequestPane`/`RequestPane` and `GrpcResponsePane`/`RealtimeResponsePane`/`ResponsePane` based on request and response type (`:505-519,571-575`).

## Feature list

P0 = required for a faithful client; P1 = expected behavior/edge handling; P2 = niche.

### GraphQL

| # | Feature | Evidence | Edge cases | Pri |
|---|---------|----------|------------|-----|
| 1 | GraphQL request type (POST + `application/graphql`, GQL badge) | `packages/insomnia/src/routes/organization.$organizationId.project.$projectId.workspace.$workspaceId.debug.request.new.tsx:57-72`; `packages/insomnia/src/ui/components/tags/method-tag.tsx:38` | Body may be raw text or JSON envelope; empty query+variables → empty body | P0 |
| 2 | Send envelope `{query, variables, operationName}` | `packages/insomnia/src/ui/components/editors/body/graph-ql-editor.tsx:71`; `...send.tsx:247-248` | Fixup runs after render/plugins; variables rendered with Nunjucks | P0 |
| 3 | Introspection through HTTP pipeline as private `<id>.graphql` request | `graph-ql-editor.tsx:110-147,163-171` | Private: not synced/exported; max timeline 5 KB; OAuth token shared via suffix strip (`get-auth-header.ts:78-95`) | P0 |
| 4 | Build client schema + editor intelligence (hint/info/jump/lint) | `graph-ql-editor.tsx:194,497-514` | Intelligence disabled when no schema; lint uses schema validation | P1 |
| 5 | Schema explorer with navigation history | `graph-ql-editor.tsx:462-473`; `ui/components/graph-ql-explorer/graph-ql-explorer.tsx:74` | Portal target `#graphql-explorer-container` must exist (`root.tsx:255`) | P1 |
| 6 | Local schema JSON import | `graph-ql-editor.tsx:409` | Requires `content.data`; invalid JSON fails silently into error state | P1 |
| 7 | Automatic schema fetch toggle + deprecation inputs option | `graph-ql-editor.tsx:268,282-304` | Persisted in localStorage, default true; refetch on env/url change | P1 |
| 8 | Multi-operation detection, cursor selection, dimming | `graph-ql-editor.tsx:343,518-591,731` | Selection preserved by index; parse failure keeps previous AST; empty query clears body | P1 |
| 9 | Operation name + variables editors | `graph-ql-editor.tsx:319,328` | Variables JSON syntax errors tracked separately | P1 |
| 10 | GraphQL subscriptions over `graphql-transport-ws` | `routes/...connect.tsx:62-89`; `main/network/websocket.ts:515-539` | Subscribe sent only after `connection_ack`; `error`/`complete` closes socket; URL scheme rewritten | P1 |
| 11 | Schema fetch status/error display | `graph-ql-editor.tsx:392`; schema errors at `:194` | Non-2xx shows status+URL; GraphQL `errors[0]` preferred | P1 |
| 12 | Query beautify | `graph-ql-editor.tsx:305` | No-op on parse failure | P2 |

### gRPC

| # | Feature | Evidence | Edge cases | Pri |
|---|---------|----------|------------|-----|
| 13 | Proto file import (single + recursive directory) | `ui/components/modals/proto-files-modal.tsx:50,100-141,333` | Validation before persist; invalid file → "Invalid Proto File" modal | P0 |
| 14 | Method discovery from proto files | `main/ipc/grpc.ts:104-129`; `grpc-request-pane.tsx:85-88` | Methods reloaded on proto change; method name cleared on new proto | P0 |
| 15 | Server reflection (grpc-reflection-js) | `main/ipc/grpc.ts:233-285` | No key needed; failure yields CANNOT_REFLECT message (`ui/utils/grpc.ts:1-10`) | P0 |
| 16 | Four method types (unary/client/server/bidi) | `main/ipc/grpc.ts:321,453-493`; `grpc-request-pane.tsx:54-60` | Unsupported type errors; labels match stream semantics | P0 |
| 17 | Unary call send/cancel | `main/ipc/grpc.ts:453,563`; `ui/components/buttons/grpc-send-button.tsx` | Cancel emits `status.CANCELLED`, suppressed in UI | P0 |
| 18 | Client/bidi streaming (send message, commit, cancel) | `main/ipc/grpc.ts:464,474,510-531`; `grpc-request-pane.tsx:424-445` | Body skipped on start for stream types; `nextTick` flush workaround | P1 |
| 19 | Metadata from enabled headers only | `main/ipc/grpc.ts:581`; `grpc-request-pane.tsx:147` | Disabled/invalid headers filtered; inherited from parent | P0 |
| 20 | TLS credentials for `grpcs:` (CA/client cert/key, rejectUnauthorized) | `main/ipc/grpc.ts:380-414`; `grpc-request-pane.tsx:181-199` | Certs passed only for `grpcs:`; insecure otherwise | P0 |
| 21 | Per-message response tabs + status tag | `ui/components/panes/grpc-response-pane.tsx`; `ui/components/tags/grpc-status-tag.tsx`; `routes/...debug.tsx:249-298` | Streaming messages accumulate as `Response n`; non-zero codes red | P0 |
| 22 | Connection-error translation | `ui/utils/grpc.ts:1-20`; `routes/...debug.tsx:286` | Self-signed, missing cert, wrong version, local issuer, unimplemented | P1 |
| 23 | Request stubs from automock | `main/ipc/automock.ts:29-220`; `grpc-request-pane.tsx` "use request stubs" | Recursive mocks incl. enums/maps/oneofs; name heuristics for plausible values | P2 |
| 24 | Proto materialization to tmp with escape guard | `main/network/grpc/write-proto-file.node.ts:18-117` | Path traversal throws; existing files not rewritten | P1 |
| 25 | Buf Schema Registry reflection backend | `main/ipc/grpc.ts:131-231,243-245` | Key not sent on pane open; 401/404 mapped to messages; UA toggle | P2 |
| 26 | URL parsing (`unix:`, `grpc:`, `grpcs:`) | `main/network/grpc/parse-grpc-url.ts` | Trailing slash trimmed; missing scheme defaults to `grpc://` | P2 |
| 27 | Close all calls on window close | `main/ipc/grpc.ts:592-599` | Avoids orphaned streams | P1 |

### WebSocket

| # | Feature | Evidence | Edge cases | Pri |
|---|---------|----------|------------|-----|
| 28 | Connect with headers, query params, subprotocols | `main/network/websocket.ts:329-346`; `ui/components/websockets/action-bar.tsx` | `sec-websocket-protocol` header becomes protocols array | P0 |
| 29 | Auth basic/apikey(header,cookie,query)/bearer | `main/network/websocket.ts:231-260`; `websocket-request-pane.tsx:48,478` | Only three types offered; apikey cookie merged into cookie header | P0 |
| 30 | Handshake response 101 with headers/cookies | `websocket.ts:348-397` | Cookies stored from `Set-Cookie` | P0 |
| 31 | Live send of rendered payload | `websocket-request-pane.tsx:88-145`; `websocket.ts:584-616` | Renders at click time; opens first if disconnected | P0 |
| 32 | Initial payload on open | `websocket.ts:447` | Used by GraphQL `connection_init` | P1 |
| 33 | Event log (NDJSON) + live virtualized view with filter | `websocket.ts:96`; `ui/components/websockets/event-log-view.tsx` | OUTGOING/incoming/open/close/error events; sensitive values redacted | P0 |
| 34 | readyState broadcast + Connect/Disconnect UI | `websocket.ts:221,580`; `ui/hooks/use-ready-state.ts`; `request-url-bar.tsx:301-357` | State survives pane unmount | P0 |
| 35 | Client certificates (PEM/PFX) | `websocket.ts:285-320` | Timeline records cert selection; passphrase support | P1 |
| 36 | Cookie jar store/send | `websocket.ts:315-322`; `data/models/websocket-request.ts:17-24` | Auth cookie appended; request may disable send/store | P1 |
| 37 | Unexpected HTTP response surfaced | `websocket.ts:399-430` | Body dumped as DataOut; connection removed | P1 |
| 38 | Close code/reason/wasClean display | `websocket.ts:468-483` | Unclean 1005 shown | P1 |
| 39 | Proxy agent support | `websocket.ts:330,656` | Only for WS requests when proxy setting on | P1 |
| 40 | followRedirects global/on/off | `websocket.ts:323` | Map to `ws` options; maxRedirects | P2 |
| 41 | Timeline entries (request prep, time, HTTP 1.1) | `websocket.ts:142-172` | HeaderOut/HeaderIn included | P1 |
| 42 | Close all connections | `websocket.ts:642-654` | Per-protocol and global | P1 |
| 43 | History read in reverse order | `websocket.ts:644-654` | Reads response files securely | P1 |

### Server-Sent Events

| # | Feature | Evidence | Edge cases | Pri |
|---|---------|----------|------------|-----|
| 44 | Event stream identified by exact `Accept: text/event-stream` | `packages/insomnia-data/src/models/request.ts:399-400` | First Accept header only; case-sensitive exact match; changing header changes type | P0 |
| 45 | Request preset (GET + Accept header) | `routes/...request.new.tsx:76-83`; `common/constants.ts:231` | No dedicated model type | P0 |
| 46 | Streaming via curl `StreamResponse` | `main/network/curl.ts:137-377`; `routes/...connect.tsx:91-98` | `POSTFIELDS` always set; request cookies cleared; dedupe per request id | P0 |
| 47 | Event storage (NDJSON in bodyPath) + Events view | `curl.ts:331-341`; `ui/components/websockets/realtime-response-pane.tsx:386` | Plain HTTP JSON not re-read as events (`curl.ts:450-497`) | P0 |
| 48 | Close/disconnect and readyState | `curl.ts:415-440`; `request-url-bar.tsx:304-306,357` | ReadyState false after close/error | P0 |
| 49 | Sidebar live indicator | `ui/components/sidebar/.../request-node.tsx:96,192` | Success dot vs spinner | P1 |
| 50 | Stream Summary with JSONPath, provider inference, Markdown | `common/stream-summary.ts:16-207`; `ui/hooks/use-stream-summary.ts`; `realtime-response-pane.tsx:62,241-249,378,393` | Empty array → null; joins multi-match arrays; path persisted; curl-only tab | P1 |
| 51 | Auth (Basic/OAuth2/bearer) and headers through HTTP pipeline | `main/network/curl.ts:184-195` | Basic header registered as sensitive | P0 |
| 52 | Repeat on interval reconnect | `request-url-bar.tsx:430-450` | Interval cleared on unmount/close | P2 |
| 53 | Error response for curl failures | `curl.ts:247,379` | Message redacted before display | P1 |

### Socket.IO

| # | Feature | Evidence | Edge cases | Pri |
|---|---------|----------|------------|-----|
| 54 | Connect with headers/query/path/proxy/certs | `main/network/socket-io.ts:329-356` | Certs only for `wss:`; `agent` only when proxy on | P0 |
| 55 | singleToken auth | `socket-io.ts:329-354`; `ui/components/socket-io/request-pane.tsx:309` | Only `{ token }` shape supported | P1 |
| 56 | Emit args (JSON/plaintext) with optional event name | `ui/components/socket-io/body-tab-pane.tsx:98-126`; `socket-io.ts:479-506` | JSON-mode args that fail `JSON.parse` are sent as raw strings, silently | P0 |
| 57 | Ack callbacks | `body-tab-pane.tsx:183`; `socket-io.ts:479-506` | Ack logged as incoming event | P1 |
| 58 | Named listeners persisted and toggled | `socket-io.ts:542-591`; `ui/components/socket-io/event-tab-pane.tsx:92-130`; `data/models/socket-io-request.ts:17,32-43` | Renaming while open forces off; removal while open calls off | P0 |
| 59 | Event log + info hint when no listeners | `socket-io.ts:362-438` | Add/remove listener events logged | P1 |
| 60 | Handshake path setting | `ui/components/socket-io/request-pane.tsx:135`; `socket-io.ts:329-354` | Default `/socket.io` in timeline | P1 |
| 61 | Disconnect reason + connect_error response | `socket-io.ts:407-436` | Error both as event and error response | P1 |
| 62 | Read-only form while connected | `ui/components/socket-io/request-pane.tsx` banner | Prevents config drift mid-connection | P2 |
| 63 | Export/import eventListeners + payload body | `common/insomnia-v5.ts:1180-1198`; `common/import.ts:168-186` | Listener `isOpen` state is exported | P1 |

### Cross-cutting

| # | Feature | Evidence | Edge cases | Pri |
|---|---------|----------|------------|-----|
| 64 | IPC bridge + channel allowlists for all protocols | `entry.preload.ts:44-79,127-136`; `main/ipc/electron.ts:39-41,103-106,179-189,225-234,254-257` | Any new channel must be added to both preload and allowlist | P0 |
| 65 | Sensitive-value redaction in logs/timelines | `main/network/sensitive-values-registry.ts`; `websocket.ts:90,484`; `curl.ts:80,86` | Off setting ⇒ nothing registered ⇒ no redaction | P1 |
| 66 | Realtime response/timeline persistence and history | `data/models/websocket-response.ts`, `socket-io-response.ts`; engine `findMany` | Reverse order; content sniffing for curl | P1 |
| 67 | Request-type routing in debug route | `routes/...debug.tsx:425-430,505-519,571-575` | GraphQL subscriptions classified as realtime | P0 |
| 68 | v5 collection export/import of non-HTTP entities | `common/import.ts:168-186,265-281,570-571,664-665`; `common/insomnia-v5.ts:1063-1207` | Proto content not embedded, only `protoFileId`; proto entities syncable (`sync/git/ne-db-client.ts:209`) | P1 |
| 69 | Smoke coverage for every protocol | `packages/insomnia-smoke-test/tests/smoke/{graphql,grpc,grpc-reflection-api,grpc-mtls,websocket,socket-io,event-stream-response-pane}.test.ts` | Dev servers: HTTP 4010, gRPC 50051, Socket.IO 4020 | P1 |
