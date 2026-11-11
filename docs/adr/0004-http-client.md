# 0004 — HTTP client

- Status: accepted
- Date: 2026-11-10

## Context

The original client uses node-libcurl. That choice is visible in the product:
the Console timeline is libcurl's debug callback rendered on screen
(`HTTP-052`, P0), multipart bodies are built by hand and handed to libcurl
(`HTTP-004`, `HTTP-005`), and NTLM, Digest, client certificates, proxy PAC
resolution and the cookie engine all come from libcurl. The parity matrix
inherited from that codebase has 422 rows, and the rows that describe wire
behaviour assume libcurl's semantics.

Two candidates were considered for the Rust rewrite.

1. **`reqwest` over `hyper` with `rustls` and `rustls-platform-verifier`** —
   the safe, idiomatic choice, which is what this ADR originally proposed.
2. **The `curl` crate** (`curl` + `curl-sys`, vendoring libcurl 8.22.0) —
   the same engine the original uses.

The evidence that decides between them was gathered on 2026-11-10 and is
recorded below rather than left implicit.

**Timing fidelity.** `hyper` exposes no per-phase timings; its `Connected`
extension carries only peer and local addresses. `reqwest` has no timing API
at all and does not re-export hyper's connection metadata. `ureq` exposes no
timings. libcurl exposes seven phase counters, and the `curl` crate binds all
seven. A request timeline that shows DNS, connect, TLS and transfer phases is
a P0 feature, and it cannot be built on `reqwest` without hand-rolling a
timing stack next to a client that already has one.

**Auth and transport.** NTLM and Digest are P0 at M3. `reqwest` supports
Digest through a middleware crate and does not support NTLM. libcurl supports
both natively, with a known caveat recorded below.

**Timeline.** The persisted Console timeline is literally the engine's debug
callback output. Reimplementing it on top of `hyper` means reimplementing the
exact event stream the user already recognizes.

## Options

### Option 1 — `reqwest` / `hyper`

Pros: no C toolchain, no `unsafe` dependency, the mainstream Rust HTTP stack,
well documented, `rustls` plus `rustls-platform-verifier` exactly as the
project brief requested.

Cons, each verified:

- No phase timings, so the timeline feature has no data source.
- No NTLM.
- The debug/console log would have to be synthesised from `tracing` events,
  which produces a different order and different text from libcurl's.

### Option 2 — the `curl` crate

Pros:

- The same engine as the original, so wire behaviour, timeline events,
  multipart building, redirect and cookie semantics match by construction.
- Every required transport feature exists in the crate's API: Digest, NTLM,
  client certificates (PEM, blob, key type), proxy types including SOCKS4/5
  and PAC results, Unix sockets, HTTP/2, and the cookie engine.
- `curl` and `curl-sys` are MIT, so they clear the `cargo-deny` allow-list.

Cons, each verified:

- TLS backend is a compile-time cargo feature, not a runtime choice. libcurl
  can reach OpenSSL, Schannel, rustls (through `rustls-ffi`, marked
  EXPERIMENTAL upstream since 7.76.0) or SecTrust on macOS. It cannot reach
  `rustls-platform-verifier` directly; `SslOpt::native_ca` routes to it only
  on Windows and macOS, and on Linux it means the Mozilla bundle rather than
  the system store. This is a departure from the brief, recorded in ADR 0010.
- NTLM is at risk outside Windows: `curl-sys` 0.4.91 does not define
  `CURL_ENABLE_NTLM` (required since libcurl 8.20) nor
  `HAVE_DES_ECB_ENCRYPT`, so the `ntlm` feature compiles without NTLM
  symbols on Unix (`curl-sys` issue #665, open at the time of writing).
- Timing precision is bounded by the `f64`-second getters the crate exposes;
  the microsecond `_TIME_T` counters are not bound. Phase values match the
  original's phases, not its last digits.
- `curl-sys` compiles C sources with the `cc` crate, which requires a C
  toolchain on every building machine. It does not require cmake, make,
  autotools or nasm.
- A statically linked libcurl carries an attribution obligation (the `curl`
  licence's name-use clause, plus `ISC` and `BSD-4-Clause-UC` files in the
  vendored tree). None of these is copyleft; the cure is a
  `THIRD-PARTY-NOTICES` file in the release archives.

## Decision

Use the `curl` crate.

The parity matrix ranks timeline fidelity (`HTTP-052`) and NTLM/Digest
(`HTTP-037`) as P0, and options 1's own escape clause — "if the inventory
ranks timeline fidelity as P0, this ADR is revisited in favor of the `curl`
crate" — is met. Choosing the same engine as the original also removes an
entire class of "works in Insomnia, does not work in Heric" defects, which is
the project's central requirement.

The engine sits behind a trait (`heric_http::HttpEngine`), so the transport is
replaceable without touching the send pipeline, and tests substitute a fake.

## Consequences

- `heric-http` depends on `curl`; the workspace gains a C build step, which
  every CI runner already satisfies.
- The TLS backend is decided in ADR 0010 after a four-target build spike.
  System trust roots on all three platforms remain a hard requirement; if no
  libcurl backend can satisfy it, this ADR is revisited before M1 closes.
- NTLM on non-Windows depends on an upstream `curl-sys` fix or a local patch.
  The M3 plan carries that risk explicitly instead of discovering it late.
- Timeline numbers are phase-accurate, not bit-identical to the original.
  Tests compare phase ordering and presence, never exact durations.
- Release artefacts include `THIRD-PARTY-NOTICES`.
- One dependency is `unsafe` internally. It is a `-sys` binding, which is the
  one category that cannot be safe Rust; our crates keep
  `unsafe_code = "forbid"`, and we never call `curl_sys` directly.
