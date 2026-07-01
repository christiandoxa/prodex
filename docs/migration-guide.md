# Prodex Enterprise Migration Guide

## Purpose

This guide describes the incremental path from the current single-binary runtime
proxy implementation to the enterprise target: a modular monolith with separate
data-plane gateway and control-plane boundaries, durable tenant-scoped storage,
external migrations, secure authentication/authorization, and production
observability.

This is not a big-bang rewrite plan. Each phase must be reversible, tested, and
compatible with existing CLI and provider behavior unless a change is explicitly
required for security.

## Phase 0: Baseline and Characterization

1. Capture current CLI, gateway API, provider fallback, streaming, cancellation,
   upstream error, quota, and continuation-affinity behavior.
2. Keep focused runtime proxy tests for:
   - `previous_response_id -> profile` affinity;
   - `x-codex-turn-state -> profile` affinity;
   - `session_id -> profile` compact affinity;
   - pre-commit route rotation;
   - no mid-stream rotation;
   - upstream error pass-through.
3. Keep `prodex-app` as the binary composition root while boundary crates are
   introduced.

## Phase 1: Domain and Security Boundaries

1. Introduce canonical typed IDs for tenants, principals, requests, calls,
   reservations, virtual keys, policy revisions, and audit events.
2. Move role mapping, tenant resolution, authorization, idempotency, accounting,
   policy cache, JWKS cache, audit, and telemetry primitives into `prodex-domain`.
3. Require explicit role mapping. Missing or unknown role claim must deny or map
   to Viewer, never Admin.
4. Require tenant claim in multi-tenant mode.
5. Split authentication and authorization into `prodex-authn` and `prodex-authz`.
6. Add negative tests for privilege escalation, stale identity metadata, replay,
   and scope misuse.

## Phase 2: Application and Gateway Boundaries

1. Introduce `prodex-application` for use-case orchestration.
2. Introduce `prodex-gateway-core` for data-plane admission planning:
   authentication result, tenant context, authorization, atomic reservation plan,
   provider invocation validation, and telemetry spans.
3. Introduce `prodex-gateway-http` for route classification, body limits, timeout
   budgets, trace propagation, and upstream header policy.
4. Keep `prodex-app` wiring thin. New business rules must land in boundary crates
   first, then adapters call those boundaries.

## Phase 3: Control Plane Boundary

1. Introduce `prodex-control-plane` for tenant, user, service identity, virtual
   key, policy, provider credential, budget, billing, audit, and configuration
   operations.
2. Enforce control-plane credential scope for admin operations.
3. Enforce separate break-glass scope with reason, expiry, and audit.
4. Add control-plane audit events for success and denial.
5. Migrate legacy admin handlers one route at a time to the control-plane
   boundary, preserving response/error compatibility where secure.

## Phase 4: Durable Storage and Migrations

1. Treat PostgreSQL as the production source of truth.
2. Move schema ownership into versioned storage crates:
   - `prodex-storage-postgres` for production durable state and RLS;
   - `prodex-storage-sqlite` for local compatibility;
   - `prodex-storage-redis` for rate limits, cache, and rebuildable coordination.
3. Run DDL only from explicit external migrators or controlled rollout jobs.
4. Request-serving paths must reject migration planning and must not execute DDL
   while opening a backend.
5. Add tenant ID to every tenant-owned key, query predicate, index, foreign key,
   unique constraint, audit event, cache key, and telemetry attribute where
   appropriate.
6. Add PostgreSQL Row-Level Security policies using tenant context as defense in
   depth.

## Phase 5: Reservation-Based Accounting

1. Reserve estimated usage atomically before upstream provider calls.
2. Reject before upstream call when budget/rate limit is not available.
3. Commit actual usage after response.
4. Release unused reservation amount.
5. Expire abandoned reservations.
6. Use `ReservationId` and `CallId` in idempotency and ledger uniqueness.
7. Reconcile completed, cancelled, and stream-interrupted calls.
8. Verify with multi-replica tests sharing PostgreSQL/Redis that there is no lost
   update, duplicate charge, dropped ledger event, request ID collision, or
   undocumented limit overshoot.

## Phase 6: Async Gateway Adapter

1. Add a concrete async HTTP adapter after policy behavior is covered by
   `prodex-gateway-http` tests.
2. Use bounded concurrency, body limits, timeout budgets, cancellation
   propagation, streaming backpressure, graceful shutdown, and connection
   draining.
3. Wrap unavoidable blocking work in explicitly bounded blocking pools.
4. Preserve upstream Codex transport behavior, including stream semantics,
   reconnect handling, and upstream error compatibility.
5. Do not print to terminal while the Codex TUI is running; runtime notices go to
   logs.

## Phase 7: Observability and Operations

1. Propagate W3C `traceparent`/`tracestate` end to end.
2. Emit OpenTelemetry-compatible spans, metrics, and logs from adapter layers.
3. Bound metric label cardinality and avoid tenant/user secret leakage.
4. Keep `/livez`, `/readyz`, and `/startupz` routes lightweight and unauthenticated
   only where deployment policy allows.
5. Maintain backup/restore, Kubernetes, and deployment security artifacts.

## Phase 8: Cutover and Compatibility

1. Run legacy and new adapters side by side in staging.
2. Replay compatibility fixtures for CLI workflows, gateway API responses,
   provider behavior, streaming, cancellation, and errors.
3. Migrate tenants with expand/backfill/contract migrations.
4. Use feature flags or config switches for adapter rollout.
5. Roll back by returning traffic to the legacy adapter without schema downgrade
   while expand-compatible migrations are active.

## Release Gates

Before declaring the enterprise target complete, verify:

- All target boundary crates exist and are protected by guards.
- `prodex-app` contains composition logic only for migrated paths.
- Data-plane and control-plane binaries or entrypoints are separately runnable.
- PostgreSQL migrations are external and RLS is enabled.
- Redis is not used as durable whole-map billing state.
- OIDC/JWKS network fetches are off request paths.
- Full test suite and focused runtime proxy tests pass.
- Architecture docs, ADRs, threat model, migration guide, deployment docs, and
  backup/restore docs are current.
