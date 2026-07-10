# ADR 0989: Provider spend debug redaction

## Status

Accepted.

## Context

Provider gateway spend events carry request and call identifiers, virtual-key
names, tenant selectors, upstream paths, model names, token counts, byte counts,
latency, and cost estimates. Structured runtime-log output intentionally keeps
its existing compatibility fields, but derived `Debug` output can leak those
values through panic output, assertions, or diagnostics.

## Decision

`RuntimeProviderGatewaySpendEvent` uses a hand-written `Debug` implementation
that preserves low-cardinality event, phase, provider, status, and sink shape
while redacting key, tenant, request/call, path, model, token, byte, latency,
and cost fields.

## Consequences

Serialization and runtime-log emission are unchanged. Debug diagnostics remain
useful for identifying the spend event shape without exposing tenant or billing
metadata. Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_spend.rs`.
