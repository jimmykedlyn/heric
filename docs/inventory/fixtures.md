# Fixtures and golden tests

Scope: every fixture directory, expected-output baseline and reference-behavior test in the Insomnia repository that can be copied into Heric as a golden test. Paths are relative to the Insomnia repo root (`D:/Projects/Heric/Insomnia`); the original tree is read-only. Priority: **P0** = blocks a correct Rust reimplementation; **P1** = required to match real user data; **P2** = defensive or optional.

Baseline facts: 12 fixture directories exist (`find . -type d \( -name __fixtures__ -o -name fixtures \)`), totaling 374 files. The largest reference-behavior assets are snapshots, not fixtures: `packages/insomnia/src/main/importers/importers/__snapshots__/index.test.ts.snap` (238,453 B, 8,879 lines, 75 entries), `packages/insomnia/src/main/ipc/__tests__/__snapshots__/extract-postman-data-dump.test.ts.snap` (240,161 B), `packages/insomnia/src/templating/sandbox/__snapshots__/sandbox-surface.test.ts.snap` (22,132 B, 483 lines). There is no `*.har` file anywhere in the repository; HAR importer fixtures are JSON request objects, not archives.

## Fixture inventory

| Path | Kind | What it exercises | Size |
|---|---|---|---|
| `packages/insomnia/src/main/importers/importers/fixtures/` | directory, 11 format subdirs | All importers, one `*-input.*` file per case; every input has a matching snapshot entry | 75 files, 418,922 B |
| `.../fixtures/curl/` | 14 `.sh` cURL commands | `-d`, `-F`, `--data-urlencode`, `-u`, `--url`, header colon, `$` in strings, no URL, squished (skipped via `skip` prefix), Chrome copy | 14 files, 2,441 B |
| `.../fixtures/har/` | 5 `.json` | HAR `log.entries` (`deep-input.json` 3,986 B), bare-request root form (`shallow-input.json`), `postData.params` (`form-data-input.json`), no requests, minimal | 5 files, 5,521 B |
| `.../fixtures/insomnia-1/` | 3 `.json` v1 exports | complex, form, minimal legacy exports | 3 files, 2,425 B |
| `.../fixtures/insomnia-2/`, `insomnia-3/`, `insomnia-4/` | 1 export each | complex v2; basic v3; basic v4 YAML (`__export_format: 4`) | 1,556 B; 401 B; 311 B |
| `.../fixtures/openapi3/` | 14 `.json`/`.yaml`/`.yml` | Petstore (JSON/YAML, tags/readOnly variants), dereferenced (59,654 B and 60,151 B), server variables/no servers, global and endpoint security, multi API keys, OAuth2, Kong `x-kong-plugin` path | 14 files, 196,263 B |
| `.../fixtures/postman/` | 25 `.json` v2.0/v2.1 collections | Every auth type in both schema versions, URL forms, collection variables, faker vars, pre-request/test scripts, OAuth2 export | 25 files, 62,014 B |
| `.../fixtures/postman-env/` | 2 `.json` | basic env; env with no name | 2 files, 956 B |
| `.../fixtures/swagger2/` | 7 `.json`/`.yml` | Petstore 2.0 (JSON/YAML, tags), dereferenced, user example | 7 files, 135,463 B |
| `.../fixtures/wsdl/` | 2 `.wsdl` | calculator (8,831 B), addition (2,740 B); importer converts WSDL to Postman then to Insomnia | 2 files, 11,571 B |
| `packages/insomnia/src/common/__fixtures__/insomnia-v5-roundtrip/` | 14 v5 YAML exports | Full real-world airline demo: collection, spec, mock, MCP client, environments, sub-envs, `variable_inheritance`, `security_collection`, runner; one raw OpenAPI doc (`security.yaml`) that must import as a design doc | 14 files, 217,801 B |
| `packages/insomnia/src/common/__fixtures__/curl/complex-input.sh` | cURL script | End-to-end `importRaw` path | 391 B |
| `packages/insomnia/src/common/__fixtures__/har/test-response.json` | 15 B JSON body | HAR body export (`{"key":"value"}`) | 15 B |
| `packages/insomnia/src/common/__fixtures__/openapi/` | 2 YAML specs | `endpoint-security-input.yaml` (security schemes, 3,673 B); `smoke-test-with-operationIds.yaml` (2,329 B) | 6,002 B |
| `packages/insomnia/src/common/__fixtures__/postman/` | 2 collections | `aws-signature-auth-v2_0-input.json`; `collection-with-variable-v2_1-input.json` (base-env merge cases) | 1,995 B |
| `packages/insomnia/src/common/__fixtures__/prettify/` | 18 JSON | 9 input/output pairs; duplicate of the UI set and referenced by no test | 1,377 B |
| `packages/insomnia/src/common/__fixtures__/nestedfolders.ts` | TS model data | Folder/request tree; referenced by no test | 974 B |
| `packages/insomnia/src/ui/utils/prettify/fixtures/` | 44 files | 12 JSON pairs (escaped chars/unicode, brackets, whitespace, nunjucks, precision, root array/string, trailing comma, unquoted) + 10 EDN pairs | 44 files, 1,975 B |
| `packages/insomnia/src/network/grpc/__fixtures__/` | 12 files | `library/` proto2 streaming (`hello.proto`), proto3 public imports (`root.proto`), `route_guide.proto` (3,489 B) + `route_guide_db.json` (13,770 B), `nested/time/time.proto` importing `google/protobuf/*`, 6 empty protos under `simulate-error/` | 12 files, 18,125 B |
| `packages/insomnia/src/main/mcp/__fixtures__/tls/` | 5 PEM | CA-signed vs self-signed `localhost` cert pairs for MCP TLS trust tests | 5 files, 8,166 B |
| `packages/insomnia/src/plugins/context/__fixtures__/basic-import.json` | v4 export | Plugin import context sample; referenced by no test | 926 B |
| `packages/insomnia/src/main/ipc/__tests__/multi_postman_data_dump.zip` | zip | Postman data dump: archive + 7 collections + 7 environments | 38,819 B |
| `packages/insomnia-inso/src/commands/fixtures/` | 3 files | `openapi-spec.yaml`; `with-ruleset/.spectral.yaml` (`extends: [spectral:oas]`); `path-plugin.yaml` (Kong annotation) | 982 B |
| `packages/insomnia-inso/src/db/fixtures/` | 46 files | NeDB datafiles (13), v4 JSON/YAML/malformed/empty exports (6), v5 spec and with-tests (2), git repo and malformed-spec repo, TLS certs | 104,697 B |
| `packages/insomnia-inso/src/fixtures/` | 10 files | `.insorc` variants plus `run-collection-report/{default,plaintext,redact}-report.json` (3,274 / 10,628 / 10,875 B) | 26,301 B |
| `packages/insomnia-inso/src/examples/` | 15 `.yml` | Collections consumed by CLI integration tests (after-response, scripts, three-requests, run-collection-result-report, timeout, transient vars, minimal) | 34,625 B |
| `packages/insomnia-smoke-test/fixtures/` | 117 files, 48 top-level YAML/TS | UI-level collections: `pre-request-collection.yaml` (50,966 B), `runner-collection.yaml` (12,471 B), `template-tag-collection.yaml` (11,291 B), `oauth.yaml`, `after-response-collection.yaml`, `vault-collection.yaml` (v4), `liquid-security-collection.yaml`, `smoke-test-collection.yaml` (SSE `text/event-stream` at line 129), `grpc.yaml`, `websockets.yaml`, `socket-io.yaml`, `graphql.yaml`, `openapi3.yaml`, `swagger2.yaml`, `mock-server.yaml`, `insomnia4.yaml`, sandbox/plugin collections, `export/Collection-A|B-wrk_*.yaml` (1,617 B each), certificates, `files/`, `import/` | 341,729 B |
| `packages/insomnia-test/misc/fixtures/` | 12 files | `files/custom.spectral.yaml`, `invalid.spectral.yaml`, `runner-data.json`, `upload-sample.txt`, bare git repo | 1,516 B |
| `packages/insomnia-testing/src/generate/fixtures/` | 10 files | 5 input/output pairs for test-code generation (empty, empty suite, basic, nested, complex) | 3,514 B |
| `schemas/insomnia.schema.5.1.json` | JSON Schema draft 2020-12 | Generated contract for the v5 file format, committed with a CI drift check | 165,468 B |
| `packages/insomnia/src/ui/components/example-openapi-specs.ts` | TS string constants | `petStoreSpec` (line 1), `todoSpec` (line 778), `blankSpec` (line 920) used by the design empty state | 25,348 B |

