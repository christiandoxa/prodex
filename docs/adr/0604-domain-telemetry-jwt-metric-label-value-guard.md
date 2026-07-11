# ADR 0604: Domain Telemetry JWT Metric Label Value Guard

## Status

Accepted.

## Context

The telemetry metric label guard rejects common credential prefixes, but callers
can pass a bearer token value without the `Bearer ` scheme.

## Decision

Metric label value validation now rejects JWT-shaped values with an `eyJ` header
prefix and three base64url segments.

## Consequences

Raw bearer tokens stay out of metrics even when passed without their auth
scheme. Ordinary dotted categorical labels remain valid.
