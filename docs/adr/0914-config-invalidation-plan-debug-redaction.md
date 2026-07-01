# ADR 0914: Config invalidation plan debug redaction

## Status

Accepted.

## Context

Configuration invalidation plans carry tenant IDs, invalidated revision IDs, and
the next cache state. Derived `Debug` output exposed direct invalidation
metadata even after cache-state debug output was redacted.

## Decision

Use a custom `Debug` implementation for `ConfigInvalidationPlan` that redacts
direct tenant and revision fields and delegates nested cache state formatting to
`ConfigCacheState`. Keep exact values available through typed fields for
invalidation logic.

Regression coverage rejects raw tenant IDs, revision IDs, and cache timings in
rendered invalidation-plan debug output.

## Consequences

Configuration invalidation diagnostics can show plan shape without exposing
tenant topology or revision metadata. Invalidation behavior remains unchanged.
