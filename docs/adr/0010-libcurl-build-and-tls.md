# 0010 — libcurl build and TLS backend

- Status: proposed
- Date: 2026-11-10

## Context

ADR 0004 selects the `curl` crate, which means libcurl is built from source
through `curl-sys` (0.4.91, vendoring libcurl 8.22.0). Two questions follow
and neither is answerable from documentation alone.

**How does it build?** `curl-sys` compiles the C sources directly with the
`cc` crate. It does not invoke cmake, make, autotools, perl or vcpkg. On
Windows it rewrites `curl/lib/config-win32.h` into `curl_config.h`, defines
`HAVE_CONFIG_H`, `WIN32`, `USE_THREADS_WIN32` and `USE_WINSOCK`, and links
`ws2_32 crypt32 iphlpapi advapi32 secur32 bcrypt`. nasm is needed only by
transitive dependencies of features this ADR does not select:
`rustls` pulls `rustls-ffi` and `aws-lc-sys`, and `static-ssl` pulls
`openssl-src`. So the requirement is a C toolchain and nothing more.

**Which TLS backend?** This is a compile-time cargo feature. `curl-sys`
chooses one of `rustls`, `windows-static-ssl` or `ssl`, in that order of
precedence. The default feature set includes `ssl`, which means Schannel on
Windows and OpenSSL elsewhere.

The hard requirement inherited from the product is `PLT-024`: the operating
system's CA store is always enabled, and a workspace CA is an *addition* to it,
never a replacement. `rustls-platform-verifier` was the project brief's
suggestion; it is not reachable directly through libcurl.

## Options

### A — `ssl` (platform default) plus `apple-sectrust` on macOS

| Platform | Backend | Trust roots |
|---|---|---|
| Windows | Schannel | OS store, automatic (CurrentUser + LocalMachine `ROOT`) |
| Linux | OpenSSL | `openssl_probe` sets `cainfo`/`capath` to the distribution bundle |
| macOS | OpenSSL + SecTrust | `apple-sectrust` evaluates the Keychain policy |

- Requires `libssl` at link time on Linux, or the `static-ssl` feature (which
  drags in `openssl-src`, perl and nasm).
- No hand-maintained CA bundle anywhere.
- `SslOpt::native_ca` is unnecessary on Windows and meaningless on Linux.

### B — `rustls`

- `SslOpt::native_ca(true)` routes to `rustls-platform-verifier` on Windows
  and macOS 10.14+. **On Linux it means the Mozilla root bundle, not the
  system store**, which fails `PLT-024` as written.
- Pulls `rustls-ffi` (explicitly unstable API) and `aws-lc-sys` (nasm
  required on Windows x86 and x86-64).
- libcurl marks the rustls backend EXPERIMENTAL since it landed in 7.76.0.
- `curl-sys` issue #687: `rustls` combined with `static-curl` installs no
  certificate verifier at all, so every HTTPS request fails until
  `native_ca` is set on each handle. Relying on that is relying on a
  configuration flag to compensate for a build defect.

### C — `windows-static-ssl`

Reported broken on MSVC in `curl-sys` issue #666, and `#667` reports SSPI is
not compiled into that build. Not viable.

## Decision

Option **A**, subject to a build spike.

Reasons: it satisfies `PLT-024` on all three platforms without a maintained
bundle, it keeps OpenSSL out of the binary wherever the platform provides an
alternative (Schannel on Windows, SecTrust on macOS), and it avoids the
experimental rustls path together with its nasm and `aws-lc-sys` baggage.

The residual risk is Linux: it needs either a system `libssl` at build time or
`static-ssl`. The project's "no vendored OpenSSL" preference pushes toward the
system library, which is already installed in CI (`libssl-dev`) and is a normal
runtime dependency on every desktop distribution.

This ADR stays **proposed** until one spike passes: building and running the
test suite for the workspace with the selected feature set on all four release
targets (`x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`,
`aarch64-apple-darwin`, `x86_64-apple-darwin`) in CI. If the spike fails, the
fallbacks in order are: `static-ssl` on Linux, then revisiting ADR 0004.

## Consequences

- CI gains no new system packages: a C toolchain is already present on all
  runners, and `libssl-dev` is already installed on the Linux job for other
  reasons.
- A locally installed MSVC build tools or equivalent is a prerequisite for
  Windows contributors. That is already true for the `egui` stack.
- `PLT-024` is implemented as: native store always on (`SslOpt::native_ca` on
  Windows/macOS where it is meaningful), plus the workspace CA through
  `CAINFO_BLOB` when a certificate record is enabled. A missing CA file fails
  the request rather than silently falling back.
- Static linkage of libcurl inside the binary keeps the installers
  self-contained, and the `THIRD-PARTY-NOTICES` file from ADR 0004 covers the
  attribution obligation.