## Tests that define reference behavior

| Test path | Subject | How the expected value is expressed |
|---|---|---|
| `packages/insomnia/src/main/importers/importers/index.test.ts` | Every importer, fixture-driven | `Date.now` mocked to `1_622_117_984_000` (`:33`); `expect(results.data).toMatchSnapshot()` (`:44`); duplicate `_id` assertion over all resources (`:45-55`). 75 snapshot entries, one per input |
| `packages/insomnia/src/main/importers/importers/curl.test.ts` | cURL parsing | 46 inline cases (`:6-347`), `it.each(testCases)` (`:349`) with `toMatchObject([expected])` (`:351`); null for non-cURL input (`:355`); explicit `Authorization` beats `-u` (`:362-369`) |
| `packages/insomnia/src/main/importers/importers/postman.test.ts` | Postman v2.0/v2.1 mapping | 23 inline `toEqual`/`toMatchObject` expectations; Nunjucks rewrite (`:41-74`), auth headers (`:82-117`), basic (`:148`), bearer (`:179`, `:205`, `:226`), digest (`:249`), OAuth1 (`:274`), OAuth2 by version/PKCE (`:368`, `:384`, `:409`), path params (`:511`, `:540`), base env from collection variable (`:569`), description forms (`:595`, `:616`, `:629`) |
| `packages/insomnia/src/main/importers/importers/openapi-3.test.ts` | OpenAPI 3 conversion | 3 inline assertions: query-parameter flattening (`:9`, `:120`), allOf/oneOf/anyOf composition (`:184`, expects at `:384-495`) |
| `packages/insomnia/src/main/importers/importers/translate-postman-script.test.ts` | `pm.*` to `insomnia.*` script rewriting | Inline script/expected string tables (`:62`, `:184`); whole-script case (`:191`) |
| `packages/insomnia/src/main/importers/utils.test.ts` | Request/group/env defaults, lenient JSON | Inline `toEqual` (`:6-60`), `unthrowableParseJson` (`:62-75`) |
| `packages/insomnia/src/main/importers/__tests__/convert.test.ts` | Importer selection | Importer-not-found (`:8`); `it.each` dot-in-key invariant (`:40`) |
| `packages/insomnia/src/common/__tests__/har.test.ts` | HAR export (request, response, cookies, multipart) | Inline full `toEqual` HAR objects (`:596`, `:677`); confidential masking by `purpose` (`:447`, `:474`, `:478`, `:526`); body fixture `test-response.json` (`:107`, `:394`) |
| `packages/insomnia/src/common/__tests__/import.test.ts` | Import routing, base-env merging, operationId | Inline `toBe`/`toMatchObject`; path matching (`:15-58`), MCP YAML shape (`:66`), spec-id detection (`:108-136`), cURL fixture import (`:144`, `:176`, `:206`), Postman AWS fixture (`:234`, `:261`), OpenAPI security fixture (`:288`), collection-variable merge (`:302`, `:340`, `:381`, `:444`), operationId resolution (`:531`, `:543`, `:555`) |
| `packages/insomnia/src/common/__tests__/import-export-roundtrip.test.ts` | v5 import/export fixed point | Normalizes ids/timestamps (`:31`), embedded model-id mask (`:37-38`), `toEqual` of normalized second export (`:80-94`); design doc not wrapped in v5 envelope (`:97`) |
| `packages/insomnia/src/common/__tests__/unknown-field-ranges.test.ts` | Line ranges of unknown YAML fields | Inline line-number assertions via `lineOf` (`:19`); fixture `operations.yaml` (`:11-17`); `expect.objectContaining({ startLine })`; perf budget 5,000 ms (`:205-247`) |
| `packages/insomnia/src/sync/git/__tests__/canonical-file.test.ts` | Canonical YAML for Git sync | 20 inline behavior assertions (`:93-314`); round-trip of `confidentialKeys` (`:214`); drift detection vs hand edits |
| `packages/insomnia/src/common/__tests__/insomnia-v5.test.ts` | v5 import/export, key order, confidential flags | Inline object equality; stable key order for mock routes (`:466`), MCP request (`:586`), collection items and cookie jar (`:633`); `confidentialKeys` export (`:909`, `:956`, `:959`, `:960`) and restore (`:963-1010`) |
| `packages/insomnia/src/common/__tests__/import-v5-parser.test.ts` | Zod schema contract for v5 | 62 inline parse results and default values; auth union (`:242-330`), settings/cookie/body defaults (`:334-472`), websocket/socket.io (`:480-534`), gRPC defaults (`:517`), mock routes and MCP workspace (`:687-749`) |
| `packages/insomnia/src/common/__tests__/render.test.ts` | Render context, env cascade, confidential masking | 61 inline `toBe`/`toEqual`; `render()` (`:52`), `buildRenderContext` (`:116`), `collectConfidentialRawValues` (`:891`), `registerConfidentialValues` (`:1103`), recursive render (`:1177`), gRPC render (`:1301`, `:1329`), rendered request (`:1418`), context (`:1490`) |
| `packages/insomnia/src/common/templating/__tests__/local-template-tags.test.ts` | Built-in template tags | 53 inline exact values: response URL (`:61`), redacted URL (`:66`), large-int JSONPath precision (`:78-114`), floats (`:118-146`), XPath filters (`:149-289`), base64 (`:296-330`), hash known digests (`:337-364`), now (`:378-400`), uuid (`:410-416`), jsonpath (`:426-437`), prompt cache (`:444-511`), file isolation (`:517-545`) |
| `packages/insomnia/src/common/templating/__tests__/utils.test.ts` | Tag tokenizer, key paths, vault-id rewrite | 24 inline exact strings: `forceBracketNotation` (`:10-15`), `getKeys` (`:31-97`), `tokenizeTag` (`:115-245`), `unTokenizeTag` (`:255-315`), encoding (`:317-337`), vault tag id replacement (`:339-400`), undefined-variable extraction (`:402`) |
| `packages/insomnia/src/templating/__tests__/liquid-compat.test.ts` | LiquidJS compatibility engine | 99 inline `toBe`; variable interpolation (`:29`), control flow (`:51`), filters (`:74`), comments/raw (`:104`, `:116`), errors (`:137`), Nunjucks breaking changes (`:162-192`), prototype/injection isolation (`:253-320`), `unless`/`case`/`for`/`echo`/`liquid`/`increment`/`capture` (`:322-522`), XSS passthrough (`:523-557`), pollution and DoS resistance (`:587-632`), unicode (`:633-648`), `allowTags:false` (`:659`) |
| `packages/insomnia/src/templating/sandbox/sandbox-surface.test.ts` | Sandbox module/global surface | `toMatchSnapshot()` (`:17`) against a 483-line surface snapshot; every registry module must resolve (`:21`) |
| `packages/insomnia/src/templating/sandbox/plugin-tag-sandbox.test.ts` | Plugin tag execution in QuickJS | Inline parity assertions: base64 (`:104`), os bridge (`:127`), crypto (`:156`), registry gating (`:301`), manifest grants (`:441`), Buffer/process/crypto/URL stubs (`:505-671`), capability gating (`:705-805`) |
| `packages/insomnia/src/templating/sandbox/marshal.test.ts`, `surface-profiles.test.ts`, `vendored-libs.test.ts` | Marshal allowlist, capability ceilings, vendored bundles | Inline assertions (`marshal.test.ts:103`; `surface-profiles.test.ts:15-82`; `vendored-libs.test.ts:65-85`) |
| `packages/insomnia/src/network/__tests__/network.test.ts` | Request send pipeline and inheritance | 36 inline assertions: generic send (`:40`), urlencoded (`:153`), multipart (`:398`), unix socket (`:495`), HEAD (`:539`), netrc (`:625`), SSL verification off (`:670`), HTTP version (`:789`), expected headers (`:822`), set-cookie parsing (`:1041-1078`), auth/header inheritance (`:1089-1155`), response-tag re-render safety (`:1181-1294`), user-agent suppression (`:1296-1317`) |
| `packages/insomnia/src/network/__tests__/authentication.test.ts` | OAuth1, bearer, API key | Inline header/cookie/query values (`:8-197`); `getAuthObjectOrNull` (`:199-225`) |
| `packages/insomnia/src/common/utils/url/confidential-mask.test.ts` | Mask preservation through URL encoding and codegen | Inline strings (`:10`, `:16`, `:36-37`), including httpsnippet output |
| `packages/insomnia/src/ui/utils/prettify/json.test.ts` and `edn.test.ts` | Prettifier byte output | Input/output fixture pair compared with `toBe` (`json.test.ts:26`, `edn.test.ts:26`) |
| `packages/insomnia-testing/src/generate/generate.test.ts` | Test-suite code generation | 5 input/output fixture pairs, trimmed `toBe` (`:32`) |
| `packages/insomnia-testing/src/run/run.test.ts`, `integration/integration.test.ts` | Mocha execution stats and failures | Inline `stats`/`failures` expectations (`run.test.ts:34-64`; `integration.test.ts:8-71`) |
| `packages/insomnia-inso/src/cli.test.ts` | CLI command matrix and run-collection reports | Report fixtures compared with `toEqual` (`:232`) after `expect.any(Number/String)` for dynamic fields (`:239-257`); help-output snapshot (`:305`) |
| `packages/insomnia-inso/src/commands/export-specification.test.ts` | x-kong annotation stripping | Exact YAML string equality (`:29`, `:34-48`) |
| `packages/insomnia-inso/src/commands/lint-specification.test.ts` | Spectral lint | `isValid` boolean (`:39`, `:75`); custom ruleset fixture (`:46`, currently `it.skip`) |
| `packages/insomnia-inso/src/get-options.test.ts` | `.insorc` discovery and parse | Inline expected object (`:14-22`, `:35-44`) |
| `packages/insomnia-inso/src/db/adapters/{git,insomnia,ne-db}-adapter.test.ts` | Git, v4/v5 file, NeDB adapters | Fixture dirs (`git-adapter.test.ts:11-38`, `insomnia-adapter.test.ts:8-71`, `ne-db-adapter.test.ts:8-11`); insomnia adapter snapshot |
| `packages/insomnia-inso/src/reporters/fancy-reporter.test.ts` | Log rendering | 11 `toMatchSnapshot()` calls (`:54-96`) |
| `packages/insomnia-smoke-test/tests/smoke/template-tags-interactions.test.ts` | Rendered built-in tags in the UI | Expected-value table (`:12-58`): base64 `aW5zb21uaWEtdGVzdA==` (`:13`), file `File Tag Test` (`:19`), md5 `b9c076eabf32fa4cdd7573a6df12d33c` (`:25`), request URL `http://127.0.0.1:4010/echo?foo=bar` (`:40`); fixture loaded at `:62` |
| `packages/insomnia-smoke-test/tests/smoke/export.test.ts` | YAML project export vs golden files | `compareWithFixture` (`:56`, `:95`, `:177`, `:201`, `:221`) against `fixtures/export/Collection-A` / `Collection-B-*.yaml` (`:18-21`); helper normalizes dynamic fields (`playwright/utils/fixture-comparison.ts:13`) |
| `packages/insomnia-smoke-test/tests/smoke/import.test.ts` | Import from clipboard paths | `importFixture` of `import/multiple-workspaces.yaml` (`:6`) and `import/import-content-type-from-postman.json` (`:17`) |
| `packages/insomnia-smoke-test/tests/smoke/{runner,chained-responses,environment-cascade,header-templates,graphql,grpc,websocket,socket-io,oauth,mtls}.test.ts` | End-to-end protocol and runner behavior | Fixture imports and UI assertions, e.g. `runner-collection.yaml` (`runner.test.ts:10`), `files/runner-data.json` (`:153`), `grpc.yaml` (`grpc.test.ts:13`), certificates (`mtls.test.ts:73-77`) |

