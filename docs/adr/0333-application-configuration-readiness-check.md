# ADR 0333: Application Configuration Readiness Check

## Status

Accepted.

## Context

The application configuration readiness planner maps config cache state into a
domain `HealthSnapshot`. Exposing only the selected revision is not enough for
operators: async refresh and last-known-good fallback should be visible as
degraded readiness, while refresh-required or invalidated-without-fallback should
be visible as failing readiness.

## Decision

Extend `plan_application_configuration_readiness_snapshot` so it appends a
`configuration` health check derived from the `ConfigRefreshDecision`.

`UseActive` is passing. `RefreshAsync` and `UseLastKnownGood` are degraded.
`RefreshRequired` and `RejectedInvalidated` are failing. The check messages are
stable, generic, and do not include tenant IDs or revision IDs.

## Consequences

- Readiness can distinguish fully healthy config from fallback or refresh states.
- Last-known-good config remains serviceable but operationally degraded.
- Missing or invalidated config without fallback fails readiness through the
  canonical health model.
