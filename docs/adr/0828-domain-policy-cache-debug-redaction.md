# ADR 0828: Redact domain policy cache debug output

Status: Accepted

## Context

`PolicyRefreshWindow` and `PolicyCacheStatus` carry cache timing windows and
policy revision state. Derived `Debug` output exposed raw timestamps and cache
revision identifiers through diagnostics.

## Decision

Use custom `Debug` implementations for policy refresh windows and cache status
that preserve cache shape while redacting timestamps and revision identity.

## Consequences

Diagnostics can still distinguish active, last-known-good, invalidated, and
timing-window state, but raw policy cache timing and revision details no longer
appear through debug output.
