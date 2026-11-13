### Task 15: ADR 0010 spike and the libcurl engine

**Files:**
- Modify: `Cargo.toml`, `crates/heric-http/Cargo.toml`
- Create: `crates/heric-http/src/engine.rs`
- Modify: `crates/heric-http/src/lib.rs`
- Test: `#[cfg(test)]` in `engine.rs`, plus `crates/heric-http/tests/pipeline.rs`

**Interfaces:**
- Produces: `heric_http::engine() -> Result<Box<dyn HttpEngine>, HttpError>`; `pub(crate) struct CurlEngine`; `engine::map_curl_error(error: curl::Error) -> HttpError`.

**Manifest (ADR 0010 option A):**

```toml
# [workspace.dependencies]
curl = "0.4.51"
# crates/heric-http/Cargo.toml
curl = { workspace = true }
[target.'cfg(target_os = "macos")'.dependencies]
curl = { workspace = true, features = ["apple-sectrust"] }
```

- [ ] **Step 1: Run the spike before writing engine code.** Add the manifest lines with a body that only constructs `curl::easy::Easy::new()`, push the branch, and confirm all four `test` jobs build and pass. If any target fails, apply ADR 0010's fallback order (`static-ssl` on Linux, then revisit ADR 0004) and record the observed result in the ADR. No engine code before this is green.
- [ ] **Step 2: Write the failing tests** against `wiremock`

```rust
#[test]
fn a_get_returns_status_headers_and_body() -> Result<(), Box<dyn std::error::Error>> {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(201).header("x-test", "yes").body("hi");
    });
    let execution = execute(&server.url("/hello"))?;
    assert_eq!(execution.status, 201);
    assert!(execution.headers.iter().any(|h| h.name.eq_ignore_ascii_case("x-test") && h.value == "yes"));
    assert_eq!(execution.body, b"hi");
    Ok(())
}

#[test]
fn no_content_has_a_status_and_no_body() { /* 204: status 204, body empty */ }
#[test]
fn a_cancelled_token_stops_the_transfer() { /* cancel from another thread -> HttpError::Cancelled */ }
#[test]
fn the_timeline_records_request_then_response_headers() { /* HeaderOut before HeaderIn */ }
#[test]
fn an_unreachable_port_is_a_connect_error() { /* bind and drop a listener */ }
#[test]
fn every_phase_timing_is_monotonic() { /* name_lookup <= connect <= pre_transfer <= total */ }
```

- [ ] **Step 3: Implement** `CurlEngine::execute`: set URL, method, headers, body from `Body::{Empty, Bytes, File}` (`File` via `read_function`, so a large upload never enters memory), `ACCEPT_ENCODING` so libcurl decompresses as the original does, timeouts from `Timeout`, `RedirectPolicy` with `max_redirects`, `CAINFO_BLOB` from `TlsConfig::workspace_ca` while never disabling the native store (`PLT-024`), the proxy from `ProxyConfig`, and a progress callback reading `CancelToken`. Map `curl::Error` by category into `HttpError::{Cancelled, Timeout, InvalidUrl, Dns, Connect, Tls, Protocol, TooManyRedirects, Engine}`. Build `Timings` from `curl::easy::Info` and push `TimelineEntry` values to `options.timeline` when present.
- [ ] **Step 4: `FIX-012`.** Port the 52 cases from `packages/insomnia/src/network/__tests__/network.test.ts` into `tests/pipeline.rs`, covering the parts of the pipeline Heric owns now: headers, HTTP version, cookie parsing, inheritance, and "a response tag is never re-rendered". Cases that need multipart, a unix socket or netrc get a named `#[ignore = "needs multipart (M3)"]`, and every skip is listed in the PR description.
- [ ] **Step 5: Verify and commit** — `cargo test -p heric-http`, then `feat(http): execute requests through libcurl`

