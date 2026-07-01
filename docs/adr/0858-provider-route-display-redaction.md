# ADR 0858: Redact provider route display metadata

Status: Accepted

## Context

Provider route validation distinguishes missing and overlong model names. Its
`Display` output named the model field directly, so generic local error
formatting could expose route metadata outside the stable response planner.

## Decision

Keep typed route validation variants for planning and tests, but render local
display output as a generic provider-route validation failure.

## Consequences

Provider route validation and response planning remain unchanged, while
stringified route errors no longer expose model-field metadata.
