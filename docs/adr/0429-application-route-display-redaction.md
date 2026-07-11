# ADR 0429: Redact application route display values

## Status

Accepted.

## Context

Application-layer wrong-route errors kept stable response envelopes, but their
`Display` text still printed internal `GatewayHttpRouteKind` enum names.

## Decision

Authentication, data-plane, and quota wrong-route display text now describes
the error class without including the route enum value. The typed error variant
still carries the route for callers that need structured handling.

## Consequences

Adapter logs and propagated application errors avoid leaking internal route
classification names. Tests cover both response redaction and display text.
