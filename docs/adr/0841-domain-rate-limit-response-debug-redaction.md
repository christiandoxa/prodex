# ADR 0841: Redact domain rate-limit response debug retry timing

Status: Accepted

## Context

`RateLimitErrorResponsePlan` carries retry-after timing as typed response
metadata. Derived `Debug` output exposed that timing value through diagnostics.

## Decision

Use a custom `Debug` implementation for `RateLimitErrorResponsePlan` that
preserves status, code, and message while redacting retry-after timing.

## Consequences

Callers still receive the typed retry-after value, but diagnostics no longer
expose it through rate-limit response plan debug output.
