# ADR 0602: Domain Telemetry Secret Base Metric Label Key Guard

## Status

Accepted.

## Context

The domain telemetry boundary rejects identifier-like metric label keys, but
exact base keys such as `token`, `secret`, `credential`, `password`, or `key`
could still carry sensitive or high-cardinality material.

## Decision

Metric label key validation now rejects those exact base secret/credential keys.
Closed categorical labels such as `secret_backend` and
`credential_rotation_result` remain valid.

## Consequences

Secret and credential material must remain redacted trace-only or trusted log
data. Metrics keep categorical backend, operation, and result labels.