## Recommended golden test set for Heric

Copy the Insomnia paths below into `tests/golden/<area>/` in Heric (read-only source). Regenerate expected JSON from the source repo only when the test itself changes; otherwise assert against the checked-in expected values.

### `crates/heric-import`

- Copy all of `packages/insomnia/src/main/importers/importers/fixtures/` (75 inputs). Golden expected output: parse the 75 entries in `.../__snapshots__/index.test.ts.snap` and store one JSON resource list per input. Assert resource ids are unique within an export (`index.test.ts:45-55`) and that `__export_date` is ignored. Use the fixed clock `1_622_117_984_000` where output timestamps are part of the comparison.
- Copy `packages/insomnia/src/common/__fixtures__/insomnia-v5-roundtrip/` (14 YAML). Assert import then export then import reaches a fixed point after masking `id`, `created`, `modified` and embedded `xx_<32 hex>` ids (`import-export-roundtrip.test.ts:31-52`, `:80-94`). Assert `security.yaml` is not wrapped in the v5 envelope (`:97`).
- Copy the inline case tables from `curl.test.ts:6-347` (46 cases) into a Rust table test; include the non-cURL returns `None` case (`:355`) and the explicit-`Authorization` precedence case (`:362-369`).
- Copy the inline Postman expectations from `postman.test.ts` (auth, path params, base environment, descriptions). The base-env merge cases additionally need `common/__fixtures__/postman/collection-with-variable-v2_1-input.json` and the expectations in `import.test.ts:302-470`.
- Copy `packages/insomnia-smoke-test/fixtures/insomnia4.yaml`, `insomnia4-update.yaml`, `simple.yaml`, `template-tag-collection.yaml`, `vault-collection.yaml` (v4), `import/multiple-workspaces.yaml`, `import/import-content-type-from-postman.json`. They exercise real v4 to v5 update paths and multi-workspace files. Note: `insomnia4.yaml`, `insomnia4-update.yaml` and `openapi3.yaml` currently have no static `.ts/.tsx/.js` reference in the repository, so they are fixtures without a reference test.

