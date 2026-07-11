# ADR 0865: Redact control-plane authorization display topology

Status: Accepted

## Context

Control-plane authorization errors distinguish scope, role, tenant, resource,
and break-glass failures. Stable response planning keeps those typed outcomes,
but local display output still revealed which control-plane authorization
topology failed.

## Decision

Keep typed authorization variants for response planning, audits, and tests, but
render all `ControlPlaneAuthorizationError` display output as one generic
denied authorization message.

## Consequences

Control-plane authorization behavior and response planning remain unchanged,
while stringified errors no longer expose scope, role, resource, tenant, or
break-glass topology.
