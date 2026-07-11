# 0224: Application Config Activation Boundary

## Status

Accepted.

## Context

ADR 0223 added a side-effect-free config cache activation planner in
`prodex-config`. Application composition roots still need a single place to
connect authorized configuration publication decisions to that activation plan.

Without an application boundary, adapters can activate denied publications,
skip last-known-good promotion, or map activation failures inconsistently.

## Decision

Add `plan_application_configuration_activation` to `prodex-application`.

The planner:

- activates only `Authorized` configuration publication decisions;
- returns no activation for denied publication decisions;
- delegates cache state transition to `prodex-config::plan_config_activation`;
- optionally plans a tenant-scoped Redis policy revision cache write with an
  explicit TTL and revision ID;
- maps activation validation failures to stable redacted application responses.

The application planner remains side-effect-free. Real gateway adapters still
own persistence, cache writes, and publication event delivery.

## Consequences

- Control-plane publication and gateway cache activation now share one
  application-level handoff.
- Denied publication decisions cannot accidentally mutate gateway cache state.
- Activation errors stay stable and do not expose tenant IDs, revision IDs, or
  cache internals.
- Redis remains a rebuildable short-lived cache, not a durable source of truth.
