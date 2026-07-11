# ADR 0243: Runtime policy explicit reload cache invalidation

## Status

Accepted

## Context

Runtime policy loading is cached per Prodex root to avoid repeatedly parsing the
same policy file. Control-plane configuration publication now emits an explicit
runtime policy reload target, but the runtime policy crate only exposed global
cache clearing and cached loads.

Production adapters need a narrow operation for one affected root. Global cache
clears are too broad, while ordinary cached loads can keep serving a stale policy
after a publication event.

## Decision

`prodex-runtime-policy` now exposes:

- `invalidate_runtime_policy_cache_for(root)`, which removes only the selected
  root entry from the process cache; and
- `reload_runtime_policy_cached(root)`, which invalidates that root entry before
  reading and storing the current policy.

The reload path is explicit and adapter-triggered. Request handlers should keep
using already-loaded snapshots and should not perform reload work directly on
the hot request path.

## Consequences

Runtime policy reload wiring can consume configuration publication events
without clearing unrelated tenants or roots. Tests prove that a changed policy
file remains cached until explicit reload and that reload updates the cached
value.

This remains a local process cache. Multi-replica propagation still belongs to
the production event transport selected by the composition root.

## Follow-up

ADR 0280 adds a redacted invalidation plan for publication-event consumers that
need cache-refresh evidence without exposing full runtime policy material.
