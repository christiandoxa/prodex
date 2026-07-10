# ADR 0495: Align Health OpenAPI Schema With Runtime Status Field

## Status

Accepted.

## Context

Gateway health probes return a stable `gateway.health` JSON object with a string `status` field. The OpenAPI component still described a required boolean `ok` field, which made generated clients and validators disagree with `/livez`, `/readyz`, and `/startupz`.

## Decision

The `GatewayHealth` OpenAPI schema now requires `status` and documents the current `ok`, `overloaded`, and `draining` values. The stale `ok` property was removed from the schema. The schema also requires nullable `policy_version` and the `draining` boolean, matching the runtime probe payload that always includes the active policy version and drain-state slots.

## Consequences

Machine-readable API documentation now matches runtime probe responses. Existing health responses are unchanged.
