# ADR 0855: Redact gateway HTTP drain display output

Status: Accepted

## Context

Gateway HTTP drain planning validates deployment shutdown timing. Its
`Display` output named the Kubernetes pre-stop hook directly, so generic local
error formatting could expose deployment topology outside stable diagnostics.

## Decision

Keep typed drain-plan errors for planning and tests, but render missing drain
delay failures with generic HTTP drain wording.

## Consequences

Drain planning behavior is unchanged, while stringified local drain errors no
longer expose deployment hook names.
