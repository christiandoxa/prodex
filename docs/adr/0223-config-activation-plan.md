# 0223: Config Activation Plan

## Status

Accepted.

## Context

The enterprise gateway needs revisioned policy/config snapshots with atomic
activation, invalidation, and last-known-good fallback. `prodex-config` already
models cache windows, refresh decisions, publication validation, and stable
errors, but adapters still need a reusable transition from an authorized
published snapshot into the next gateway cache state.

If each adapter mutates cache state independently, active revision,
last-known-good revision, and invalidation clearing can drift.

## Decision

Add `plan_config_activation` to `prodex-config`. The planner:

- validates candidate tenant and revision ordering against the current state;
- activates the candidate revision;
- promotes the previous active revision to last-known-good;
- clears invalidation for the newly activated state;
- preserves the configured refresh/stale/expiry window.

The planner remains side-effect-free and does not read files, contact a control
plane, or mutate storage.

## Consequences

- Gateway and control-plane composition roots have one cache activation
  contract for revisioned policy snapshots.
- Cross-tenant and stale publication candidates fail before cache mutation.
- Last-known-good state is updated consistently when a new snapshot becomes
  active.
