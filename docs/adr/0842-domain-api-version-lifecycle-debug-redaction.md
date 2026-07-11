# ADR 0842: Redact domain API version lifecycle debug output

Status: Accepted

## Context

`ApiVersion`, `ApiVersionStatus`, `ApiVersionPolicy`, and `ApiVersionDecision`
carry requested versions and lifecycle timestamps. Derived `Debug` output
exposed those values through diagnostics.

## Decision

Use custom `Debug` implementations for API versions, lifecycle status,
policies, and decisions that preserve lifecycle shape while redacting versions
and timestamps.

## Consequences

Diagnostics can still distinguish current, deprecated, sunset, and allowed
deprecated states, but API versions and lifecycle timestamps no longer appear
through lifecycle debug output.
