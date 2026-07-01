# ADR 0876: Redact domain resource authorization display output

Status: Accepted

## Context

Resource authorization errors wrap scope, role, and tenant-access failures.
Response planning keeps typed outcomes, but local display output still revealed
which authorization topology failed through delegated display formatting.

## Decision

Keep typed resource authorization variants for response planning and tests, but
render `ResourceAuthorizationError` display output as one generic resource
authorization denial.

## Consequences

Resource authorization behavior and response planning remain unchanged, while
stringified errors no longer expose scope, role, or tenant topology.
