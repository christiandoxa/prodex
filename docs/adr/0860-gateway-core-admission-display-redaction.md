# ADR 0860: Redact gateway admission display topology

Status: Accepted

## Context

Gateway admission errors distinguish unavailable authorization boundaries from
tenant mismatches. Stable response planning already redacts those details, but
generic local display output still named boundary and tenant topology.

## Decision

Keep typed admission variants for response planning and tests, but render local
display output with generic gateway admission wording.

## Consequences

Gateway admission behavior and response planning remain unchanged, while
stringified admission errors no longer expose authorization-boundary or tenant
topology.
