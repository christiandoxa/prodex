# ADR 0721: Redact domain rate-limit debug output

## Status

Accepted

## Context

Rate-limit domain state carries tenant IDs, optional virtual-key IDs, window
timestamps, usage counts, remaining capacity, and atomic update internals.
Stable client response planners already hide those details, but derived `Debug`
output could still print them through panic diagnostics or generic structured
logging.

## Decision

Implement custom `Debug` for rate-limit snapshots, bucket keys, requests,
allowances, rejections, decisions, and atomic updates. The debug output
preserves the state shape while replacing tenant, virtual-key, window, and
usage details with redacted placeholders.

## Consequences

- Rate-limit evaluation, cache keys, serialization, and atomic update contracts
  are unchanged.
- Generic debug logs no longer expose raw tenant or virtual-key topology.
- Operators should use explicit, redacted observability events for rate-limit
  diagnostics instead of relying on derived debug dumps.
