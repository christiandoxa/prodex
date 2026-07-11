# ADR 0913: Config publication event plan debug redaction

## Status

Accepted.

## Context

Configuration publication event plans carry tenant IDs, activated revision IDs,
previous active revisions, last-known-good revisions, and delivery targets.
Derived `Debug` output exposed tenant and revision metadata.

## Decision

Use a custom `Debug` implementation for `ConfigPublicationEventPlan` that
redacts tenant and revision identifiers while preserving the low-cardinality
target list. Keep exact values available through typed fields for delivery
planning.

Regression coverage rejects raw tenant and revision IDs in rendered publication
event-plan debug output.

## Consequences

Publication delivery diagnostics can show target shape without exposing tenant
topology or policy revision metadata. Delivery behavior remains unchanged.