### `crates/heric-openapi`

- Copy all of `.../importers/fixtures/openapi3/` (14) and `.../fixtures/swagger2/` (7). Use the snapshot entries for full expected trees; use `openapi-3.test.ts:9-495` as the focused golden for query flattening and `allOf`/`oneOf`/`anyOf` composition.
- Copy `common/__fixtures__/openapi/endpoint-security-input.yaml` plus expectations from `import.test.ts:288`, and `smoke-test-with-operationIds.yaml` plus `resolveOperationId` expectations (`import.test.ts:531-561`; implementation `common/import.ts:721`).
- Copy `packages/insomnia-inso/src/commands/fixtures/openapi-spec.yaml`, `path-plugin.yaml` and `with-ruleset/.spectral.yaml` for linting and Kong-annotation stripping. Expected exact YAML for annotation stripping is inline in `export-specification.test.ts:29-48`.
- Copy `packages/insomnia-inso/src/db/fixtures/insomnia-v4/insomnia_v4.{json,yaml}` and the malformed/empty/no-export-format/v3-format variants; these define adapter rejection behavior (`insomnia-adapter.test.ts:8-71`).

### `crates/heric-template`

- Copy `common/templating/__tests__/local-template-tags.test.ts` expectations for the 12 built-in tags (source definitions at `common/templating/local-template-tags.ts:15-1032`: faker `:18`, base64 `:35`, now `:95`, uuid `:150`, os `:170`, hash `:206`, file `:269`, jsonpath `:290`, cookie `:331`, prompt `:373`, response `:477`, request `:811`). Golden values include the response-tag JSONPath large-integer precision and XPath blocks.
- Copy `templating/__tests__/liquid-compat.test.ts` as the LiquidJS conformance suite (99 cases, all inline). Keep the Nunjucks-breaking-change group (`:162-192`) and the security groups (`:253-320`, `:523-557`, `:587-648`) as separate test names.
- Copy `common/templating/__tests__/utils.test.ts` (`tokenizeTag` `:115`, `unTokenizeTag` `:255`, `forceBracketNotation` `:10`, vault-id rewrite `:339-400`).
- Copy `common/__tests__/render.test.ts` sections for context building (`:116-890`), confidential masking (`:891-1176`), recursive render (`:1177-1300`) and rendered-request assembly (`:1418-1611`). These are inline expectations, not fixtures.
- Copy `packages/insomnia-smoke-test/fixtures/template-tag-collection.yaml` plus the expected table in `tests/smoke/template-tags-interactions.test.ts:12-58`. The file contains `__TEMPLATE_TAG_FILE_PATH` placeholders that the test substitutes (`:62-65`); replace with a local path in Heric.
- Copy `packages/insomnia-smoke-test/fixtures/liquid-security-collection.yaml` and `environment-cascade-collection.yaml` for isolation and cascade scenarios.

