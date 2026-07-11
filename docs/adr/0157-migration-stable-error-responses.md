# ADR 0157: Migration stable error responses

## Status

Accepted

## Context

The domain migration boundary models version validation, dedicated migrator
execution rules, expand/backfill/verify/contract ordering, and startup
compatibility windows. Raw migration errors can reveal request-path migration
mode, lock-owner requirements, failed-step topology, step ordering internals, or
compatible source-version windows. Those details are useful in trusted
diagnostics and migration status commands, but client-visible startup/readiness
or control-plane APIs should return stable redacted error envelopes.

## Decision

Add stable response planners to `prodex-domain`:

- `plan_migration_version_error_response`;
- `plan_migration_plan_error_response`; and
- `plan_migration_compatibility_error_response`.

The planners map migration failures to deterministic status/code/message triples
without exposing lock owners, migration versions, step names, ordering internals,
or compatibility window contents.

## Consequences

Gateway, control-plane, and migrator composition roots can expose
machine-readable migration failures while keeping detailed migration topology in
trusted diagnostics, migration status commands, and audit trails. Request-serving
runtime migration attempts remain explicitly rejected.
