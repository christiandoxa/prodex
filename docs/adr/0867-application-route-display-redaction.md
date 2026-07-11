# ADR 0867: Redact application route display topology

Status: Accepted

## Context

Application data-plane and quota-read planners reject wrong HTTP route classes
before gateway admission or quota authorization. Stable response planning
already maps those failures to a generic route denial, but local display output
still named data-plane or quota route topology.

## Decision

Keep typed wrong-route variants for response planning and tests, but render
local display output as one generic unavailable application-route message.

## Consequences

Data-plane and quota-read route enforcement remain unchanged, while
stringified errors no longer expose route topology.
