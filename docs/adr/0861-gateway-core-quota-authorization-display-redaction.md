# ADR 0861: Redact gateway quota authorization display topology

Status: Accepted

## Context

Gateway quota-read authorization errors distinguish unavailable authorization
boundaries from tenant mismatches. Stable response planning already redacts
those details, but generic local display output still named boundary and tenant
topology.

## Decision

Keep typed quota authorization variants for response planning and tests, but
render local display output with generic gateway quota authorization wording.

## Consequences

Gateway quota authorization behavior and response planning remain unchanged,
while stringified quota authorization errors no longer expose boundary or
tenant topology.
