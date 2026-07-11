# ADR 0016: Mark Gateway Admin Responses as No-Store

## Status

Accepted

## Context

Gateway admin responses can contain sensitive operational metadata, virtual-key configuration, billing ledger details, generated one-time key material, or OpenAPI contract details for an internal control plane. Enterprise deployments often place admin APIs behind shared proxies and browser-based operators, so cacheable admin responses increase accidental disclosure risk.

## Decision

Prodex-owned gateway admin JSON and CSV responses now include:

```text
Cache-Control: no-store
```

The header is applied through the shared admin response helpers and the virtual-key `ETag` response helper. It does not alter upstream OpenAI-compatible data-plane pass-through behavior.

## Consequences

Admin clients and intermediate caches are instructed not to store control-plane responses. Existing API shapes and status codes remain unchanged.
