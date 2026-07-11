# ADR 0551: Redact gateway JSONL observability export failures

## Status

Accepted

## Context

Gateway JSONL observability exports write to an operator-configured path. Even
though paths should not contain secrets, bad deployments can place token-like
material in directory names or produce filesystem errors that echo those paths.

## Decision

Reuse the existing redaction boundary before logging JSONL observability export
failures. The sink still reports the request sequence and failure event, but
path and error fields are redacted first.

## Consequences

Filesystem export failures remain diagnosable without leaking secret-like path
segments or error text.
