# ADR 0540: Gateway budget groups are governance-scoped

## Status

Accepted

## Context

Legacy virtual-key budget groups matched only on `budget_id`. In a multi-tenant
deployment, two tenants may legitimately use the same budget name. Matching only
the label can let one tenant's usage deny another tenant's requests.

## Decision

Budget-group aggregation now requires matching `budget_id` and governance scope:
tenant, team, project, and user. The existing budget label remains unchanged for
compatibility.

## Consequences

Same-name budget groups no longer cross tenant or governance boundaries. Durable
reservation storage must keep using tenant-scoped keys and constraints for
multi-replica enforcement.