### `crates/heric-codegen`

- There is no per-target golden in the source repository. The only reference assertions are `common/utils/url/confidential-mask.test.ts:36-37` (masked URL and `Authorization: Basic` survive httpsnippet for cURL) and the dependency pin `packages/insomnia/package.json:122` plus the IPC call `src/main/ipc/main.ts:576`. Generate Heric's golden snippets from the source Insomnia for each httpsnippet target/client and check them in; until then, seed the tests with the two mask assertions and Heric's own cURL/fetch output.

### `crates/heric-http`

- Copy the inline expectations of `network/__tests__/network.test.ts` (headers, multipart, unix socket, netrc, SSL verification, HTTP version, cookie parsing, auth/header inheritance, response-tag non-re-render, user-agent suppression). These rely on a mock adapter, not fixtures, so port the assertions as table tests.
- Copy `packages/insomnia-smoke-test/fixtures/header-templates.yaml`, `client-certs.yaml`, `ipv6-collection.yaml`, `response-download-collection.yaml` and `certificates/localhost.pem`, `localhost-key.pem`, `rootCA.pem`, `rootCA-key.pem`, `client.crt`, `client.key` for TLS and header template behavior (certificate fixtures are consumed by `tests/critical/certificates.test.ts:13-36`, `tests/smoke/mtls.test.ts:73-77` and `tests/smoke/grpc-mtls.test.ts:38`).

