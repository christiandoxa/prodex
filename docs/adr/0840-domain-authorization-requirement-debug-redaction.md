# ADR 0840: Redact domain authorization requirement debug output

Status: Accepted

## Context

`AuthorizationRequirement` carries resource, action, scope, and role policy
requirements. Derived `Debug` output exposed those authorization policy details
through diagnostics.

## Decision

Use a custom `Debug` implementation for `AuthorizationRequirement` that
preserves requirement shape while redacting resource, action, scope, and role
values.

## Consequences

Diagnostics can still identify an authorization requirement value, but exact
authorization policy requirements no longer appear through its debug output.
