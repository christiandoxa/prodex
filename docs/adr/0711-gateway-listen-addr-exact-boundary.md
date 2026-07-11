# ADR 0711: Gateway Listen Address Exact Boundary

## Status

Accepted.

## Context

`gateway.listen_addr` and `--listen` choose the local network bind address for
gateway launch. Policy validation and runtime launch previously trimmed the
value before use, so padded input could be silently normalized into an exposed
listener.

## Decision

Treat gateway listen addresses as exact configuration boundaries. Policy
validation rejects empty or whitespace-bearing `gateway.listen_addr` values, and
runtime launch rejects invalid CLI or policy listen addresses instead of
trimming them.

## Consequences

Operators must correct padded listen addresses. Gateway network exposure remains
explicit and auditable rather than relying on hidden cleanup.
