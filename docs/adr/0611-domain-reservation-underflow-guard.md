# ADR 0611: Domain Reservation Underflow Guard

## Status

Accepted.

## Context

Reservation commit, reconciliation, and expired recovery subtracted reserved
usage with saturating arithmetic. A duplicate or stale operation could therefore
hide that the snapshot no longer held enough reserved usage.

## Decision

Reservation commit, reconciliation, and expired recovery now reject when the
operation's reserved amount exceeds the snapshot's reserved balance.

## Consequences

Accounting fails closed instead of silently masking double release, duplicate
commit, or stale snapshot bugs. Client-facing responses stay stable and
redacted through the existing accounting response planners.
