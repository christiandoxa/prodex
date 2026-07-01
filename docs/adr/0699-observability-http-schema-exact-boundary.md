# ADR 0699: Observability HTTP Schema Rejects Whitespace Padding

## Status

Accepted.

## Context

`gateway.observability.http_schema` selects the outbound HTTP telemetry payload
schema. Policy validation and direct runtime config resolution trimmed schema
values before matching canonical names such as `generic`, `otel`, and `datadog`.

## Decision

Observability HTTP schema values must be exact non-empty values without
whitespace. Policy validation rejects whitespace-bearing schema values, and
direct runtime config resolution fails closed instead of trim-normalizing them.

## Consequences

Canonical schema names remain valid. Padded schema values no longer silently
activate a telemetry payload format.
