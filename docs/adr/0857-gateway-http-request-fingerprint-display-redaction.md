# ADR 0857: Redact gateway HTTP request fingerprint display output

Status: Accepted

## Context

Gateway HTTP request fingerprint planning validates request paths and
adapter-supplied body digests. Its `Display` output named those inputs directly,
so generic local error formatting could expose replay-sensitive metadata.

## Decision

Keep typed fingerprint errors for response planning, but render local display
output as a generic request-fingerprint validation failure.

## Consequences

Fingerprint derivation and response planning remain unchanged, while
stringified local errors no longer expose path or body-digest metadata.
