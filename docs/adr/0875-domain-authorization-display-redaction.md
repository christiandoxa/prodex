# ADR 0875: Redact domain authorization display output

Status: Accepted

## Context

Domain authorization errors carry expected and actual credential scopes or
roles. Debug output already redacts those fields, but local display output still
rendered scope and role names directly.

## Decision

Keep typed authorization variants for response planning and tests, but render
all domain `AuthorizationError` display output as one generic denied
authorization message.

## Consequences

Domain authorization behavior and response planning remain unchanged, while
stringified errors no longer expose scope or role topology.
