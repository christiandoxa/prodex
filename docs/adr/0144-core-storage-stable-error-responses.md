# ADR 0144: Core storage stable error responses

## Status

Accepted

## Context

`prodex-storage` owns adapter-neutral commands and planning checks for tenant
storage keys, atomic reservation, usage reconciliation, expired-reservation
recovery, append-only audit envelopes, gateway topology, and multi-replica
accounting verification. Raw planning errors can contain tenant IDs, usage
amounts, replica counts, and topology details. Those details are useful for
trusted diagnostics but should not be copied into generic API or operator-facing
responses.

## Decision

Add stable redacted response planners to `prodex-storage` for:

- storage topology validation;
- atomic reservation planning;
- usage reconciliation planning;
- expired-reservation recovery planning;
- append-only audit planning; and
- multi-replica accounting verification planning.

Application runtime error responses now adapt the topology and accounting
planners instead of matching those raw errors directly.

## Consequences

Adapter-neutral storage failures have a reusable redaction boundary before they
reach gateway, application, or future HTTP adapters. Trusted diagnostics may
still log raw errors through redacted logging paths, while client-facing
responses use stable machine-readable codes and generic messages.
