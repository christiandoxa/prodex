# ADR 0550: Redact gateway HTTP observability export failures

## Status

Accepted

## Context

Gateway HTTP observability exports can be configured with operator-owned
endpoints. If an endpoint URL includes a token-like query parameter, or if a
transport error displays bearer material, logging the raw failure can expose
telemetry credentials.

## Decision

Reuse the existing redaction boundary before writing HTTP observability export
failure diagnostics. The sink still logs the request sequence and failure event,
but endpoint and error fields are redacted first.

## Consequences

Observability export failures remain diagnosable without leaking configured
tokens, bearer values, or secret-like endpoint query parameters.
