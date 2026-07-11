# ADR 0909: Config cache window debug redaction

## Status

Accepted.

## Context

Configuration cache windows carry refresh, stale, and expiry timestamps. These
timings guide cache behavior but should not leak through generic diagnostics or
containing plan debug output.

## Decision

Use a custom `Debug` implementation for `ConfigCacheWindow` that preserves the
field names while redacting the timing values. Keep exact values available
through typed fields for cache refresh evaluation.

Regression coverage rejects raw cache timing values in rendered debug output.

## Consequences

Configuration cache state and activation plans can be inspected without
exposing cache timing internals. Cache refresh behavior remains unchanged.
