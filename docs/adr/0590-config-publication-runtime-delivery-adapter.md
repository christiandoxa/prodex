# ADR 0590: Config Publication Runtime Delivery Adapter

## Status

Accepted

## Context

Configuration publication planning already requires a gateway cache refresh
target and a runtime policy reload target. The legacy production composition
root still needed an adapter that consumes the publication event and refreshes
local gateway/runtime policy state instead of leaving stale cached policy in
process.

## Decision

Add a small production-exported `prodex-app` runtime-policy delivery adapter
for config publication events. The adapter validates that both required event targets are present,
marks the gateway cache refresh target as delivered for the local process,
invalidates the runtime policy cache for the resolved root, and immediately
reloads the runtime policy from disk. It also returns the existing
low-cardinality configuration publication delivery metric plans for the gateway
cache refresh and runtime policy reload targets.

## Consequences

- Local gateway/runtime policy cache state no longer keeps serving a stale
  cached policy after a successful publication event.
- The delivery adapter has the telemetry plans needed to publish closed-label
  delivery counters without placing tenant IDs, revision IDs, roots, or payloads
  in metric labels.
- Publication delivery remains in the composition root instead of adding a
  runtime-policy dependency to the side-effect-free application boundary.
- A future multi-process control-plane deployment still needs an external event
  transport to call this adapter in every gateway replica.
