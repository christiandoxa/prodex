# ADR 0610: Domain Reservation Zero Estimate Guard

## Status

Accepted.

## Context

Reservation-based accounting must reject work before upstream dispatch when no
budget can be reserved. A reservation request with a zero token and zero cost
estimate could previously be accepted as a no-op reservation.

## Decision

Budget reservation now rejects `UsageAmount::ZERO` estimates with
`budget_estimate_invalid`.

## Consequences

Every admitted reservation must hold at least one usage dimension. Callers must
provide a non-zero estimate before dispatching upstream work.