### `crates/heric-auth`

- Port `packages/insomnia/src/network/__tests__/authentication.test.ts` (OAuth1 HMAC/RSA-SHA1, defaults, bearer prefixes, API-key header/cookie placement) and the Postman auth expectations in `postman.test.ts:82-490` (awsv4, basic, bearer, digest, oauth1, oauth2 by schema version and PKCE).
- Copy `packages/insomnia-smoke-test/fixtures/oauth.yaml` and `auth-types.yaml`; `auth-types.yaml` is exercised by `packages/insomnia-inso/src/cli.test.ts:46`.

### `crates/heric-protocols`

- Copy `packages/insomnia/src/network/grpc/__fixtures__/library/` (`hello.proto`, `root.proto`, `route_guide.proto`, `route_guide_db.json`, `nested/time/time.proto`) and `simulate-error/`. The `nested/time/time.proto:2-3` imports `google/protobuf/timestamp.proto` and `empty.proto`, so the parser must resolve well-known types without the files on disk.
- Port `network/grpc/__tests__/write-proto-file.test.ts:45-270` (individual/nested files, path traversal and `..` handling) and `parse-grpc-url.test.ts` (source `parse-grpc-url.ts:1`, `write-proto-file.node.ts:60`).
- Copy `packages/insomnia-smoke-test/fixtures/grpc.yaml`, `grpc-mtls.yaml`, `grpc-reflection-api.yaml`, `websockets.yaml`, `socket-io.yaml`, `graphql.yaml` for handler and payload shapes; `main/mcp/__fixtures__/tls/` plus `main/mcp/common.test.ts:76-176` for CA vs self-signed trust.

### `crates/heric-model` and `crates/heric-storage`

- Encode the 62 parser expectations of `common/__tests__/import-v5-parser.test.ts` as model validation tests (defaults for settings, cookies, body, websocket/socket.io, gRPC reflection; discriminated `type` union; mock/MCP variants).
- Use `schemas/insomnia.schema.5.1.json` (generated from `common/import-v5-parser.ts:488`, per `schemas/README.md:11-13`, drift-checked per `:118-123`) as the external contract for Heric's serializer.
- Copy `packages/insomnia-inso/src/db/fixtures/nedb/` (13 NeDB datafiles), `git-repo/.insomnia/` and `git-repo-malformed-spec/` for the NeDB reader and migration tests; `packages/insomnia-smoke-test/fixtures/insomnia-legacy-db/` (29 datafiles, 132,688 B) is the larger real-data corpus consumed by `tests/migration/local-to-cloud-projects.test.ts:6`.
- Port `common/__tests__/insomnia-v5.test.ts:466-1010` key-order and `confidentialKeys` expectations; canonical YAML output behavior comes from `sync/git/__tests__/canonical-file.test.ts:87-314`.

