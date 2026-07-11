# ADR 0866: Redact application request authentication display topology

Status: Accepted

## Context

Application request authentication rejects unsupported authenticated routes and
credential-scope mismatches before data-plane or control-plane use cases run.
Stable response planning already redacts route and scope details, but local
display output still distinguished route and credential-scope failure topology.

## Decision

Keep typed authentication errors for response planning and tests, but render
local display output for wrong-route and scope-mismatch failures as one generic
request-authentication denial.

## Consequences

Request authentication behavior and response planning remain unchanged, while
stringified errors no longer expose route or credential-scope topology.
