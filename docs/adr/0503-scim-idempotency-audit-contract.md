# ADR 0503: SCIM mutations require idempotency and audit

## Status

Accepted

## Context

SCIM user create, update, and delete operations are control-plane identity
mutations. Enterprise API governance requires mutating admin operations to be
idempotent and audited.

## Decision

The gateway HTTP control-plane route planner marks SCIM user create, update, and
delete operations as requiring idempotency and audit.

Regression coverage lives in
`control_plane_route_planner_maps_scim_paths_and_methods`.

## Consequences

Identity lifecycle adapters get the same replay and audit contract as other
control-plane mutations.
