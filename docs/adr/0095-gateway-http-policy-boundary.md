# ADR 0095: Gateway HTTP Policy Boundary

## Status

Accepted

## Context

The enterprise target requires the hot data-plane HTTP path to become async,
bounded, observable, and transport-transparent. The existing local rewrite proxy
still uses framework-specific request/response types across many handlers. A
big-bang replacement with Axum/Hyper would risk changing streaming, upstream
error compatibility, continuation affinity, and provider fallback behavior in the
same patch.

## Decision

Add `prodex-gateway-http` as a framework-neutral HTTP policy boundary before
introducing a concrete async server adapter. It owns:

- data-plane/control-plane/health route classification;
- bounded request body policy;
- explicit request, stream-idle, and connection-drain timeout budgets;
- required traceparent propagation for data-plane and control-plane routes;
- method validation per route type;
- preservation of Codex metadata headers;
- stripping of hop-by-hop and selected-profile auth headers.

Add `scripts/ci/gateway-http-boundary-guard.mjs` and wire its self-test plus
workspace scan into preflight. The guard keeps this crate free of Axum, Hyper,
Tower, Tokio, reqwest, storage
drivers, provider SDKs, filesystem/network/process access, and transport
implementations. Concrete async adapters can depend on this crate later.

## Consequences

HTTP behavior can now be regression-tested independently from server framework
migration. Later `prodex-gateway` wiring can translate Axum/Hyper requests into
these boundary types while preserving existing Codex transport semantics and
runtime proxy invariants.
