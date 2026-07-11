# ADR 0864: Redact authz display topology

Status: Accepted

## Context

Authorization errors distinguish credential-scope, role, principal-kind, and
tenant-access failures. Stable response planning keeps those typed outcomes for
clients, but generic local display output still exposed which authorization
topology failed.

## Decision

Keep typed authorization variants for response planning, audits, and tests, but
render all `BoundaryAuthorizationError` display output as a single denied
authorization message.

## Consequences

Authorization behavior and response planning remain unchanged, while
stringified authorization errors no longer expose boundary, role, principal, or
tenant topology.
