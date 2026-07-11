# 0237: Config Invalidation Plan

## Status

Accepted

## Context

Revisioned gateway policy/config caches need explicit invalidation semantics.
The boundary already models refresh windows, activation, and last-known-good
fallback, but adapters could still mutate `invalidated_revision_id` directly and
drift on tenant checks, unknown revisions, or fallback behavior.

## Decision

`prodex-config` owns `plan_config_invalidation`.

The planner:

- requires the request tenant to match the cache tenant;
- allows invalidation only for the active or last-known-good revision;
- records the invalidated revision without mutating active or LKG pointers;
- returns stable redacted error responses through
  `plan_config_invalidation_error_response`.

## Consequences

- Control-plane publication and runtime policy invalidation paths can share one
  side-effect-free transition contract.
- Unknown revision invalidation fails closed instead of poisoning cache state.
- Active invalidation falls back to last-known-good only when the cache already
  has a safe LKG revision.
