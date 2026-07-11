# ADR 0912: Config activation plan debug redaction

## Status

Accepted.

## Context

Configuration activation plans carry the previous active revision, the newly
activated revision, and the next cache state. Derived `Debug` output exposed
revision metadata directly even after cache state fields were redacted.

## Decision

Use a custom `Debug` implementation for `ConfigActivationPlan` that redacts
direct revision fields and delegates next-state formatting to the redacted
`ConfigCacheState` formatter. Keep exact values available through typed fields
for activation logic.

Regression coverage rejects raw tenant IDs, revision IDs, and cache timings in
rendered activation-plan debug output.

## Consequences

Activation planning diagnostics can show plan shape without exposing revision
topology. Cache activation behavior remains unchanged.
