# ADR 0810: Redact health check debug messages

Status: Accepted

## Context

`HealthCheck` carries optional free-form readiness diagnostics. Its derived
`Debug` formatter exposed those messages, which can include endpoint, DSN, path,
or backend details.

## Decision

Use a custom `Debug` implementation for `HealthCheck` that preserves the check
name and state while redacting the optional message value.

## Consequences

Diagnostics can still identify which health check changed state, but raw
readiness messages no longer appear through `HealthCheck` debug output.
