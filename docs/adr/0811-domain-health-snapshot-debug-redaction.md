# ADR 0811: Redact health snapshot debug output

Status: Accepted

## Context

`HealthSnapshot` carries readiness booleans, active policy revision metadata,
and the full set of health checks. Its derived `Debug` formatter exposed raw
policy revision identifiers and check internals through diagnostics.

## Decision

Use a custom `Debug` implementation for `HealthSnapshot` that preserves readiness
booleans, active-policy presence, and check count while redacting raw revision
and check details.

## Consequences

Diagnostics can still reason about readiness shape, but raw policy revision and
health-check internals no longer appear through `HealthSnapshot` debug output.
