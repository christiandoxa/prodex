# ADR 0447: Authz display errors are redacted

## Status

Accepted.

## Context

Authorization response planning already returns stable, redacted envelopes, but
`BoundaryAuthorizationError::Display` still included boundary, role, and scope
details. A composition root that accidentally surfaced `Display` could leak
policy internals.

## Decision

`prodex-authz` now keeps `Display` messages generic for credential-scope and
role denials. Structured enum fields remain available for trusted diagnostics
and audit paths.

## Consequences

Client-facing and accidental-display paths no longer expose role names, scope
names, or boundary names. Debug output still carries structured values for
tests and trusted tooling.
