# ADR 0145: Gateway core storage error boundary

## Status

Accepted

## Context

Gateway-core composes authorization, provider validation, observability, and
adapter-neutral storage planning. ADR 0144 introduced stable redacted response
planners in `prodex-storage`, but gateway-core still had local copies of the
reservation, reconciliation, and expired-recovery storage error mappings.

Duplicating those mappings makes gateway responses drift from the shared storage
redaction boundary.

## Decision

Gateway-core now adapts the storage-level response planners for:

- atomic reservation failures during admission;
- usage reconciliation failures; and
- expired-reservation recovery failures.

Gateway-only tenant-affinity and telemetry failures remain gateway-owned.

## Consequences

Gateway data-plane response envelopes stay aligned with the adapter-neutral
storage redaction boundary while preserving gateway-specific codes where needed
for admission. Raw storage errors remain available only for trusted redacted
diagnostics.
