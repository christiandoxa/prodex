# ADR 0839: Redact domain authorization error debug output

Status: Accepted

## Context

`AuthorizationError` carries expected and actual credential scopes or roles.
Derived `Debug` output exposed those authorization comparison details through
diagnostics.

## Decision

Use a custom `Debug` implementation for `AuthorizationError` that preserves the
failure variant while redacting expected and actual authorization values.

## Consequences

Diagnostics can still distinguish scope mismatches from insufficient-role
failures, but exact scope and role comparisons no longer appear through domain
authorization error debug output.
