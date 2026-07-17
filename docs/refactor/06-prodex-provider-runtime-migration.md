# Provider Runtime Follow-on

Status: design note for the next PR-sized migration. This document does not authorize runtime
changes in the provider implementation registry PR.

## Boundary

The future `prodex-provider-runtime` crate should own concrete provider credential pools, auth
decoration, transport assembly, process lifecycle, pre-commit retry state, and stream sequencing.
Pure request, response, and SSE transformations remain in `prodex-provider-core`. Validated
`ProviderInvocation`, `SecretRef`, capability decisions, and retry plans remain in
`prodex-provider-spi`.

A runtime adapter receives a route that has already been selected and authorized. It must not:

- choose another tenant or provider;
- resolve arbitrary secrets;
- bypass admission or authorization;
- retry after commit; or
- weaken continuation affinity.

Transport remains host-owned. Provider adapters must use host-supplied transport so proxy
configuration, TLS policy, redaction, logging, metrics, cancellation, connection reuse, and
upstream error pass-through cannot be bypassed.

## Migration unit

Move one built-in runtime adapter per PR. Each move requires byte-identical request/response
fixtures and event-order-identical stream tests against the existing application implementation.
Do not move profile selection, affinity ownership, quota classification, admission, or post-commit
retry policy into an adapter.

The recommended first extraction is DeepSeek. Its credential and HTTP lifecycle are narrower than
the OAuth-backed providers, while its pure Responses-to-chat transformations already live in
`prodex-provider-core`. This isolates the runtime boundary without changing WebSocket reuse or
OAuth refresh behavior.

If external providers are supported later, prefer an out-of-process, versioned, allowlisted
protocol over in-process Rust dynamic libraries.

## Refresh model

Future provider and credential refresh can adopt the useful delta-coalescing idea from
CLIProxyAPI while retaining Prodex's stricter safety model:

1. Build and validate a candidate snapshot off the request hot path.
2. Coalesce repeated updates by provider or credential identity.
3. Atomically replace the active snapshot through `ArcSwap` only after complete validation.
4. Preserve the last-known-good snapshot when candidate construction or validation fails.
5. Never scan filesystems or perform broad reload work inside request dispatch.

The implementation registry remains immutable code metadata. Refreshable tenant governance,
routing scores, credentials, and runtime health remain separate snapshots with separate owners.