### `crates/heric-git`

- Copy `packages/insomnia/src/common/__fixtures__/insomnia-v5-roundtrip/` (already listed) and port `canonical-file.test.ts` (drift detection, sibling ordering by `created`, confidential-flag survival). Copy `packages/insomnia-test/misc/fixtures/git-repo/git-server.git` or `packages/insomnia-inso/src/db/fixtures/git-repo` as a bare repo fixture.

### `apps/cli`

- Copy `packages/insomnia-inso/src/fixtures/run-collection-report/*.json` (default, redact, plaintext) and `src/examples/run-collection-result-report.yml`. Assert equality with the report while allowing `expect.any(Number/String)` for the volatile fields listed at `cli.test.ts:239-257`.
- Copy `packages/insomnia-inso/src/fixtures/.insorc*.yaml`; only `.insorc-test.yaml`, `.insorc-blank.yaml` and `.insorc-missing-properties.yaml` have reference tests (`get-options.test.ts:12-44`). Copy the help-output snapshot from `packages/insomnia-inso/src/reporters/__snapshots__/fancy-reporter.test.ts.snap` and `src/reporter/__snapshots__/index.test.ts.snap` for log formatting.
- Copy `packages/insomnia-testing/src/generate/fixtures/` (5 pairs) if Heric generates Mocha suites.
## Gaps

- **HAR importer**: only the snapshot entry exists. `har/minimal-input.json` and `shallow-input.json` are bare request objects (root fallback), and `deep-input.json` is a WebInspector export; no fixture contains a full HAR archive with multiple `log.entries`, pages or response bodies. Obtain one by exporting a real HAR from Chrome DevTools and adding a `*-input.har` fixture plus expected resources.
- **WSDL importer**: two fixtures, snapshot only; no test asserts the Postman intermediate (`wsdl.ts:90-121`). No WSDL with SOAP 1.2 bindings, multiple services or imports.
- **Swagger 2.0 importer**: snapshot only; no inline tests. Hand-edit cases (body/query parameter combinations, `$ref` siblings) are unverified.
- **Legacy insomnia-1/2/3/4 importers**: snapshot only; v2/v3/v4 have one fixture each, so field coverage is thin (no environments, unit tests or certificates in those fixtures).
- **Postman environment importer**: two fixtures, snapshot only.
- **Codegen**: no golden snippets for any httpsnippet target/client, and no test for the target list (`availableTargets` imported at `main/ipc/main.ts:19`). Heric's `Target` enum currently exposes only Curl and Fetch (`crates/heric-codegen/src/lib.rs`).
- **SSE**: the only fixture is the `text/event-stream` request inside `smoke-test-collection.yaml:129`, exercised through the UI; there is no golden event-log parser fixture.
- **Plugins/sandbox**: unit tests assert inline values and a surface snapshot, but there is no fixture plugin package with expected rendered output; `examples/insomnia-plugin-sandbox-demo` exists but is unreferenced by tests.
- **Unreferenced fixtures**: `common/__fixtures__/prettify/` (duplicate of the UI set), `common/__fixtures__/nestedfolders.ts`, `plugins/context/__fixtures__/basic-import.json`, `insomnia-smoke-test/fixtures/inso-nedb/`, and `.insorc-from-file-example.yaml`, `.insorc-from-git-example.yaml`, `.insorc-with-scripts.yaml`, `.insorc.yaml`. `insomnia4.yaml`, `insomnia4-update.yaml`, `openapi3.yaml` and `mock-server.yaml` have no static reference in `.ts/.tsx/.js`. Treat these as candidate inputs, not reference behavior.
- **No fixture for**: mock-route matching/routing, OAuth2 token refresh exchange, response download streaming, WebSocket/Socket.IO message golden payloads, gRPC reflection result trees (only UI smoke), and CLI exit codes beyond `cli.test.ts:83-120`.
- **Snapshot coupling**: 75 importer expectations and the Postman data-dump snapshot are stored as Vitest snapshots with regenerated formatting; Heric must store them as plain JSON goldens, not snapshot text.

## Feature list

