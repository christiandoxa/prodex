# ADR 0955: Storage Multi-Replica Accounting Error Debug Redaction

## Status

Accepted

## Context

Multi-replica accounting verification errors can carry storage topology,
replica counts, and required evidence checks. Display output is generic, but
derived debug output would expose those operational details in diagnostics.

## Decision

Use a custom `Debug` implementation for
`MultiReplicaAccountingConcurrencySpecError`. Redact topology, replica-count,
and evidence-check fields while preserving error variant names.

## Consequences

Storage diagnostics can distinguish multi-replica accounting verification
failures without leaking topology, replica count, or checklist details.
