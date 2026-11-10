# 0004 — HTTP client

- Status: proposed
- Date: 2026-11-10

## Context

The original client uses node-libcurl and exposes a request timeline with
per-phase timings (DNS, connect, TLS, transfer), plus libcurl-specific
behaviors: client certificates, proxy handling, redirects, NTLM/Digest and
cookie jars.

## Options

1. `reqwest` over `hyper` with `rustls` and `rustls-platform-verifier`.
2. The `curl` crate, which links libcurl and reproduces the original engine
   most faithfully.

## Decision

Proposed: option 1 for M1, with a thin abstraction so the engine can change.
The cost is losing parts of the libcurl timeline and NTLM, which are P1/P2 and
belong to M3.

## Consequences

- No OpenSSL dependency, system trust roots on all three platforms.
- The timeline is approximated from Hyper's connection phases; if the
  inventory ranks timeline fidelity as P0, this ADR is revisited in favor of
  the `curl` crate.
