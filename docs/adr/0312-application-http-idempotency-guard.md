# ADR 0312: Application HTTP Idempotency Guard

## Status

Accepted.

## Context

Control-plane HTTP route/action binding now protects both digest-derived and
precomputed-fingerprint idempotency paths. That protection is security-sensitive
because a regression could let an adapter accept a replay key for one admin
route while executing another control-plane operation.

## Decision

`ci:application-boundary-guard` now checks `prodex-application` source for the
shared `validate_control_plane_http_action` path, both HTTP idempotency helpers,
the explicit `OperationMismatch` error, and the stable
`control_plane_route_invalid` response.

The guard self-test rejects sources that remove the precomputed-fingerprint
route/action validation or replace the redacted response with dynamic operation
details.

## Consequences

The route/action binding remains enforced by tests and by the enterprise
application boundary guard. Future adapter work can still evolve route mappings,
but it must keep the shared validation path and redacted error contract intact.
