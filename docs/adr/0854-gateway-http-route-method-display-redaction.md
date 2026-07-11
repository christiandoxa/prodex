# ADR 0854: Redact gateway HTTP route method display output

Status: Accepted

## Context

`GatewayHttpPlanError::MethodNotAllowed` carries route and method details for
typed response planning. Its `Display` output still referenced route topology
through generic local error formatting.

## Decision

Keep the route and method fields for trusted planning, but render local display
output as a generic method-not-allowed message.

## Consequences

Response planning is unchanged, while stringified gateway HTTP plan errors no
longer expose route topology.
