# ADR 0833: Redact domain SLO debug output

Status: Accepted

## Context

Domain SLO objectives, measurements, and alert decisions carry objective names,
observed values, and targets. Derived `Debug` output exposed those details
through diagnostics.

## Decision

Use custom `Debug` implementations for `SloObjective`, `SliMeasurement`, and
`AlertDecision` that preserve SLI, direction, and severity shape while
redacting objective names and numeric values.

## Consequences

Diagnostics can still distinguish SLO type and alert severity, but objective
names, observed values, and targets no longer appear through SLO debug output.