**P0**
- Importer registry order and trial conversion — `packages/insomnia/src/main/importers/importers/index.ts:15-28`, `src/main/importers/convert.ts:21`. Edge cases: each input is offered to every importer; cURL must return null for non-cURL input (`curl.test.ts:355`); WSDL conversion runs Postman conversion internally (`wsdl.ts:102-121`).
- Insomnia v5 read — `src/common/import-v5-parser.ts:488`, schema version `'5.1'` at `src/common/insomnia-schema-migrations/schema-version.ts:19`. Edge cases: future/unsupported `schema_version`, malformed YAML, invalid literal shapes (`import-v5-parser.test.ts:149-174`, `insomnia-v5.test.ts:85-126`).
- Insomnia v5 write with stable key order and confidential flags — `src/common/insomnia-v5.ts:1043`; `insomnia-v5.test.ts:466`, `:586`, `:633`, `:909-1010`.
- v5 round-trip fixed point — `common/__tests__/import-export-roundtrip.test.ts:80-94`; design docs are not wrapped in the envelope (`:97`).
- Insomnia v4 import/export — `importers/insomnia-4.ts:17`; fixture `insomnia-4/basic-input.yaml`.
- Postman v2.0/v2.1 import — `importers/postman.ts:964`; auth mapping, URL path parameters, collection variables to base environment, description object forms (`postman.test.ts`).
- cURL import — `importers/curl.ts:468`; 46 inline cases plus non-cURL null and explicit Authorization precedence.
- OpenAPI 3 import — `importers/openapi-3.ts:801`; query object flattening, schema composition, server variables, security schemes, Kong `x-kong-plugin` path keys (`openapi3/path-plugin-input.yaml`).
- HAR export with confidentiality masking — `src/main/har.ts:129`, `:190`, `:222`, `:246`, `:292`; mask value `'••••••'` (`common/templating/confidential-value-policy.ts:13`), fail-closed default (`:39-44`); expected HAR objects at `har.test.ts:596`, `:677`, masking at `:447-537`.
- HTTP request pipeline and auth/header inheritance — `src/network/__tests__/network.test.ts:1089-1155`; response-sourced values must not be re-rendered (`:1181-1294`).
- Template rendering and built-in tags — `src/templating/index.ts:25`, `src/common/templating/local-template-tags.ts:15-1032`; tag argument tokenizer `src/common/templating/utils.ts:57`, `:71`.
- Environment cascade and render context — `src/common/render.ts:355`, `:715`, `:837`; confidential raw-value collection `:327`, registration `:301`.
- Liquid compatibility and security boundaries — `src/templating/__tests__/liquid-compat.test.ts` (all 99 cases).

**P1**
- Postman environment import — `importers/postman-env.ts:26`; no-name fixture.
- HAR import — `importers/har.ts:99`; root-request fallback and `log.entries` mapping.
- Swagger 2.0 import — `importers/swagger-2.ts:495`; 7 fixtures.
- insomnia-1/2/3 import — `insomnia-1.ts:136`, `insomnia-2.ts:13`, `insomnia-3.ts:12`.
- WSDL import — `wsdl.ts:102`; two fixtures, Postman intermediate.
- Postman script translation (`pm.*` to `insomnia.*`) — `importers/translate-postman-script.test.ts:191`.
- operationId resolution — `src/common/import.ts:721`; fixture `smoke-test-with-operationIds.yaml`.
- Canonical YAML for Git sync and drift detection — `src/sync/git/__tests__/canonical-file.test.ts:87-314`.
- CLI command surface, `.insorc`, Spectral linting and run-collection reports — `packages/insomnia-inso/src/cli.test.ts:200-233`, `commands/export-specification.test.ts:27-48`, `commands/lint-specification.test.ts:39-75`, `get-options.test.ts:12-44`, report fixtures.
- gRPC URL parsing and proto file writing — `src/network/grpc/parse-grpc-url.ts:1`, `src/network/grpc/write-proto-file.node.ts:60`; path traversal cases `write-proto-file.test.ts:169-270`.
- TLS trust model (CA vs self-signed) — `src/main/mcp/common.test.ts:150-176`; fixture `main/mcp/__fixtures__/tls/`.
- NeDB reader — fixture corpus `packages/insomnia-smoke-test/fixtures/insomnia-legacy-db/` and `packages/insomnia-inso/src/db/fixtures/nedb/`.
- Test-suite code generation — `packages/insomnia-testing/src/generate/generate.test.ts:32`, fixtures `01_empty` through `05_complex-suite`.
- Sandbox module surface and capability gating — `src/templating/sandbox/module-registry.ts:131`; tests `sandbox-surface.test.ts:17`, `surface-profiles.test.ts:15-82`, `plugin-tag-sandbox.test.ts:301-805`.

**P2**
- JSON and EDN prettification — `src/ui/utils/prettify/json.ts:53`, `edn.ts:165`; 22 fixture pairs.
- Unknown-field line-range reporting for editor diagnostics — `src/common/__tests__/unknown-field-ranges.test.ts`; `operations.yaml`.
- Postman data-dump zip extraction — `src/main/ipc/__tests__/extract-postman-data-dump.test.ts:11`; 38.8 KB zip.
- Plugin context import sample — `src/plugins/context/__fixtures__/basic-import.json` (currently unreferenced).
- Fancy CLI reporter formatting — `packages/insomnia-inso/src/reporters/fancy-reporter.test.ts:54-96`.
- Legacy prettify fixture duplicate cleanup — `src/common/__fixtures__/prettify/`.
